//! The sync file: header, master key locked by password and sync secret, encrypted body
//! (ADR-0018, docs/specs/sync-format-v1.md).

use std::fmt;

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use thiserror::Error;
use zeroize::Zeroizing;

use super::container::{array_at, decrypt, encrypt};
use super::error::VaultError;
use super::kdf::{derive_key, KdfParams, KEY_LENGTH, SALT_LENGTH};
use super::model::VaultData;
use super::payload;
use super::rng::{random_array, Rng};
use super::sync_secret::SyncSecret;

const MAGIC: &[u8; 8] = b"MFASYNC\0";
const VERSION: u16 = 1;
const KDF_ARGON2ID_HKDF: u8 = 1;
const AEAD_XCHACHA20_POLY1305: u8 = 1;
const KEK_INFO: &[u8] = b"mfa-provider sync v1 kek";

const NONCE_LENGTH: usize = 24;
const TAG_LENGTH: usize = 16;
const WRAPPED_KEY_LENGTH: usize = KEY_LENGTH + TAG_LENGTH;
pub const SYNC_ID_LENGTH: usize = 16;

const WRAP_AAD_END: usize = 77;
/// Everything up to the body nonce: authenticated by the body.
const HEADER_LENGTH: usize = 125;
const BODY_START: usize = 153;
const MAX_FILE_LENGTH: usize = 64 * 1024 * 1024;

pub type SyncId = [u8; SYNC_ID_LENGTH];
pub type MasterKey = Zeroizing<[u8; KEY_LENGTH]>;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SyncFileError {
    #[error("not a sync file")]
    NotASyncFile,
    #[error("this sync file needs a newer version of the app")]
    Unsupported,
    #[error("the sync file's header is damaged")]
    InvalidHeader,
    #[error("wrong sync password or sync secret, or a damaged header")]
    UnwrapFailed,
    #[error("the sync file belongs to another master key")]
    OtherMasterKey,
    #[error("the sync file is damaged or has been altered")]
    Damaged,
    #[error(transparent)]
    Vault(#[from] VaultError),
}

/// The part of a sync file that only a password change or a new master key rewrites.
#[derive(Clone, PartialEq, Eq)]
pub struct SyncHeader([u8; HEADER_LENGTH]);

impl fmt::Debug for SyncHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SyncHeader(..)")
    }
}

impl SyncHeader {
    /// Locks `master_key` so that only `password` and `secret` together open it.
    ///
    /// # Errors
    /// If key derivation or encryption fails.
    pub fn lock(
        password: &str,
        secret: &SyncSecret,
        sync_id: SyncId,
        master_key: &MasterKey,
        params: KdfParams,
        rng: &mut dyn Rng,
    ) -> Result<Self, SyncFileError> {
        let salt = random_array::<SALT_LENGTH>(rng);
        let wrap_nonce = random_array::<NONCE_LENGTH>(rng);
        let mut header = [0_u8; HEADER_LENGTH];
        header[..8].copy_from_slice(MAGIC);
        header[8..10].copy_from_slice(&VERSION.to_be_bytes());
        header[10] = KDF_ARGON2ID_HKDF;
        header[11] = AEAD_XCHACHA20_POLY1305;
        header[12..28].copy_from_slice(&sync_id);
        header[28..32].copy_from_slice(&params.memory_kib().to_be_bytes());
        header[32..36].copy_from_slice(&params.iterations().to_be_bytes());
        header[36] = params.lanes();
        header[37..53].copy_from_slice(&salt);
        header[53..WRAP_AAD_END].copy_from_slice(&wrap_nonce);
        let kek = unlock_key(password, secret, &sync_id, &salt, params)?;
        let wrapped = encrypt(&kek, &wrap_nonce, &header[..WRAP_AAD_END], &**master_key)?;
        header[WRAP_AAD_END..].copy_from_slice(&wrapped[..WRAPPED_KEY_LENGTH]);
        Ok(Self(header))
    }

    #[must_use]
    pub fn sync_id(&self) -> SyncId {
        array_at(&self.0, 12)
    }

    /// A complete sync file with this header.
    ///
    /// # Errors
    /// If the data cannot be encoded or encryption fails.
    pub fn seal(
        &self,
        master_key: &MasterKey,
        revision: u64,
        data: &VaultData,
        rng: &mut dyn Rng,
    ) -> Result<Vec<u8>, SyncFileError> {
        let padded = payload::pad(&payload::encode_shared(data, revision)?);
        let nonce = random_array::<NONCE_LENGTH>(rng);
        let body = encrypt(master_key, &nonce, &self.0, &padded)?;
        let length = u32::try_from(body.len()).map_err(|_| SyncFileError::InvalidHeader)?;
        let mut file = Vec::with_capacity(BODY_START + body.len());
        file.extend_from_slice(&self.0);
        file.extend_from_slice(&nonce);
        file.extend_from_slice(&length.to_be_bytes());
        file.extend_from_slice(&body);
        Ok(file)
    }
}

/// A sync file that has been opened and authenticated.
#[derive(Debug)]
pub struct OpenedSyncFile {
    pub header: SyncHeader,
    pub revision: u64,
    pub data: VaultData,
}

struct Parsed<'a> {
    header: SyncHeader,
    params: KdfParams,
    body_nonce: [u8; NONCE_LENGTH],
    body: &'a [u8],
}

/// Opens a file with the master key a joined device already holds.
///
/// # Errors
/// [`SyncFileError::OtherMasterKey`] if the file has another sync id;
/// [`SyncFileError::Damaged`] if the header or body does not authenticate; or a malformed or
/// unsupported file.
pub fn open_with_master_key(
    sync_id: &SyncId,
    master_key: &MasterKey,
    file: &[u8],
) -> Result<OpenedSyncFile, SyncFileError> {
    let parsed = parse(file)?;
    if parsed.header.sync_id() != *sync_id {
        return Err(SyncFileError::OtherMasterKey);
    }
    open_body(parsed, master_key)
}

/// Opens a file the way a joining device does, and also returns its master key.
///
/// # Errors
/// [`SyncFileError::UnwrapFailed`] for a wrong password or secret (indistinguishable from a
/// tampered password slot); otherwise as [`open_with_master_key`].
pub fn open_with_password(
    password: &str,
    secret: &SyncSecret,
    file: &[u8],
) -> Result<(MasterKey, OpenedSyncFile), SyncFileError> {
    let parsed = parse(file)?;
    let header = &parsed.header.0;
    let kek = unlock_key(
        password,
        secret,
        &parsed.header.sync_id(),
        &array_at(header, 37),
        parsed.params,
    )?;
    let unwrapped = decrypt(
        &kek,
        &array_at(header, 53),
        &header[..WRAP_AAD_END],
        &header[WRAP_AAD_END..],
    )
    .ok_or(SyncFileError::UnwrapFailed)?;
    let master_key = Zeroizing::new(
        <[u8; KEY_LENGTH]>::try_from(unwrapped.as_slice())
            .map_err(|_| SyncFileError::UnwrapFailed)?,
    );
    let opened = open_body(parsed, &master_key)?;
    Ok((master_key, opened))
}

fn open_body(parsed: Parsed<'_>, master_key: &MasterKey) -> Result<OpenedSyncFile, SyncFileError> {
    let padded = decrypt(
        master_key,
        &parsed.body_nonce,
        &parsed.header.0,
        parsed.body,
    )
    .ok_or(SyncFileError::Damaged)?;
    let (revision, data) = payload::decode_shared(payload::unpad(&padded)?)?;
    Ok(OpenedSyncFile {
        header: parsed.header,
        revision,
        data,
    })
}

/// Checks everything that can be checked without a key, including the cost settings, before
/// anything is derived from an untrusted header.
fn parse(file: &[u8]) -> Result<Parsed<'_>, SyncFileError> {
    if !file.starts_with(MAGIC) {
        return Err(SyncFileError::NotASyncFile);
    }
    if file.len() > MAX_FILE_LENGTH || file.len() < BODY_START {
        return Err(SyncFileError::InvalidHeader);
    }
    if u16::from_be_bytes(array_at(file, 8)) != VERSION
        || file[10] != KDF_ARGON2ID_HKDF
        || file[11] != AEAD_XCHACHA20_POLY1305
    {
        return Err(SyncFileError::Unsupported);
    }
    let params = KdfParams::new(
        u32::from_be_bytes(array_at(file, 28)),
        u32::from_be_bytes(array_at(file, 32)),
        file[36],
    )?;
    let length = usize::try_from(u32::from_be_bytes(array_at(file, 149)))
        .map_err(|_| SyncFileError::InvalidHeader)?;
    if length < TAG_LENGTH || length != file.len() - BODY_START {
        return Err(SyncFileError::InvalidHeader);
    }
    Ok(Parsed {
        header: SyncHeader(array_at(file, 0)),
        params,
        body_nonce: array_at(file, HEADER_LENGTH),
        body: &file[BODY_START..],
    })
}

/// Argon2id of the password, then bound to the sync secret and sync id with HKDF-SHA-256.
fn unlock_key(
    password: &str,
    secret: &SyncSecret,
    sync_id: &SyncId,
    salt: &[u8; SALT_LENGTH],
    params: KdfParams,
) -> Result<MasterKey, SyncFileError> {
    let stretched = derive_key(password, salt, params)?;
    let info = [KEK_INFO, sync_id].concat();
    Ok(hkdf_sha256(secret.as_bytes(), &*stretched, &info))
}

/// HKDF (RFC 5869) for one output block of 32 bytes.
fn hkdf_sha256(salt: &[u8], input: &[u8], info: &[u8]) -> MasterKey {
    let pseudo_random_key = Zeroizing::new(hmac_sha256(salt, &[input]));
    Zeroizing::new(hmac_sha256(&*pseudo_random_key, &[info, &[1]]))
}

fn hmac_sha256(key: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC takes keys of any length");
    for part in parts {
        mac.update(part);
    }
    mac.finalize().into_bytes().into()
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use sha2::Digest;

    use super::*;
    use crate::vault::test_support::{sample_vault, CountingRng};

    const PASSWORD: &str = "a long sync passphrase";
    const SYNC_ID: SyncId = [7; SYNC_ID_LENGTH];

    fn secret() -> SyncSecret {
        SyncSecret::from_bytes([9; 16])
    }

    fn master_key() -> MasterKey {
        Zeroizing::new([5; KEY_LENGTH])
    }

    fn header() -> SyncHeader {
        SyncHeader::lock(
            PASSWORD,
            &secret(),
            SYNC_ID,
            &master_key(),
            KdfParams::MINIMUM,
            &mut CountingRng(0),
        )
        .unwrap()
    }

    fn file() -> Vec<u8> {
        header()
            .seal(&master_key(), 3, &sample_vault(), &mut CountingRng(100))
            .unwrap()
    }

    fn flipped(file: &[u8], index: usize) -> Vec<u8> {
        let mut copy = file.to_vec();
        copy[index] ^= 1;
        copy
    }

    fn with_key(file: &[u8]) -> Result<OpenedSyncFile, SyncFileError> {
        open_with_master_key(&SYNC_ID, &master_key(), file)
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().fold(String::new(), |mut text, byte| {
            write!(text, "{byte:02x}").expect("writing to a String cannot fail");
            text
        })
    }

    #[test]
    fn hkdf_matches_the_first_block_of_rfc_5869_test_case_1() {
        let salt: Vec<u8> = (0x00..=0x0c).collect();
        let info: Vec<u8> = (0xf0..=0xf9).collect();
        let output = hkdf_sha256(&salt, &[0x0b; 22], &info);
        assert_eq!(
            hex(&*output),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf"
        );
    }

    #[test]
    fn password_and_secret_together_open_the_file_and_give_the_master_key() {
        let (key, opened) = open_with_password(PASSWORD, &secret(), &file()).unwrap();
        assert_eq!(key, master_key());
        assert_eq!((opened.revision, opened.header), (3, header()));
        assert_eq!(opened.data, sample_vault());
    }

    #[test]
    fn neither_the_password_nor_the_secret_is_enough_alone() {
        let wrong_secret = SyncSecret::from_bytes([8; 16]);
        for (password, secret) in [(PASSWORD, wrong_secret), ("another passphrase", secret())] {
            assert_eq!(
                open_with_password(password, &secret, &file()).unwrap_err(),
                SyncFileError::UnwrapFailed
            );
        }
    }

    #[test]
    fn a_joined_device_needs_only_the_master_key() {
        let opened = with_key(&file()).unwrap();
        assert_eq!((opened.revision, opened.data), (3, sample_vault()));
    }

    #[test]
    fn another_sync_id_is_told_apart_from_damage() {
        let other = open_with_master_key(&[1; 16], &master_key(), &file());
        assert_eq!(other.unwrap_err(), SyncFileError::OtherMasterKey);
        let wrong_key = Zeroizing::new([6; KEY_LENGTH]);
        let damaged = open_with_master_key(&SYNC_ID, &wrong_key, &file());
        assert_eq!(damaged.unwrap_err(), SyncFileError::Damaged);
    }

    #[test]
    fn a_change_anywhere_in_the_header_or_body_is_noticed_by_a_joined_device() {
        let file = file();
        // Cost settings, salt, nonce, wrapped key, body nonce and body.
        for index in [
            31,
            35,
            37,
            52,
            53,
            76,
            77,
            124,
            125,
            148,
            153,
            file.len() - 1,
        ] {
            assert!(with_key(&flipped(&file, index)).is_err(), "byte {index}");
        }
    }

    #[test]
    fn the_password_slot_cannot_be_swapped_for_another_valid_one() {
        let other = SyncHeader::lock(
            "someone else's passphrase",
            &secret(),
            SYNC_ID,
            &master_key(),
            KdfParams::MINIMUM,
            &mut CountingRng(200),
        )
        .unwrap();
        let mut swapped = file();
        swapped[..HEADER_LENGTH].copy_from_slice(&other.0);
        assert_eq!(with_key(&swapped).unwrap_err(), SyncFileError::Damaged);
    }

    #[test]
    fn foreign_newer_truncated_and_costly_files_are_refused_before_any_key_work() {
        let file = file();
        assert_eq!(
            with_key(b"MFAVAULT and more").unwrap_err(),
            SyncFileError::NotASyncFile
        );
        assert_eq!(
            with_key(&flipped(&file, 9)).unwrap_err(),
            SyncFileError::Unsupported
        );
        for length in [8, BODY_START - 1, BODY_START, file.len() - 1] {
            assert_eq!(
                with_key(&file[..length]).unwrap_err(),
                SyncFileError::InvalidHeader,
                "length {length}"
            );
        }
        let mut costly = file;
        costly[28..32].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(matches!(
            with_key(&costly).unwrap_err(),
            SyncFileError::Vault(VaultError::InvalidKdfParams(_))
        ));
    }

    #[test]
    fn the_file_has_the_documented_layout() {
        let file = file();
        assert_eq!(&file[..12], b"MFASYNC\x00\x00\x01\x01\x01");
        assert_eq!(&file[12..28], &SYNC_ID);
        let length = u32::from_be_bytes(file[149..153].try_into().unwrap()) as usize;
        assert_eq!(file.len(), BODY_START + length);
        assert_eq!(length % 1024, TAG_LENGTH);
    }

    /// Pins the bytes: any change to this value is a change of format.
    #[test]
    fn golden_vector() {
        assert_eq!(
            hex(&Sha256::digest(file())),
            "9fc08b9f09d41f3260cea3b45143009070ee0c4fa31091b02d747d683cb83139"
        );
    }
}
