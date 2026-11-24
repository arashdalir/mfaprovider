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

use std::cell::Cell;
use std::rc::Rc;

use super::clock::Clock;

/// Test clock whose handles share one time; `advance` moves wall-clock and monotonic time together.
#[derive(Clone)]
pub(crate) struct FakeClock(Rc<Cell<u64>>);

impl FakeClock {
    pub(crate) const START_UNIX_MS: u64 = 1_700_000_000_000;

    pub(crate) fn new() -> Self {
        Self(Rc::new(Cell::new(0)))
    }

    pub(crate) fn advance_ms(&self, milliseconds: u64) {
        self.0.set(self.0.get() + milliseconds);
    }

    pub(crate) fn advance_secs(&self, seconds: u64) {
        self.advance_ms(seconds * 1000);
    }
}

impl Clock for FakeClock {
    fn unix_ms(&self) -> i64 {
        i64::try_from(Self::START_UNIX_MS + self.0.get()).expect("fits in i64")
    }

    fn monotonic_ms(&self) -> u64 {
        self.0.get()
    }
}

/// A small TOTP entry whose id and account derive from `number`.
pub(crate) fn entry(number: u8) -> Entry {
    Entry {
        id: id(number),
        otp_type: "totp".into(),
        issuer: "Example".into(),
        account: format!("user{number}@example.com"),
        secret: Secret::new(vec![number; 20]),
        params: Params::new(),
        icon: None,
        tags: Vec::new(),
        notes: String::new(),
        created_at: 0,
        updated_at: 0,
        extra: Extra::new(),
    }
}
