//! The sync secret (ADR-0018): the second thing, besides the sync password, that a device
//! needs to join. It is never stored in the sync file. Text form: sync-format-v1.md.

use std::fmt;

use sha2::{Digest, Sha256};
use thiserror::Error;
use zeroize::Zeroizing;

use super::rng::{random_array, Rng};
use crate::secret::Secret;

pub const SYNC_SECRET_LENGTH: usize = 16;

const QR_PREFIX: &str = "mfaprovider-sync:v1:";
const DATA_CHARS: usize = 26;
const TEXT_CHARS: usize = DATA_CHARS + 2;
const GROUP: usize = 4;
const BASE32_ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

#[derive(Clone, PartialEq, Eq)]
pub struct SyncSecret(Zeroizing<[u8; SYNC_SECRET_LENGTH]>);

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SyncSecretError {
    #[error("a sync secret has {TEXT_CHARS} letters and digits")]
    Length,
    #[error("the sync secret contains a character that cannot be part of it")]
    Character,
    #[error("the sync secret was mistyped")]
    Checksum,
}

impl fmt::Debug for SyncSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SyncSecret(<redacted>)")
    }
}

impl SyncSecret {
    #[must_use]
    pub fn from_bytes(bytes: [u8; SYNC_SECRET_LENGTH]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    pub fn random(rng: &mut dyn Rng) -> Self {
        Self::from_bytes(random_array(rng))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8; SYNC_SECRET_LENGTH] {
        &self.0
    }

    /// Seven groups of four, for the recovery sheet.
    #[must_use]
    pub fn to_text(&self) -> Zeroizing<String> {
        let compact = self.compact();
        let mut text = Zeroizing::new(String::with_capacity(TEXT_CHARS + TEXT_CHARS / GROUP));
        for (index, character) in compact.chars().enumerate() {
            if index > 0 && index % GROUP == 0 {
                text.push('-');
            }
            text.push(character);
        }
        text
    }

    /// What the QR code holds.
    #[must_use]
    pub fn to_qr_payload(&self) -> Zeroizing<String> {
        let mut payload = Zeroizing::new(String::with_capacity(QR_PREFIX.len() + TEXT_CHARS));
        payload.push_str(QR_PREFIX);
        payload.push_str(&self.compact());
        payload
    }

    /// Reads the typed text or the content of the QR code; case, spaces and hyphens in the
    /// typed form do not matter.
    ///
    /// # Errors
    /// If it is not a sync secret, or its check characters do not match.
    pub fn parse(input: &str) -> Result<Self, SyncSecretError> {
        let input = input.trim();
        let input = input.strip_prefix(QR_PREFIX).unwrap_or(input);
        let mut compact = Zeroizing::new(String::with_capacity(TEXT_CHARS));
        for character in input.chars().filter(|c| !c.is_whitespace() && *c != '-') {
            let upper = character.to_ascii_uppercase();
            if !upper.is_ascii() || !BASE32_ALPHABET.contains(&(upper as u8)) {
                return Err(SyncSecretError::Character);
            }
            compact.push(upper);
        }
        if compact.len() != TEXT_CHARS {
            return Err(SyncSecretError::Length);
        }
        let decoded =
            Secret::from_base32(&compact[..DATA_CHARS]).map_err(|_| SyncSecretError::Character)?;
        let bytes = <[u8; SYNC_SECRET_LENGTH]>::try_from(decoded.as_bytes())
            .map_err(|_| SyncSecretError::Length)?;
        let secret = Self::from_bytes(bytes);
        // Comparing whole texts also rejects a last data character with stray bits.
        if *secret.compact() == *compact {
            Ok(secret)
        } else {
            Err(SyncSecretError::Checksum)
        }
    }

    fn compact(&self) -> Zeroizing<String> {
        let data = Secret::new(self.0.to_vec()).to_base32();
        let digest = Sha256::digest(*self.0);
        let check = u16::from_be_bytes([digest[0], digest[1]]) >> 6;
        let mut compact = Zeroizing::new(String::with_capacity(TEXT_CHARS));
        compact.push_str(&data);
        for value in [check >> 5, check & 31] {
            compact.push(char::from(BASE32_ALPHABET[usize::from(value)]));
        }
        compact
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SyncSecret {
        SyncSecret::from_bytes(*b"0123456789abcdef")
    }

    #[test]
    fn the_text_is_seven_groups_of_four_and_reads_back() {
        let text = sample().to_text();
        let groups: Vec<_> = text.split('-').collect();
        assert_eq!(groups.len(), 7);
        assert!(groups.iter().all(|group| group.len() == 4));
        assert_eq!(SyncSecret::parse(&text), Ok(sample()));
    }

    #[test]
    fn the_data_part_is_plain_base32_of_the_bytes() {
        let text = sample().to_text().replace('-', "");
        assert_eq!(&text[..26], "GAYTEMZUGU3DOOBZMFRGGZDFMY");
    }

    #[test]
    fn the_qr_code_content_reads_back() {
        let payload = sample().to_qr_payload();
        assert!(payload.starts_with("mfaprovider-sync:v1:"));
        assert_eq!(SyncSecret::parse(&payload), Ok(sample()));
    }

    #[test]
    fn typing_style_does_not_matter() {
        let sloppy = format!("  {} ", sample().to_text().to_lowercase().replace('-', " "));
        assert_eq!(SyncSecret::parse(&sloppy), Ok(sample()));
    }

    #[test]
    // Ten check bits let about one typo in a thousand through; such a secret then simply
    // fails to open the file.
    fn single_typos_in_the_sample_are_noticed() {
        let text = sample().to_text().replace('-', "");
        let mut missed = 0;
        for position in 0..text.len() {
            let mut typo: Vec<char> = text.chars().collect();
            typo[position] = if typo[position] == 'A' { 'B' } else { 'A' };
            let typo: String = typo.into_iter().collect();
            missed += usize::from(SyncSecret::parse(&typo).is_ok());
        }
        assert_eq!(missed, 0);
    }

    #[test]
    fn wrong_lengths_and_characters_are_named() {
        assert_eq!(SyncSecret::parse(""), Err(SyncSecretError::Length));
        assert_eq!(SyncSecret::parse("ABCD"), Err(SyncSecretError::Length));
        let with_one = format!("1{}", &sample().to_text()[1..]);
        assert_eq!(
            SyncSecret::parse(&with_one),
            Err(SyncSecretError::Character)
        );
        assert_eq!(SyncSecret::parse("é"), Err(SyncSecretError::Character));
    }

    #[test]
    fn random_secrets_differ_and_debug_output_hides_them() {
        let mut rng = crate::vault::test_support::CountingRng(1);
        assert_ne!(SyncSecret::random(&mut rng), SyncSecret::random(&mut rng));
        assert_eq!(format!("{:?}", sample()), "SyncSecret(<redacted>)");
    }
}
