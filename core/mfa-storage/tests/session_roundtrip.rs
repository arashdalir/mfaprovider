//! A real vault session over a real file, using only the public API of both crates.

use std::cell::Cell;
use std::rc::Rc;

use mfa_core::otp::Params;
use mfa_core::secret::Secret;
use mfa_core::vault::{
    Clock, Entry, EntryId, Extra, KdfParams, LockPolicy, Rng, SessionError, VaultError,
    VaultSession,
};
use mfa_storage::FileStorage;

const PASSWORD: &str = "correct horse battery staple";

/// Deterministic stand-in for OS randomness; the production RNG lives in the app shell.
struct CountingRng(u8);

impl Rng for CountingRng {
    fn fill_bytes(&mut self, destination: &mut [u8]) {
        for byte in destination {
            *byte = self.0;
            self.0 = self.0.wrapping_add(1);
        }
    }
}

#[derive(Clone)]
struct TestClock(Rc<Cell<u64>>);

impl Clock for TestClock {
    fn unix_ms(&self) -> i64 {
        1_700_000_000_000 + i64::try_from(self.0.get()).unwrap()
    }

    fn monotonic_ms(&self) -> u64 {
        self.0.get()
    }
}

fn session(path: &std::path::Path, seed: u8) -> VaultSession<FileStorage, TestClock, CountingRng> {
    VaultSession::new(
        FileStorage::new(path),
        TestClock(Rc::new(Cell::new(0))),
        CountingRng(seed),
        LockPolicy::default(),
    )
}

fn entry(number: u8) -> Entry {
    Entry {
        id: EntryId::from_bytes([number; 16]),
        otp_type: "totp".into(),
        issuer: "Example".into(),
        account: format!("user{number}"),
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

#[test]
fn a_vault_survives_being_closed_and_reopened_from_disk() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("vault.mfa");

    let mut first = session(&path, 0);
    first.create(PASSWORD, KdfParams::MINIMUM).unwrap();
    first.edit(|data| data.entries.push(entry(1))).unwrap();
    first.edit(|data| data.entries.push(entry(2))).unwrap();
    drop(first);

    let mut second = session(&path, 100);
    second.unlock(PASSWORD).unwrap();
    let accounts: Vec<_> = second
        .data()
        .unwrap()
        .entries
        .iter()
        .map(|entry| entry.account.clone())
        .collect();
    assert_eq!(accounts, ["user1", "user2"]);
}

#[test]
fn the_file_on_disk_never_contains_the_seed_or_account_names() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("vault.mfa");
    let mut session = session(&path, 0);
    session.create(PASSWORD, KdfParams::MINIMUM).unwrap();
    session
        .edit(|data| {
            let mut entry = entry(9);
            entry.secret = Secret::new(b"PLAINTEXT-SEED-0123456789".to_vec());
            entry.account = "plaintext-account-name".into();
            data.entries.push(entry);
        })
        .unwrap();

    let on_disk = std::fs::read(&path).unwrap();
    let contains = |needle: &[u8]| on_disk.windows(needle.len()).any(|window| window == needle);
    assert!(!contains(b"PLAINTEXT-SEED-0123456789"));
    assert!(!contains(b"plaintext-account-name"));
    assert!(!contains(PASSWORD.as_bytes()));
}

#[test]
fn a_password_change_persists_on_disk() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("vault.mfa");
    let mut first = session(&path, 0);
    first.create(PASSWORD, KdfParams::MINIMUM).unwrap();
    first.edit(|data| data.entries.push(entry(1))).unwrap();
    first
        .change_password(PASSWORD, "a different password", KdfParams::MINIMUM)
        .unwrap();
    drop(first);

    let mut second = session(&path, 100);
    assert_eq!(
        second.unlock(PASSWORD),
        Err(SessionError::Vault(VaultError::UnwrapFailed))
    );
    second.unlock("a different password").unwrap();
    assert_eq!(second.data().unwrap().entries.len(), 1);
}

#[test]
fn creating_over_an_existing_file_is_refused_and_leaves_it_untouched() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("vault.mfa");
    let mut first = session(&path, 0);
    first.create(PASSWORD, KdfParams::MINIMUM).unwrap();
    let before = std::fs::read(&path).unwrap();

    let mut second = session(&path, 100);
    assert_eq!(
        second.create("another", KdfParams::MINIMUM),
        Err(SessionError::AlreadyExists)
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
