use std::collections::BTreeMap;
use std::fmt;

use ciborium::Value;

use super::rng::{random_array, Rng};
use super::sync_state::SyncState;
use crate::otp::Params;
use crate::secret::Secret;

/// Fields a newer version added that this version does not understand. Kept verbatim
/// so saving never drops them (ADR-0011).
pub type Extra = BTreeMap<String, Value>;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryId([u8; 16]);

impl EntryId {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Reads the form [`EntryId`] displays as: 32 hex digits with hyphens after digits
    /// 8, 12, 16 and 20.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.as_bytes();
        let hyphens = [8, 13, 18, 23];
        if text.len() != 36 || hyphens.iter().any(|&index| text[index] != b'-') {
            return None;
        }
        let digits: Vec<u32> = text
            .iter()
            .enumerate()
            .filter(|(index, _)| !hyphens.contains(index))
            .map(|(_, character)| char::from(*character).to_digit(16))
            .collect::<Option<_>>()?;
        let mut bytes = [0_u8; 16];
        for (byte, pair) in bytes.iter_mut().zip(digits.chunks(2)) {
            *byte = u8::try_from(pair[0] * 16 + pair[1]).ok()?;
        }
        Some(Self(bytes))
    }

    /// A random (version 4) UUID.
    pub fn random(rng: &mut dyn Rng) -> Self {
        let mut bytes = random_array::<16>(rng);
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self(bytes)
    }
}

impl fmt::Display for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, byte) in self.0.iter().enumerate() {
            if matches!(index, 4 | 6 | 8 | 10) {
                f.write_str("-")?;
            }
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EntryId({self})")
    }
}

/// One OTP account (ADR-0004). Timestamps are Unix milliseconds.
#[derive(Debug, PartialEq)]
pub struct Entry {
    pub id: EntryId,
    pub otp_type: String,
    pub issuer: String,
    pub account: String,
    pub secret: Secret,
    pub params: Params,
    pub icon: Option<String>,
    pub tags: Vec<String>,
    pub notes: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub extra: Extra,
}

/// A deletion marker, so deletes survive sync merges (ADR-0004).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tombstone {
    pub id: EntryId,
    pub deleted_at: i64,
}

/// Everything inside the encrypted body. Entry order is display order.
#[derive(Debug, PartialEq)]
pub struct VaultData {
    pub created_at: i64,
    pub updated_at: i64,
    pub entries: Vec<Entry>,
    pub tombstones: Vec<Tombstone>,
    /// This device's sync settings; `None` while sync is off. Never shared.
    pub sync: Option<SyncState>,
    pub extra: Extra,
}

impl VaultData {
    #[must_use]
    pub fn empty(now_ms: i64) -> Self {
        Self {
            created_at: now_ms,
            updated_at: now_ms,
            entries: Vec::new(),
            tombstones: Vec::new(),
            sync: None,
            extra: Extra::new(),
        }
    }

    /// Drops tombstones older than `retention_ms`; younger ones are kept so slow
    /// devices still learn about the deletion when they sync.
    pub fn compact_tombstones(&mut self, now_ms: i64, retention_ms: i64) {
        self.tombstones
            .retain(|tombstone| now_ms.saturating_sub(tombstone.deleted_at) < retention_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(last: u8) -> EntryId {
        let mut bytes = [0_u8; 16];
        bytes[15] = last;
        EntryId::from_bytes(bytes)
    }

    #[test]
    fn entry_id_displays_in_uuid_layout() {
        let id = EntryId::from_bytes([
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88,
        ]);
        assert_eq!(id.to_string(), "12345678-9abc-def0-1122-334455667788");
    }

    #[test]
    fn ids_parse_from_what_they_display_as() {
        let original = EntryId::from_bytes([
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88,
        ]);
        assert_eq!(EntryId::parse(&original.to_string()), Some(original));
        assert_eq!(
            EntryId::parse("12345678-9ABC-DEF0-1122-334455667788"),
            Some(original)
        );
    }

    #[test]
    fn malformed_ids_do_not_parse() {
        for text in [
            "",
            "12345678-9abc-def0-1122-33445566778",
            "12345678-9abc-def0-1122-3344556677889",
            "123456789abc-def0-1122-334455667788-",
            "12345678-9abc-def0-1122-33445566778g",
            "12345678-9abc-def0-1122-33445566778é",
            "+2345678-9abc-def0-1122-334455667788",
        ] {
            assert_eq!(EntryId::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn random_ids_are_version_4_uuids_and_differ() {
        let mut rng = crate::vault::test_support::CountingRng(0xff);
        let first = EntryId::random(&mut rng);
        let second = EntryId::random(&mut rng);
        assert_ne!(first, second);
        let text = first.to_string();
        assert_eq!(text.as_bytes()[14], b'4');
        assert!(matches!(text.as_bytes()[19], b'8' | b'9' | b'a' | b'b'));
    }

    #[test]
    fn compaction_drops_only_expired_tombstones() {
        let mut data = VaultData::empty(0);
        data.tombstones = vec![
            Tombstone {
                id: id(1),
                deleted_at: 100,
            },
            Tombstone {
                id: id(2),
                deleted_at: 900,
            },
            Tombstone {
                id: id(3),
                deleted_at: 1000,
            },
        ];
        data.compact_tombstones(1000, 500);
        let kept: Vec<_> = data.tombstones.iter().map(|t| t.id).collect();
        assert_eq!(kept, [id(2), id(3)]);
    }

    #[test]
    fn debug_output_of_an_entry_hides_the_secret() {
        let entry = Entry {
            id: id(1),
            otp_type: "totp".into(),
            issuer: "Example".into(),
            account: "a@example.com".into(),
            secret: Secret::new(b"topsecretseed".to_vec()),
            params: Params::new(),
            icon: None,
            tags: Vec::new(),
            notes: String::new(),
            created_at: 0,
            updated_at: 0,
            extra: Extra::new(),
        };
        assert!(!format!("{entry:?}").contains("topsecretseed"));
    }
}
