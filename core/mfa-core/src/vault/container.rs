//! The vault file: header, password slot and encrypted body (docs/specs/vault-format-v1.md).

use std::fmt;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::XChaCha20Poly1305;
use zeroize::Zeroizing;

use super::error::VaultError;
use super::kdf::{derive_key, KdfParams, KEY_LENGTH, SALT_LENGTH};
use super::model::VaultData;
use super::payload;
use super::rng::{random_array, random_key, Rng};

const MAGIC: &[u8; 8] = b"MFAVAULT";
const VERSION: u16 = 1;
const KDF_ARGON2ID: u8 = 1;
const AEAD_XCHACHA20_POLY1305: u8 = 1;

const NONCE_LENGTH: usize = 24;
const TAG_LENGTH: usize = 16;
const WRAPPED_KEY_LENGTH: usize = KEY_LENGTH + TAG_LENGTH;

const PREFIX_END: usize = 12;
const WRAP_AAD_END: usize = 61;
const SLOT_END: usize = 109;
const HEADER_END: usize = 137;
const MAX_FILE_LENGTH: usize = 64 * 1024 * 1024;

/// Length of the vault key in bytes.
pub const VAULT_KEY_LENGTH: usize = KEY_LENGTH;

/// An unlocked vault: the vault key plus the password slot it was opened with.
/// Wiped from memory when dropped.
pub struct OpenedVault {
    key: Zeroizing<[u8; KEY_LENGTH]>,
    slot: Slot,
}

impl fmt::Debug for OpenedVault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OpenedVault(<redacted>)")
    }
}

impl OpenedVault {
    /// A copy of the vault key, for wrapping by a key the operating system binds to a
    /// biometric (ADR-0003, ADR-0016). The copy is wiped when dropped.
    #[must_use]
    pub fn key_copy(&self) -> Zeroizing<[u8; VAULT_KEY_LENGTH]> {
        self.key.clone()
    }

    /// Encrypts `data` under a fresh nonce, keeping the password slot unchanged.
    ///
    /// # Errors
    /// If the data cannot be encoded or encryption fails.
    pub fn save(&self, data: &VaultData, rng: &mut dyn Rng) -> Result<Vec<u8>, VaultError> {
        let body = seal_body(&self.key, data, rng)?;
        assemble(&self.slot, &body.nonce, &body.ciphertext)
    }

    /// Replaces the password slot of `file` (the bytes this vault was opened from or last
    /// saved to) with one for `new_password`. The encrypted body is copied unchanged, so
    /// no entry is re-encrypted.
    ///
    /// # Errors
    /// If `file` is not a well-formed vault file, or key derivation or wrapping fails.
    pub fn rewrap(
        &mut self,
        file: &[u8],
        new_password: &str,
        params: KdfParams,
        rng: &mut dyn Rng,
    ) -> Result<Vec<u8>, VaultError> {
        parse(file)?;
        let slot = new_slot(new_password, &self.key, params, rng)?;
        let mut rewrapped = prefix().to_vec();
        rewrapped.extend_from_slice(&slot.to_bytes());
        rewrapped.extend_from_slice(&file[SLOT_END..]);
        self.slot = slot;
        Ok(rewrapped)
    }
}

/// Creates a new vault protected by `password` and returns it with its file bytes.
///
/// # Errors
/// If key derivation or encryption fails.
pub fn create(
    password: &str,
    data: &VaultData,
    params: KdfParams,
    rng: &mut dyn Rng,
) -> Result<(OpenedVault, Vec<u8>), VaultError> {
    let key = random_key(rng);
    let slot = new_slot(password, &key, params, rng)?;
    let vault = OpenedVault { key, slot };
    let file = vault.save(data, rng)?;
    Ok((vault, file))
}

/// Opens a vault file. A wrong password and a tampered header are deliberately
/// indistinguishable ([`VaultError::UnwrapFailed`]).
///
/// # Errors
/// If the file is malformed, uses an unsupported version, algorithm or cost, the
/// password is wrong, or the body is damaged.
pub fn open(password: &str, file: &[u8]) -> Result<(OpenedVault, VaultData), VaultError> {
    let parsed = parse(file)?;
    let kek = derive_key(password, &parsed.slot.salt, parsed.slot.params)?;
    let key_bytes = decrypt(
        &kek,
        &parsed.slot.wrap_nonce,
        &file[..WRAP_AAD_END],
        &parsed.slot.wrapped_key,
    )
    .ok_or(VaultError::UnwrapFailed)?;
    let key = Zeroizing::new(
        <[u8; KEY_LENGTH]>::try_from(key_bytes.as_slice()).map_err(|_| VaultError::UnwrapFailed)?,
    );
    let padded = decrypt(&key, &parsed.body_nonce, &file[..PREFIX_END], parsed.body)
        .ok_or(VaultError::BodyCorrupted)?;
    let data = payload::decode(payload::unpad(&padded)?)?;
    Ok((
        OpenedVault {
            key,
            slot: parsed.slot,
        },
        data,
    ))
}

/// Opens a vault file with the vault key itself instead of the password. A wrong key and a
/// damaged body are indistinguishable ([`VaultError::UnwrapFailed`]).
///
/// # Errors
/// If the file is malformed or unsupported, or the key does not open the body.
pub fn open_with_key(
    key: &[u8; VAULT_KEY_LENGTH],
    file: &[u8],
) -> Result<(OpenedVault, VaultData), VaultError> {
    let parsed = parse(file)?;
    let key = Zeroizing::new(*key);
    let padded = decrypt(&key, &parsed.body_nonce, &file[..PREFIX_END], parsed.body)
        .ok_or(VaultError::UnwrapFailed)?;
    let data = payload::decode(payload::unpad(&padded)?)?;
    Ok((
        OpenedVault {
            key,
            slot: parsed.slot,
        },
        data,
    ))
}

#[derive(Clone)]
struct Slot {
    params: KdfParams,
    salt: [u8; SALT_LENGTH],
    wrap_nonce: [u8; NONCE_LENGTH],
    wrapped_key: [u8; WRAPPED_KEY_LENGTH],
}

impl Slot {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = wrap_aad_tail(self.params, &self.salt, &self.wrap_nonce);
        bytes.extend_from_slice(&self.wrapped_key);
        bytes
    }
}

fn prefix() -> [u8; PREFIX_END] {
    let mut bytes = [0_u8; PREFIX_END];
    bytes[..8].copy_from_slice(MAGIC);
    bytes[8..10].copy_from_slice(&VERSION.to_be_bytes());
    bytes[10] = KDF_ARGON2ID;
    bytes[11] = AEAD_XCHACHA20_POLY1305;
    bytes
}

/// Slot bytes that precede `wrapped_key`: the cost settings, salt and nonce.
fn wrap_aad_tail(
    params: KdfParams,
    salt: &[u8; SALT_LENGTH],
    wrap_nonce: &[u8; NONCE_LENGTH],
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(SLOT_END - PREFIX_END);
    bytes.extend_from_slice(&params.memory_kib().to_be_bytes());
    bytes.extend_from_slice(&params.iterations().to_be_bytes());
    bytes.push(params.lanes());
    bytes.extend_from_slice(salt);
    bytes.extend_from_slice(wrap_nonce);
    bytes
}

fn new_slot(
    password: &str,
    key: &[u8; KEY_LENGTH],
    params: KdfParams,
    rng: &mut dyn Rng,
) -> Result<Slot, VaultError> {
    let salt = random_array::<SALT_LENGTH>(rng);
    let wrap_nonce = random_array::<NONCE_LENGTH>(rng);
    let kek = derive_key(password, &salt, params)?;
    let mut aad = prefix().to_vec();
    aad.extend_from_slice(&wrap_aad_tail(params, &salt, &wrap_nonce));
    let wrapped_key = encrypt(&kek, &wrap_nonce, &aad, key)?
        .try_into()
        .map_err(|_| VaultError::EncryptionFailed)?;
    Ok(Slot {
        params,
        salt,
        wrap_nonce,
        wrapped_key,
    })
}

struct SealedBody {
    nonce: [u8; NONCE_LENGTH],
    ciphertext: Vec<u8>,
}

fn seal_body(
    key: &[u8; KEY_LENGTH],
    data: &VaultData,
    rng: &mut dyn Rng,
) -> Result<SealedBody, VaultError> {
    let encoded = payload::encode(data)?;
    let padded = payload::pad(&encoded);
    let nonce = random_array::<NONCE_LENGTH>(rng);
    let ciphertext = encrypt(key, &nonce, &prefix(), &padded)?;
    Ok(SealedBody { nonce, ciphertext })
}

fn assemble(
    slot: &Slot,
    body_nonce: &[u8; NONCE_LENGTH],
    body: &[u8],
) -> Result<Vec<u8>, VaultError> {
    let body_length =
        u32::try_from(body.len()).map_err(|_| VaultError::InvalidHeader("body too large"))?;
    let mut file = Vec::with_capacity(HEADER_END + body.len());
    file.extend_from_slice(&prefix());
    file.extend_from_slice(&slot.to_bytes());
    file.extend_from_slice(body_nonce);
    file.extend_from_slice(&body_length.to_be_bytes());
    file.extend_from_slice(body);
    Ok(file)
}

struct Parsed<'a> {
    slot: Slot,
    body_nonce: [u8; NONCE_LENGTH],
    body: &'a [u8],
}

pub(super) fn array_at<const N: usize>(file: &[u8], offset: usize) -> [u8; N] {
    file[offset..offset + N]
        .try_into()
        .expect("offsets are checked against HEADER_END before parsing")
}

fn parse(file: &[u8]) -> Result<Parsed<'_>, VaultError> {
    if file.len() > MAX_FILE_LENGTH {
        return Err(VaultError::InvalidHeader("file too large"));
    }
    if !file.starts_with(MAGIC) {
        return Err(VaultError::NotAVault);
    }
    if file.len() < HEADER_END {
        return Err(VaultError::InvalidHeader("truncated header"));
    }
    let version = u16::from_be_bytes(array_at(file, 8));
    if version != VERSION {
        return Err(VaultError::UnsupportedVersion(version));
    }
    if file[10] != KDF_ARGON2ID {
        return Err(VaultError::UnsupportedAlgorithm(file[10]));
    }
    if file[11] != AEAD_XCHACHA20_POLY1305 {
        return Err(VaultError::UnsupportedAlgorithm(file[11]));
    }
    let params = KdfParams::new(
        u32::from_be_bytes(array_at(file, 12)),
        u32::from_be_bytes(array_at(file, 16)),
        file[20],
    )?;
    let body_length = usize::try_from(u32::from_be_bytes(array_at(file, 133)))
        .map_err(|_| VaultError::InvalidHeader("body length mismatch"))?;
    if HEADER_END + body_length != file.len() {
        return Err(VaultError::InvalidHeader("body length mismatch"));
    }
    Ok(Parsed {
        slot: Slot {
            params,
            salt: array_at(file, 21),
            wrap_nonce: array_at(file, 37),
            wrapped_key: array_at(file, 61),
        },
        body_nonce: array_at(file, 109),
        body: &file[HEADER_END..],
    })
}

pub(super) fn encrypt(
    key: &[u8; KEY_LENGTH],
    nonce: &[u8; NONCE_LENGTH],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, VaultError> {
    XChaCha20Poly1305::new(key.into())
        .encrypt(
            nonce.into(),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| VaultError::EncryptionFailed)
}

pub(super) fn decrypt(
    key: &[u8; KEY_LENGTH],
    nonce: &[u8; NONCE_LENGTH],
    aad: &[u8],
    ciphertext: &[u8],
) -> Option<Zeroizing<Vec<u8>>> {
    XChaCha20Poly1305::new(key.into())
        .decrypt(
            nonce.into(),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .ok()
        .map(Zeroizing::new)
}

#[cfg(test)]
mod tests {
    use super::super::model::Entry;
    use super::super::test_support::{id, sample_vault, CountingRng};
    use super::*;
    use crate::otp::Params;
    use crate::secret::Secret;

    const PASSWORD: &str = "correct horse battery staple";

    fn cheap() -> KdfParams {
        KdfParams::MINIMUM
    }

    fn created() -> (OpenedVault, Vec<u8>) {
        create(PASSWORD, &sample_vault(), cheap(), &mut CountingRng(0)).unwrap()
    }

    fn flipped(file: &[u8], index: usize) -> Vec<u8> {
        let mut copy = file.to_vec();
        copy[index] ^= 0x01;
        copy
    }

    fn patched(file: &[u8], offset: usize, bytes: &[u8]) -> Vec<u8> {
        let mut copy = file.to_vec();
        copy[offset..offset + bytes.len()].copy_from_slice(bytes);
        copy
    }

    fn open_error(password: &str, file: &[u8]) -> VaultError {
        open(password, file).unwrap_err()
    }

    #[test]
    fn a_created_vault_opens_with_the_same_password() {
        let (_, file) = created();
        let (_, data) = open(PASSWORD, &file).unwrap();
        assert_eq!(data, sample_vault());
    }

    #[test]
    fn the_file_starts_with_the_documented_prefix_and_has_the_documented_size() {
        let (_, file) = created();
        assert_eq!(&file[..12], b"MFAVAULT\x00\x01\x01\x01");
        let body_length = u32::from_be_bytes(file[133..137].try_into().unwrap()) as usize;
        assert_eq!(file.len(), 137 + body_length);
        assert_eq!(body_length % 1024, TAG_LENGTH);
    }

    #[test]
    fn a_wrong_password_cannot_open_the_vault() {
        let (_, file) = created();
        assert_eq!(open_error("wrong", &file), VaultError::UnwrapFailed);
    }

    #[test]
    fn differently_normalised_spellings_of_a_password_are_the_same_password() {
        let composed = "caf\u{e9} \u{fb01}sh";
        let decomposed = "cafe\u{301} fish";
        let (_, file) = create(composed, &sample_vault(), cheap(), &mut CountingRng(0)).unwrap();
        assert!(open(decomposed, &file).is_ok());
    }

    #[test]
    fn identical_inputs_and_randomness_give_identical_files() {
        let first = created().1;
        let second = created().1;
        assert_eq!(first, second);
    }

    #[test]
    fn tampering_with_any_part_of_the_password_slot_prevents_unlocking() {
        let (_, file) = created();
        for index in [12, 15, 16, 19, 20, 21, 36, 37, 60, 61, 84, 108] {
            assert!(
                open(PASSWORD, &flipped(&file, index)).is_err(),
                "byte {index}"
            );
        }
    }

    #[test]
    fn lowering_the_cost_settings_is_detected() {
        let (_, file) = created();
        let downgraded = patched(&file, 16, &3_u32.to_be_bytes());
        assert_eq!(open_error(PASSWORD, &downgraded), VaultError::UnwrapFailed);
    }

    #[test]
    fn the_fixed_prefix_is_checked() {
        let (_, file) = created();
        assert_eq!(
            open_error(PASSWORD, &flipped(&file, 0)),
            VaultError::NotAVault
        );
        assert_eq!(
            open_error(PASSWORD, &patched(&file, 8, &2_u16.to_be_bytes())),
            VaultError::UnsupportedVersion(2)
        );
        assert_eq!(
            open_error(PASSWORD, &patched(&file, 10, &[9])),
            VaultError::UnsupportedAlgorithm(9)
        );
        assert_eq!(
            open_error(PASSWORD, &patched(&file, 11, &[9])),
            VaultError::UnsupportedAlgorithm(9)
        );
    }

    #[test]
    fn tampering_with_the_body_is_detected() {
        let (_, file) = created();
        let last = file.len() - 1;
        assert_eq!(
            open_error(PASSWORD, &flipped(&file, last)),
            VaultError::BodyCorrupted
        );
        assert_eq!(
            open_error(PASSWORD, &flipped(&file, 109)),
            VaultError::BodyCorrupted
        );
        assert_eq!(
            open_error(PASSWORD, &flipped(&file, 140)),
            VaultError::BodyCorrupted
        );
    }

    #[test]
    fn malformed_files_are_refused() {
        let (_, file) = created();
        assert_eq!(open_error(PASSWORD, &[]), VaultError::NotAVault);
        assert_eq!(
            open_error(PASSWORD, b"not a vault at all, just text"),
            VaultError::NotAVault
        );
        assert_eq!(
            open_error(PASSWORD, &file[..50]),
            VaultError::InvalidHeader("truncated header")
        );
        assert_eq!(
            open_error(PASSWORD, &file[..file.len() - 1]),
            VaultError::InvalidHeader("body length mismatch")
        );
        let mut extended = file.clone();
        extended.push(0);
        assert_eq!(
            open_error(PASSWORD, &extended),
            VaultError::InvalidHeader("body length mismatch")
        );
    }

    #[test]
    fn hostile_cost_settings_are_rejected_before_any_derivation() {
        let (_, file) = created();
        let cases = [
            (12, u32::MAX.to_be_bytes().to_vec()),
            (12, (KdfParams::MAX_MEMORY_KIB + 1).to_be_bytes().to_vec()),
            (12, 1024_u32.to_be_bytes().to_vec()),
            (16, 21_u32.to_be_bytes().to_vec()),
            (16, 1_u32.to_be_bytes().to_vec()),
            (20, vec![17]),
            (20, vec![0]),
        ];
        for (offset, bytes) in cases {
            let started = std::time::Instant::now();
            let error = open_error(PASSWORD, &patched(&file, offset, &bytes));
            assert!(
                matches!(error, VaultError::InvalidKdfParams(_)),
                "{offset} {bytes:?}"
            );
            assert!(
                started.elapsed().as_millis() < 50,
                "rejection must be immediate"
            );
        }
    }

    #[test]
    fn saving_keeps_the_password_slot_and_uses_a_fresh_body_nonce() {
        let (vault, file) = created();
        let mut data = sample_vault();
        data.entries.truncate(1);
        let saved = vault.save(&data, &mut CountingRng(200)).unwrap();
        assert_eq!(saved[..109], file[..109]);
        assert_ne!(saved[109..133], file[109..133]);
        let (_, reopened) = open(PASSWORD, &saved).unwrap();
        assert_eq!(reopened, data);
    }

    #[test]
    fn changing_the_password_replaces_only_the_slot() {
        let (mut vault, file) = created();
        let rewrapped = vault
            .rewrap(
                &file,
                "a brand new password",
                cheap(),
                &mut CountingRng(100),
            )
            .unwrap();
        assert_eq!(rewrapped[..12], file[..12]);
        assert_ne!(rewrapped[12..109], file[12..109]);
        assert_eq!(
            rewrapped[109..],
            file[109..],
            "the encrypted body must not change"
        );
        assert_eq!(open_error(PASSWORD, &rewrapped), VaultError::UnwrapFailed);
        let (_, data) = open("a brand new password", &rewrapped).unwrap();
        assert_eq!(data, sample_vault());
    }

    #[test]
    fn after_a_password_change_later_saves_use_the_new_password() {
        let (mut vault, file) = created();
        vault
            .rewrap(&file, "another one", cheap(), &mut CountingRng(100))
            .unwrap();
        let saved = vault.save(&sample_vault(), &mut CountingRng(50)).unwrap();
        assert!(open("another one", &saved).is_ok());
        assert_eq!(open_error(PASSWORD, &saved), VaultError::UnwrapFailed);
    }

    #[test]
    fn rewrapping_refuses_a_malformed_file() {
        let (mut vault, _) = created();
        assert!(vault
            .rewrap(b"junk", "x", cheap(), &mut CountingRng(0))
            .is_err());
    }

    #[test]
    fn small_vaults_have_the_same_file_size_whatever_their_entry_count() {
        let empty = create(PASSWORD, &VaultData::empty(0), cheap(), &mut CountingRng(0))
            .unwrap()
            .1;
        let mut one = VaultData::empty(0);
        one.entries.push(Entry {
            id: id(1),
            otp_type: "totp".into(),
            issuer: "A".into(),
            account: "b".into(),
            secret: Secret::new(vec![1; 20]),
            params: Params::new(),
            icon: None,
            tags: Vec::new(),
            notes: String::new(),
            created_at: 0,
            updated_at: 0,
            extra: super::super::model::Extra::new(),
        });
        let one = create(PASSWORD, &one, cheap(), &mut CountingRng(0))
            .unwrap()
            .1;
        assert_eq!(empty.len(), one.len());
    }

    #[test]
    fn the_unlocked_vault_does_not_print_its_key() {
        let (vault, _) = created();
        assert_eq!(format!("{vault:?}"), "OpenedVault(<redacted>)");
    }

    /// Format v1 pinned byte for byte. Inputs: `PASSWORD`, `KdfParams::MINIMUM`, the
    /// `sample_vault()` fixture and `CountingRng(0)`. If this fails, the on-disk format
    /// changed: that needs a new format version and an ADR, not an updated fixture.
    const GOLDEN: &[u8] = include_bytes!("../../tests/golden/vault-v1.bin");

    #[test]
    fn writing_reproduces_the_golden_file_byte_for_byte() {
        assert_eq!(created().1, GOLDEN);
    }

    #[test]
    fn the_golden_file_still_opens_and_holds_the_sample_vault() {
        let (_, data) = open(PASSWORD, GOLDEN).unwrap();
        assert_eq!(data, sample_vault());
    }

    #[test]
    fn the_vault_key_opens_the_vault_like_the_password_does() {
        let (vault, file) = created();
        let key = vault.key_copy();
        let (reopened, data) = open_with_key(&key, &file).unwrap();
        assert_eq!(data, sample_vault());
        assert_eq!(*reopened.key_copy(), *key);
    }

    #[test]
    fn a_wrong_key_is_refused_like_a_wrong_password() {
        let (_, file) = created();
        let wrong = [7_u8; VAULT_KEY_LENGTH];
        assert_eq!(
            open_with_key(&wrong, &file).unwrap_err(),
            VaultError::UnwrapFailed
        );
    }

    #[test]
    fn opening_with_the_key_survives_a_password_change() {
        let (mut vault, file) = created();
        let key = vault.key_copy();
        let rewrapped = vault
            .rewrap(&file, "another password", cheap(), &mut CountingRng(9))
            .unwrap();
        assert!(open_with_key(&key, &rewrapped).is_ok());
        assert!(open("another password", &rewrapped).is_ok());
    }

    #[test]
    fn a_malformed_file_is_refused_when_opening_with_the_key() {
        let key = [0_u8; VAULT_KEY_LENGTH];
        assert!(open_with_key(&key, b"not a vault").is_err());
    }
}
