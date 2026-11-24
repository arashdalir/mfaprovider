use ciborium::Value;

use super::model::{Entry, EntryId, Extra, Tombstone, VaultData};
use crate::otp::Params;
use crate::secret::Secret;

pub(crate) fn id(last: u8) -> EntryId {
    let mut bytes = [0xa0_u8; 16];
    bytes[15] = last;
    EntryId::from_bytes(bytes)
}

/// Two entries (one with an icon and unknown fields), one tombstone, one unknown top-level field.
pub(crate) fn sample_vault() -> VaultData {
    let mut entry_extra = Extra::new();
    entry_extra.insert("future_flag".into(), Value::Bool(true));
    entry_extra.insert("future_blob".into(), Value::Bytes(vec![9, 8, 7]));
    let mut vault_extra = Extra::new();
    vault_extra.insert("future_vault_field".into(), Value::Text("kept".into()));
    VaultData {
        created_at: 1_700_000_000_000,
        updated_at: 1_700_000_500_000,
        entries: vec![
            Entry {
                id: id(1),
                otp_type: "totp".into(),
                issuer: "Example".into(),
                account: "alice@example.com".into(),
                secret: Secret::new(b"12345678901234567890".to_vec()),
                params: Params::new().with("digits", "8").with("period", "60"),
                icon: Some("example".into()),
                tags: vec!["work".into(), "mail".into()],
                notes: "recovery codes in the safe".into(),
                created_at: 1_700_000_000_000,
                updated_at: 1_700_000_100_000,
                extra: entry_extra,
            },
            Entry {
                id: id(2),
                otp_type: "vendor-x".into(),
                issuer: "Vendor".into(),
                account: "bob".into(),
                secret: Secret::new(vec![0xde, 0xad, 0xbe, 0xef]),
                params: Params::new(),
                icon: None,
                tags: Vec::new(),
                notes: String::new(),
                created_at: 1_700_000_200_000,
                updated_at: 1_700_000_200_000,
                extra: Extra::new(),
            },
        ],
        tombstones: vec![Tombstone {
            id: id(3),
            deleted_at: 1_700_000_400_000,
        }],
        extra: vault_extra,
    }
}

use super::rng::Rng;

/// Deterministic "randomness": every requested byte is the next value of a counter.
pub(crate) struct CountingRng(pub(crate) u8);

impl Rng for CountingRng {
    fn fill_bytes(&mut self, destination: &mut [u8]) {
        for byte in destination {
            *byte = self.0;
            self.0 = self.0.wrapping_add(1);
        }
    }
}
