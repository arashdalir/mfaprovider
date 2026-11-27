//! Devices that share a "cloud" file.

use std::time::Duration;

use super::*;
use crate::otp::{OtpRegistry, Params};
use crate::secret::Secret;
use crate::vault::entries::{EntryChanges, NewEntry};
use crate::vault::error::VaultError;
use crate::vault::session::LockPolicy;
use crate::vault::storage::MemoryStorage;
use crate::vault::test_support::{CountingRng, FakeClock};

const SYNC_PASSWORD: &str = "a long sync passphrase";
const HERE: &str = "cloud/vault.mfasync";

struct Device {
    session: VaultSession<MemoryStorage, FakeClock, CountingRng>,
    storage: MemoryStorage,
    password: String,
}

fn kdf() -> KdfParams {
    KdfParams::MINIMUM
}

/// Each device draws different "randomness" and has its own device password.
fn device(number: u8, clock: &FakeClock) -> Device {
    let storage = MemoryStorage::new();
    let policy = LockPolicy {
        idle_timeout: Some(Duration::from_secs(300)),
        ..LockPolicy::default()
    };
    let mut session = VaultSession::new(
        storage.clone(),
        clock.clone(),
        CountingRng(number.wrapping_mul(61)),
        policy,
    );
    let password = format!("device password {number}");
    session.create(&password, kdf()).unwrap();
    Device {
        session,
        storage,
        password,
    }
}

fn account(name: &str) -> NewEntry {
    NewEntry {
        otp_type: "totp".into(),
        issuer: "Example".into(),
        account: name.into(),
        secret: Secret::new(name.repeat(10).into_bytes()),
        params: Params::new(),
        icon: None,
        tags: Vec::new(),
        notes: String::new(),
    }
}

impl Device {
    fn add(&mut self, name: &str) {
        self.session
            .add_entry(account(name), &OtpRegistry::with_builtins())
            .unwrap();
    }

    fn names(&mut self) -> Vec<String> {
        let data = self.session.data().unwrap();
        let mut names: Vec<String> = data.entries.iter().map(|e| e.account.clone()).collect();
        names.sort();
        names
    }

    fn id_of(&mut self, name: &str) -> EntryId {
        let data = self.session.data().unwrap();
        data.entries.iter().find(|e| e.account == name).unwrap().id
    }

    /// Turns sync on and returns the new file and the secret.
    fn turn_on(&mut self, location: &str) -> (Vec<u8>, SyncSecret) {
        let pending = self
            .session
            .prepare_new_sync(SYNC_PASSWORD, location, kdf())
            .unwrap();
        let (file, secret) = (pending.file().to_vec(), pending.sync_secret().clone());
        self.session.commit_new_sync(pending).unwrap();
        (file, secret)
    }

    fn join(&mut self, cloud: &mut Vec<u8>, secret: &SyncSecret) {
        let pending = self
            .session
            .prepare_join(cloud, secret.clone(), SYNC_PASSWORD, HERE)
            .unwrap();
        let step = self.session.commit_join(pending).unwrap();
        self.write(cloud, step.upload);
    }

    /// One round including the write; returns whether anything was written.
    fn sync(&mut self, cloud: &mut Vec<u8>) -> (bool, SyncStep) {
        let step = self.session.sync(cloud).unwrap();
        let summary = SyncStep {
            upload: None,
            ..step
        };
        let wrote = step.upload.is_some();
        self.write(cloud, step.upload);
        (wrote, summary)
    }

    fn write(&mut self, cloud: &mut Vec<u8>, upload: Option<Upload>) {
        if let Some(upload) = upload {
            cloud.clone_from(&upload.file);
            self.session.record_upload(upload.revision).unwrap();
        }
    }

    fn entries_in(&mut self, file: &[u8]) -> usize {
        let sync = self.session.data().unwrap().sync.as_ref().unwrap();
        let opened = open_with_master_key(&sync.sync_id, &sync.master_key, file).unwrap();
        opened.data.entries.len()
    }
}

struct Pair {
    first: Device,
    second: Device,
    cloud: Vec<u8>,
    secret: SyncSecret,
    clock: FakeClock,
}

/// Device 1 had "a" and turned sync on; device 2 joined with "b" of its own.
fn pair() -> Pair {
    let clock = FakeClock::new();
    let (mut first, mut second) = (device(1, &clock), device(2, &clock));
    first.add("a");
    let (mut cloud, secret) = first.turn_on(HERE);
    clock.advance_secs(1);
    second.add("b");
    second.join(&mut cloud, &secret);
    first.sync(&mut cloud);
    Pair {
        first,
        second,
        cloud,
        secret,
        clock,
    }
}

#[test]
fn turning_sync_on_changes_nothing_until_it_is_committed() {
    let mut first = device(1, &FakeClock::new());
    let before = first.storage.bytes();
    let pending = first
        .session
        .prepare_new_sync(SYNC_PASSWORD, HERE, kdf())
        .unwrap();
    assert_eq!(first.storage.bytes(), before);
    assert_eq!(first.session.sync_status(), Ok(None));
    assert!(open_with_password(SYNC_PASSWORD, pending.sync_secret(), pending.file()).is_ok());
    first.session.commit_new_sync(pending).unwrap();
    let expected = SyncStatus {
        location: HERE.into(),
        highest_revision: 1,
    };
    assert_eq!(first.session.sync_status(), Ok(Some(expected)));
}

#[test]
fn a_sync_password_needs_twelve_characters() {
    let mut first = device(1, &FakeClock::new());
    let short = first.session.prepare_new_sync("elevenchars", HERE, kdf());
    assert_eq!(short.unwrap_err(), SyncError::PasswordTooShort);
    let enough = first.session.prepare_new_sync("twelve chars", HERE, kdf());
    assert!(enough.is_ok());
}

#[test]
fn after_joining_both_devices_hold_both_accounts() {
    let mut pair = pair();
    assert_eq!(pair.first.names(), ["a", "b"]);
    assert_eq!(pair.second.names(), ["a", "b"]);
}

#[test]
fn joining_keeps_the_device_password_and_the_settings_survive_a_lock() {
    let mut pair = pair();
    pair.second.session.lock();
    pair.second.session.unlock(&pair.second.password).unwrap();
    assert!(pair.second.session.sync_status().unwrap().is_some());
    assert!(!pair.second.sync(&mut pair.cloud).0);
}

#[test]
fn joining_needs_the_right_password_and_secret_and_changes_nothing_otherwise() {
    let clock = FakeClock::new();
    let (mut first, mut second) = (device(1, &clock), device(2, &clock));
    let (cloud, secret) = first.turn_on(HERE);
    let before = second.storage.bytes();
    for (password, secret) in [
        ("not the sync passphrase", secret),
        (SYNC_PASSWORD, SyncSecret::from_bytes([1; 16])),
    ] {
        let refused = second.session.prepare_join(&cloud, secret, password, HERE);
        let expected = SyncError::File(SyncFileError::UnwrapFailed);
        assert_eq!(refused.unwrap_err(), expected);
    }
    assert_eq!(second.storage.bytes(), before);
}

#[test]
fn identical_accounts_are_not_doubled_on_joining_and_the_preview_says_so() {
    let clock = FakeClock::new();
    let (mut first, mut second) = (device(1, &clock), device(2, &clock));
    first.add("same");
    first.add("only-first");
    let (mut cloud, secret) = first.turn_on(HERE);
    second.add("same");
    second.add("only-second");
    let pending = second
        .session
        .prepare_join(&cloud, secret, SYNC_PASSWORD, HERE)
        .unwrap();
    let expected = JoinPreview {
        added: 1,
        duplicates: 1,
        removed: 0,
    };
    assert_eq!(second.session.preview_join(&pending), Ok(expected));
    let step = second.session.commit_join(pending).unwrap();
    second.write(&mut cloud, step.upload);
    assert_eq!(second.names(), ["only-first", "only-second", "same"]);
    first.sync(&mut cloud);
    assert_eq!(first.names(), second.names());
}

#[test]
fn one_account_in_the_file_replaces_only_one_of_two_identical_local_ones() {
    let clock = FakeClock::new();
    let (mut first, mut second) = (device(1, &clock), device(2, &clock));
    first.add("same");
    let (mut cloud, secret) = first.turn_on(HERE);
    second.add("same");
    second.add("same");
    second.join(&mut cloud, &secret);
    assert_eq!(second.names(), ["same", "same"]);
}

#[test]
fn edits_and_deletions_travel() {
    let mut pair = pair();
    pair.clock.advance_secs(1);
    let a = pair.first.id_of("a");
    let rename = EntryChanges {
        account: Some("a renamed".into()),
        ..EntryChanges::default()
    };
    pair.first
        .session
        .edit_entry(a, rename, &OtpRegistry::with_builtins())
        .unwrap();
    let b = pair.first.id_of("b");
    pair.first.session.delete_entry(b).unwrap();
    pair.first.sync(&mut pair.cloud);
    let (wrote, step) = pair.second.sync(&mut pair.cloud);
    assert!(step.local_changed && !wrote);
    assert_eq!(pair.second.names(), ["a renamed"]);
}

#[test]
fn a_round_with_nothing_new_writes_neither_the_file_nor_the_vault() {
    let mut pair = pair();
    pair.first.sync(&mut pair.cloud);
    let before = pair.first.storage.bytes();
    let step = pair.first.session.sync(&pair.cloud).unwrap();
    let nothing = SyncStep {
        upload: None,
        local_changed: false,
        older_than_seen: false,
    };
    assert_eq!(step, nothing);
    assert_eq!(pair.first.storage.bytes(), before);
}

#[test]
fn a_change_lost_to_another_devices_upload_comes_back() {
    let mut pair = pair();
    pair.clock.advance_secs(1);
    pair.first.add("from-first");
    pair.second.add("from-second");
    // Both read the same version, so the second write replaces the first.
    let read = pair.cloud.clone();
    let first_step = pair.first.session.sync(&read).unwrap();
    let second_step = pair.second.session.sync(&read).unwrap();
    pair.first.write(&mut pair.cloud, first_step.upload);
    pair.second.write(&mut pair.cloud, second_step.upload);
    pair.first.sync(&mut pair.cloud);
    pair.second.sync(&mut pair.cloud);
    assert_eq!(pair.first.names(), ["a", "b", "from-first", "from-second"]);
    assert_eq!(pair.second.names(), pair.first.names());
}

#[test]
fn an_older_copy_is_flagged_and_receives_the_newer_state() {
    let mut pair = pair();
    let mut restored = pair.cloud.clone();
    pair.clock.advance_secs(1);
    pair.first.add("newer");
    pair.first.sync(&mut pair.cloud);
    let (wrote, step) = pair.first.sync(&mut restored);
    assert!(wrote && step.older_than_seen && !step.local_changed);
    assert_eq!(pair.first.entries_in(&restored), 3);
}

#[test]
fn an_upload_that_was_never_written_is_offered_again() {
    let mut pair = pair();
    pair.clock.advance_secs(1);
    pair.first.add("waiting");
    let lost = pair
        .first
        .session
        .sync(&pair.cloud)
        .unwrap()
        .upload
        .unwrap();
    let again = pair
        .first
        .session
        .sync(&pair.cloud)
        .unwrap()
        .upload
        .unwrap();
    assert_eq!(again.revision, lost.revision);
    pair.first.sync(&mut pair.cloud);
    assert_eq!(pair.first.session.sync(&pair.cloud).unwrap().upload, None);
}

#[test]
fn a_damaged_file_changes_nothing() {
    let mut pair = pair();
    let last = pair.cloud.len() - 1;
    pair.cloud[last] ^= 1;
    let before = pair.first.storage.bytes();
    let result = pair.first.session.sync(&pair.cloud);
    assert_eq!(result, Err(SyncError::File(SyncFileError::Damaged)));
    assert_eq!(pair.first.storage.bytes(), before);
}

#[test]
fn a_replacement_file_repairs_sync_for_the_other_devices() {
    let mut pair = pair();
    pair.clock.advance_secs(1);
    pair.first.add("kept");
    let repaired = pair
        .first
        .session
        .replacement_sync_file("another long passphrase", kdf())
        .unwrap();
    pair.first.write(&mut pair.cloud, Some(repaired));
    let opened = open_with_password("another long passphrase", &pair.secret, &pair.cloud);
    assert!(opened.is_ok());
    pair.second.sync(&mut pair.cloud);
    assert_eq!(pair.second.names(), ["a", "b", "kept"]);
}

#[test]
fn after_a_password_change_other_devices_carry_on_and_keep_the_new_lock() {
    let mut pair = pair();
    let new_password = "the new long passphrase";
    let step = pair
        .first
        .session
        .change_sync_password(&pair.cloud, new_password, kdf())
        .unwrap();
    pair.first.write(&mut pair.cloud, step.upload);
    assert!(open_with_password(SYNC_PASSWORD, &pair.secret, &pair.cloud).is_err());
    pair.clock.advance_secs(1);
    pair.second.add("after-the-change");
    pair.second.sync(&mut pair.cloud);
    assert!(open_with_password(new_password, &pair.secret, &pair.cloud).is_ok());
    pair.first.sync(&mut pair.cloud);
    assert_eq!(pair.first.names(), ["a", "after-the-change", "b"]);
}

#[test]
fn a_new_master_key_needs_a_new_place() {
    let mut pair = pair();
    let same = pair
        .first
        .session
        .prepare_new_sync(SYNC_PASSWORD, HERE, kdf());
    assert_eq!(same.unwrap_err(), SyncError::SameLocation);
}

#[test]
fn after_a_new_master_key_other_devices_must_join_again() {
    let mut pair = pair();
    let (mut new_cloud, new_secret) = pair.first.turn_on("cloud/second.mfasync");
    assert_ne!(pair.secret, new_secret);
    let refused = pair.second.session.sync(&new_cloud);
    assert_eq!(refused, Err(SyncError::File(SyncFileError::OtherMasterKey)));
    pair.second.join(&mut new_cloud, &new_secret);
    assert_eq!(pair.second.names(), ["a", "b"]);
}

#[test]
fn turning_sync_off_keeps_the_accounts_and_forgets_the_keys() {
    let mut pair = pair();
    pair.first.session.disable_sync().unwrap();
    assert_eq!(pair.first.session.sync_status(), Ok(None));
    let after = pair.first.session.sync(&pair.cloud);
    assert_eq!(after, Err(SyncError::NotEnabled));
    assert_eq!(
        pair.first.session.disable_sync(),
        Err(SyncError::NotEnabled)
    );
    assert_eq!(pair.first.names(), ["a", "b"]);
}

#[test]
fn the_sync_secret_is_shown_only_for_the_device_password() {
    let mut pair = pair();
    let wrong = pair.first.session.reveal_sync_secret(SYNC_PASSWORD);
    let expected = SyncError::Session(SessionError::Vault(VaultError::UnwrapFailed));
    assert_eq!(wrong, Err(expected));
    let password = pair.first.password.clone();
    assert_eq!(
        pair.first.session.reveal_sync_secret(&password),
        Ok(pair.secret)
    );
}

#[test]
fn a_locked_vault_refuses_to_sync() {
    let mut pair = pair();
    pair.first.session.lock();
    let result = pair.first.session.sync(&pair.cloud);
    assert_eq!(result, Err(SyncError::Session(SessionError::Locked)));
}

#[test]
fn syncing_in_the_background_does_not_delay_the_idle_lock() {
    let mut pair = pair();
    pair.first.session.record_activity();
    pair.clock.advance_secs(200);
    pair.first.session.sync(&pair.cloud).unwrap();
    pair.clock.advance_secs(101);
    assert!(!pair.first.session.is_unlocked());
}

#[test]
fn if_the_vault_cannot_be_saved_nothing_is_offered_for_upload() {
    let mut pair = pair();
    pair.clock.advance_secs(1);
    pair.second.add("incoming");
    pair.first.add("outgoing");
    pair.second.sync(&mut pair.cloud);
    pair.first.storage.set_failing(true);
    let result = pair.first.session.sync(&pair.cloud);
    assert!(matches!(
        result,
        Err(SyncError::Session(SessionError::Storage(_)))
    ));
    pair.first.storage.set_failing(false);
    // The merge stayed in memory; the next round saves it and writes the file.
    assert!(pair
        .first
        .session
        .sync(&pair.cloud)
        .unwrap()
        .upload
        .is_some());
}
