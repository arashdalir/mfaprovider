//! A vault that can be created, unlocked, edited and locked (ADR-0003, TASK-0006).

use std::time::Duration;

use thiserror::Error;
use zeroize::Zeroizing;

use super::clock::Clock;
use super::codes::EntryCode;
use super::container::{self, OpenedVault, VAULT_KEY_LENGTH};
use super::entries::{EntryChanges, EntryError, ImportOp, NewEntry};
use super::error::VaultError;
use super::kdf::KdfParams;
use super::model::{EntryId, VaultData};
use super::rng::Rng;
use super::storage::{Storage, StorageError};
use crate::otp::OtpRegistry;

mod sync_ops;

pub use sync_ops::{
    JoinPreview, PendingJoin, PendingSyncSetup, SyncError, SyncStatus, SyncStep, Upload,
    MIN_SYNC_PASSWORD_CHARS,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SessionError {
    #[error("the vault is locked")]
    Locked,
    #[error("no vault exists yet")]
    NoVault,
    #[error("a vault already exists")]
    AlreadyExists,
    #[error(transparent)]
    Entry(#[from] EntryError),
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// When the session locks itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LockPolicy {
    /// Lock after this much time without user activity; `None` never locks on idle.
    pub idle_timeout: Option<Duration>,
    pub on_background: bool,
    pub on_suspend: bool,
}

impl Default for LockPolicy {
    fn default() -> Self {
        Self {
            idle_timeout: Some(Duration::from_secs(300)),
            on_background: true,
            on_suspend: true,
        }
    }
}

/// Things the platform tells the session about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemEvent {
    Backgrounded,
    Suspended,
}

struct Unlocked {
    vault: OpenedVault,
    data: VaultData,
    last_activity_ms: u64,
    dirty: bool,
}

/// The vault key and entries exist in memory only between `unlock`/`create` and `lock`.
pub struct VaultSession<S: Storage, C: Clock, R: Rng> {
    storage: S,
    clock: C,
    rng: R,
    policy: LockPolicy,
    unlocked: Option<Unlocked>,
}

impl<S: Storage, C: Clock, R: Rng> VaultSession<S, C, R> {
    pub fn new(storage: S, clock: C, rng: R, policy: LockPolicy) -> Self {
        Self {
            storage,
            clock,
            rng,
            policy,
            unlocked: None,
        }
    }

    pub fn set_policy(&mut self, policy: LockPolicy) {
        self.policy = policy;
    }

    /// # Errors
    /// If the storage cannot be read.
    pub fn vault_exists(&self) -> Result<bool, SessionError> {
        Ok(self.storage.read()?.is_some())
    }

    /// Creates the vault and leaves it unlocked. Never overwrites an existing vault.
    ///
    /// # Errors
    /// [`SessionError::AlreadyExists`] if storage already holds a vault; otherwise key
    /// derivation, encryption or storage errors. On error the session stays locked.
    pub fn create(&mut self, password: &str, kdf: KdfParams) -> Result<(), SessionError> {
        if self.vault_exists()? {
            return Err(SessionError::AlreadyExists);
        }
        let data = VaultData::empty(self.clock.unix_ms());
        let (vault, file) = container::create(password, &data, kdf, &mut self.rng)?;
        self.storage.write(&file)?;
        self.unlocked = Some(Unlocked {
            vault,
            data,
            last_activity_ms: self.clock.monotonic_ms(),
            dirty: false,
        });
        Ok(())
    }

    /// # Errors
    /// [`SessionError::NoVault`] if nothing is stored; [`VaultError::UnwrapFailed`] for a wrong
    /// password (indistinguishable from a damaged header). On error the session stays locked.
    pub fn unlock(&mut self, password: &str) -> Result<(), SessionError> {
        let file = self.storage.read()?.ok_or(SessionError::NoVault)?;
        let (vault, data) = container::open(password, &file)?;
        self.unlocked = Some(Unlocked {
            vault,
            data,
            last_activity_ms: self.clock.monotonic_ms(),
            dirty: false,
        });
        Ok(())
    }

    /// Unlocks with the vault key itself (released by a biometric, ADR-0016).
    ///
    /// # Errors
    /// [`SessionError::NoVault`] if nothing is stored; [`VaultError::UnwrapFailed`] if the key
    /// is wrong. On error the session stays locked.
    pub fn unlock_with_key(&mut self, key: &[u8; VAULT_KEY_LENGTH]) -> Result<(), SessionError> {
        let file = self.storage.read()?.ok_or(SessionError::NoVault)?;
        let (vault, data) = container::open_with_key(key, &file)?;
        self.unlocked = Some(Unlocked {
            vault,
            data,
            last_activity_ms: self.clock.monotonic_ms(),
            dirty: false,
        });
        Ok(())
    }

    /// The vault key, to be wrapped by an OS-bound key. The password is checked again first,
    /// so an unlocked but unattended session cannot hand the key out.
    ///
    /// # Errors
    /// [`SessionError::Locked`]; [`VaultError::UnwrapFailed`] for a wrong password.
    pub fn export_key(
        &mut self,
        password: &str,
    ) -> Result<Zeroizing<[u8; VAULT_KEY_LENGTH]>, SessionError> {
        self.verify_password(password)?;
        let unlocked = self.unlocked.as_ref().ok_or(SessionError::Locked)?;
        Ok(unlocked.vault.key_copy())
    }

    /// Drops the vault key and all entries; they are wiped from memory as they are dropped.
    pub fn lock(&mut self) {
        self.unlocked = None;
    }

    pub fn is_unlocked(&mut self) -> bool {
        self.enforce_idle_timeout();
        self.unlocked.is_some()
    }

    /// Call periodically from a timer so an idle vault locks even if nobody touches it.
    pub fn tick(&mut self) {
        self.enforce_idle_timeout();
    }

    pub fn notify(&mut self, event: SystemEvent) {
        let locks = match event {
            SystemEvent::Backgrounded => self.policy.on_background,
            SystemEvent::Suspended => self.policy.on_suspend,
        };
        if locks {
            self.lock();
        }
    }

    /// Records user activity (a tap, a keypress), postponing the idle lock. Reading codes
    /// with [`VaultSession::data`] is *not* activity: the UI polls them every second.
    pub fn record_activity(&mut self) {
        self.enforce_idle_timeout();
        let now = self.clock.monotonic_ms();
        if let Some(unlocked) = &mut self.unlocked {
            unlocked.last_activity_ms = now;
        }
    }

    /// # Errors
    /// [`SessionError::Locked`] if the vault is locked.
    pub fn data(&mut self) -> Result<&VaultData, SessionError> {
        self.enforce_idle_timeout();
        self.unlocked
            .as_ref()
            .map(|unlocked| &unlocked.data)
            .ok_or(SessionError::Locked)
    }

    /// Applies `change`, stamps `updated_at` and saves atomically. Counts as user activity.
    ///
    /// If saving fails, the change stays in memory and [`VaultSession::is_dirty`] turns true
    /// until [`VaultSession::save`] succeeds; locking first discards it.
    ///
    /// # Errors
    /// [`SessionError::Locked`], or an encryption or storage error from saving.
    pub fn edit<T>(&mut self, change: impl FnOnce(&mut VaultData) -> T) -> Result<T, SessionError> {
        self.try_edit(|data| Ok(change(data)))
    }

    /// Like [`VaultSession::edit`] for a change that can be refused. If `change` returns an
    /// error nothing is stamped or saved and it does not count as activity, so it must not
    /// have modified the data (the [`VaultData`] entry operations all validate first).
    ///
    /// # Errors
    /// [`SessionError::Locked`], the change's own error, or an encryption or storage error.
    pub fn try_edit<T>(
        &mut self,
        change: impl FnOnce(&mut VaultData) -> Result<T, EntryError>,
    ) -> Result<T, SessionError> {
        self.enforce_idle_timeout();
        let (now_unix, now_monotonic) = (self.clock.unix_ms(), self.clock.monotonic_ms());
        let unlocked = self.unlocked.as_mut().ok_or(SessionError::Locked)?;
        let result = change(&mut unlocked.data)?;
        unlocked.data.updated_at = now_unix;
        unlocked.last_activity_ms = now_monotonic;
        unlocked.dirty = true;
        Self::persist(unlocked, &mut self.storage, &mut self.rng)?;
        Ok(result)
    }

    /// Adds an account under a fresh random id and saves.
    ///
    /// # Errors
    /// As [`VaultData::add_entry`], plus those of [`VaultSession::edit`].
    pub fn add_entry(
        &mut self,
        new: NewEntry,
        registry: &OtpRegistry,
    ) -> Result<EntryId, SessionError> {
        let id = EntryId::random(&mut self.rng);
        let now = self.clock.unix_ms();
        self.try_edit(|data| data.add_entry(id, new, registry, now))?;
        Ok(id)
    }

    /// A fresh random id for an entry about to be added by [`VaultSession::import`].
    pub fn new_entry_id(&mut self) -> EntryId {
        EntryId::random(&mut self.rng)
    }

    /// Applies every op of an import with a single save, or none of them if any is invalid.
    ///
    /// # Errors
    /// As [`VaultData::apply_import`], plus those of [`VaultSession::edit`].
    pub fn import(
        &mut self,
        ops: Vec<ImportOp>,
        registry: &OtpRegistry,
    ) -> Result<(), SessionError> {
        let now = self.clock.unix_ms();
        self.try_edit(|data| data.apply_import(ops, registry, now))
    }

    /// # Errors
    /// As [`VaultData::edit_entry`], plus those of [`VaultSession::edit`].
    pub fn edit_entry(
        &mut self,
        id: EntryId,
        changes: EntryChanges,
        registry: &OtpRegistry,
    ) -> Result<(), SessionError> {
        let now = self.clock.unix_ms();
        self.try_edit(|data| data.edit_entry(id, changes, registry, now))
    }

    /// # Errors
    /// As [`VaultData::delete_entry`], plus those of [`VaultSession::edit`].
    pub fn delete_entry(&mut self, id: EntryId) -> Result<(), SessionError> {
        let now = self.clock.unix_ms();
        self.try_edit(|data| data.delete_entry(id, now))
    }

    /// # Errors
    /// As [`VaultData::move_entry`], plus those of [`VaultSession::edit`].
    pub fn move_entry(&mut self, id: EntryId, position: usize) -> Result<(), SessionError> {
        self.try_edit(|data| data.move_entry(id, position))
    }

    /// Moves a counter-based entry to its next code and saves; returns the new counter.
    ///
    /// # Errors
    /// As [`VaultData::advance_counter`], plus those of [`VaultSession::edit`].
    pub fn advance_counter(
        &mut self,
        id: EntryId,
        registry: &OtpRegistry,
    ) -> Result<u64, SessionError> {
        let now = self.clock.unix_ms();
        self.try_edit(|data| data.advance_counter(id, registry, now))
    }

    /// The current code of every entry. Like [`VaultSession::data`], not user activity.
    ///
    /// # Errors
    /// [`SessionError::Locked`].
    pub fn codes(&mut self, registry: &OtpRegistry) -> Result<Vec<EntryCode>, SessionError> {
        let now = self.clock.unix_ms();
        Ok(self.data()?.codes(registry, now))
    }

    /// True while the last change could not be saved.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.unlocked
            .as_ref()
            .is_some_and(|unlocked| unlocked.dirty)
    }

    /// Saves the current entries again, e.g. after a failed save.
    ///
    /// # Errors
    /// [`SessionError::Locked`], or an encryption or storage error.
    pub fn save(&mut self) -> Result<(), SessionError> {
        self.enforce_idle_timeout();
        let unlocked = self.unlocked.as_mut().ok_or(SessionError::Locked)?;
        Self::persist(unlocked, &mut self.storage, &mut self.rng)
    }

    /// Checks `password` against the stored vault without changing anything, for actions that
    /// need the user to prove presence again (such as revealing a seed). Success counts as
    /// user activity.
    ///
    /// # Errors
    /// [`SessionError::Locked`]; [`VaultError::UnwrapFailed`] for a wrong password.
    pub fn verify_password(&mut self, password: &str) -> Result<(), SessionError> {
        self.enforce_idle_timeout();
        if self.unlocked.is_none() {
            return Err(SessionError::Locked);
        }
        let file = self.storage.read()?.ok_or(SessionError::NoVault)?;
        container::open(password, &file)?;
        self.record_activity();
        Ok(())
    }

    /// Changes the master password. The current password is checked again, so an unlocked
    /// but unattended session cannot silently change it. Entries are not re-encrypted:
    /// only the key slot is replaced.
    ///
    /// # Errors
    /// [`SessionError::Locked`]; [`VaultError::UnwrapFailed`] if `current` is wrong (nothing
    /// is written); key derivation or storage errors.
    pub fn change_password(
        &mut self,
        current: &str,
        new: &str,
        kdf: KdfParams,
    ) -> Result<(), SessionError> {
        self.enforce_idle_timeout();
        let unlocked = self.unlocked.as_mut().ok_or(SessionError::Locked)?;
        let file = self.storage.read()?.ok_or(SessionError::NoVault)?;
        let (mut verified, _discarded) = container::open(current, &file)?;
        let rewrapped = verified.rewrap(&file, new, kdf, &mut self.rng)?;
        self.storage.write(&rewrapped)?;
        unlocked.vault = verified;
        Ok(())
    }

    fn persist(unlocked: &mut Unlocked, storage: &mut S, rng: &mut R) -> Result<(), SessionError> {
        let file = unlocked.vault.save(&unlocked.data, rng)?;
        storage.write(&file)?;
        unlocked.dirty = false;
        Ok(())
    }

    fn enforce_idle_timeout(&mut self) {
        let (Some(timeout), Some(unlocked)) = (self.policy.idle_timeout, &self.unlocked) else {
            return;
        };
        let idle_ms = self
            .clock
            .monotonic_ms()
            .saturating_sub(unlocked.last_activity_ms);
        if u128::from(idle_ms) >= timeout.as_millis() {
            self.unlocked = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::storage::MemoryStorage;
    use super::super::test_support::{entry, CountingRng, FakeClock};
    use super::*;

    type TestSession = VaultSession<MemoryStorage, FakeClock, CountingRng>;

    const PASSWORD: &str = "correct horse battery staple";

    fn kdf() -> KdfParams {
        KdfParams::MINIMUM
    }

    fn session_over(storage: &MemoryStorage, clock: &FakeClock, policy: LockPolicy) -> TestSession {
        VaultSession::new(storage.clone(), clock.clone(), CountingRng(100), policy)
    }

    fn created(policy: LockPolicy) -> (TestSession, MemoryStorage, FakeClock) {
        let (storage, clock) = (MemoryStorage::new(), FakeClock::new());
        let mut session = session_over(&storage, &clock, policy);
        session.create(PASSWORD, kdf()).unwrap();
        (session, storage, clock)
    }

    fn entry_count(session: &mut TestSession) -> usize {
        session.data().unwrap().entries.len()
    }

    #[test]
    fn creating_a_vault_leaves_it_unlocked_and_stores_it() {
        let (mut session, storage, _) = created(LockPolicy::default());
        assert!(session.is_unlocked());
        assert!(storage.bytes().is_some());
        let data = session.data().unwrap();
        assert_eq!(data.entries.len(), 0);
        assert_eq!(
            data.created_at,
            i64::try_from(FakeClock::START_UNIX_MS).unwrap()
        );
    }

    #[test]
    fn an_existing_vault_is_never_overwritten() {
        let (_, storage, clock) = created(LockPolicy::default());
        let before = storage.bytes();
        let mut second = session_over(&storage, &clock, LockPolicy::default());
        assert_eq!(
            second.create("other", kdf()),
            Err(SessionError::AlreadyExists)
        );
        assert_eq!(storage.bytes(), before);
        assert!(!second.is_unlocked());
    }

    #[test]
    fn a_failed_create_leaves_the_session_locked_and_nothing_stored() {
        let (storage, clock) = (MemoryStorage::new(), FakeClock::new());
        storage.set_failing(true);
        let mut session = session_over(&storage, &clock, LockPolicy::default());
        assert!(matches!(
            session.create(PASSWORD, kdf()),
            Err(SessionError::Storage(_))
        ));
        assert!(!session.is_unlocked());
        assert_eq!(storage.bytes(), None);
    }

    #[test]
    fn unlocking_requires_the_right_password() {
        let (_, storage, clock) = created(LockPolicy::default());
        let mut session = session_over(&storage, &clock, LockPolicy::default());
        assert_eq!(
            session.unlock("wrong"),
            Err(SessionError::Vault(VaultError::UnwrapFailed))
        );
        assert!(!session.is_unlocked());
        assert_eq!(session.data().err(), Some(SessionError::Locked));
        session.unlock(PASSWORD).unwrap();
        assert!(session.is_unlocked());
    }

    #[test]
    fn unlocking_without_a_vault_says_so() {
        let mut session = session_over(
            &MemoryStorage::new(),
            &FakeClock::new(),
            LockPolicy::default(),
        );
        assert_eq!(session.unlock(PASSWORD), Err(SessionError::NoVault));
        assert_eq!(session.vault_exists(), Ok(false));
    }

    #[test]
    fn failed_unlock_attempts_do_not_touch_the_stored_vault() {
        let (_, storage, clock) = created(LockPolicy::default());
        let before = storage.bytes();
        let mut session = session_over(&storage, &clock, LockPolicy::default());
        for guess in ["a", "b", "c"] {
            assert!(session.unlock(guess).is_err());
        }
        assert_eq!(storage.bytes(), before);
    }

    #[test]
    fn locking_drops_access_to_everything() {
        let (mut session, _, _) = created(LockPolicy::default());
        session.edit(|data| data.entries.push(entry(1))).unwrap();
        session.lock();
        assert!(!session.is_unlocked());
        assert_eq!(session.data().err(), Some(SessionError::Locked));
        assert_eq!(session.edit(|_| ()), Err(SessionError::Locked));
        assert_eq!(session.save(), Err(SessionError::Locked));
        assert_eq!(
            session.change_password(PASSWORD, "x", kdf()),
            Err(SessionError::Locked)
        );
        session.lock();
    }

    #[test]
    fn edits_are_saved_and_survive_a_restart() {
        let (mut session, storage, clock) = created(LockPolicy::default());
        clock.advance_secs(10);
        session.edit(|data| data.entries.push(entry(7))).unwrap();
        assert!(!session.is_dirty());

        let mut restarted = session_over(&storage, &clock, LockPolicy::default());
        restarted.unlock(PASSWORD).unwrap();
        let data = restarted.data().unwrap();
        assert_eq!(data.entries.len(), 1);
        assert_eq!(data.entries[0].account, "user7@example.com");
        assert_eq!(
            data.updated_at,
            i64::try_from(FakeClock::START_UNIX_MS + 10_000).unwrap()
        );
    }

    #[test]
    fn edit_returns_what_the_closure_returns() {
        let (mut session, _, _) = created(LockPolicy::default());
        let count = session
            .edit(|data| {
                data.entries.push(entry(1));
                data.entries.len()
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn a_failed_save_keeps_the_edit_in_memory_marks_it_dirty_and_can_be_retried() {
        let (mut session, storage, clock) = created(LockPolicy::default());
        let before = storage.bytes();
        storage.set_failing(true);
        assert!(matches!(
            session.edit(|data| data.entries.push(entry(1))),
            Err(SessionError::Storage(_))
        ));
        assert!(session.is_dirty());
        assert_eq!(entry_count(&mut session), 1);
        assert_eq!(
            storage.bytes(),
            before,
            "the stored vault must be untouched"
        );

        storage.set_failing(false);
        session.save().unwrap();
        assert!(!session.is_dirty());
        let mut restarted = session_over(&storage, &clock, LockPolicy::default());
        restarted.unlock(PASSWORD).unwrap();
        assert_eq!(entry_count(&mut restarted), 1);
    }

    #[test]
    fn locking_discards_unsaved_edits() {
        let (mut session, storage, _) = created(LockPolicy::default());
        storage.set_failing(true);
        let _ = session.edit(|data| data.entries.push(entry(1)));
        session.lock();
        assert!(!session.is_dirty());
        storage.set_failing(false);
        session.unlock(PASSWORD).unwrap();
        assert_eq!(entry_count(&mut session), 0);
    }

    fn short_idle() -> LockPolicy {
        LockPolicy {
            idle_timeout: Some(Duration::from_secs(60)),
            on_background: false,
            on_suspend: false,
        }
    }

    #[test]
    fn the_vault_locks_after_the_idle_timeout() {
        let (mut session, _, clock) = created(short_idle());
        clock.advance_secs(59);
        session.tick();
        assert!(session.is_unlocked());
        clock.advance_secs(1);
        session.tick();
        assert!(!session.is_unlocked());
    }

    #[test]
    fn operations_enforce_the_timeout_even_without_a_tick() {
        let (mut session, _, clock) = created(short_idle());
        clock.advance_secs(61);
        assert_eq!(session.data().err(), Some(SessionError::Locked));
    }

    #[test]
    fn reading_codes_does_not_postpone_the_idle_lock() {
        let (mut session, _, clock) = created(short_idle());
        for _ in 0..59 {
            clock.advance_secs(1);
            assert!(session.data().is_ok());
        }
        clock.advance_secs(1);
        assert_eq!(session.data().err(), Some(SessionError::Locked));
    }

    #[test]
    fn user_activity_and_edits_postpone_the_idle_lock() {
        let (mut session, _, clock) = created(short_idle());
        clock.advance_secs(50);
        session.record_activity();
        clock.advance_secs(50);
        assert!(session.is_unlocked());
        session.edit(|data| data.entries.push(entry(1))).unwrap();
        clock.advance_secs(50);
        assert!(session.is_unlocked());
        clock.advance_secs(10);
        assert!(!session.is_unlocked());
    }

    #[test]
    fn activity_cannot_revive_an_already_expired_session() {
        let (mut session, _, clock) = created(short_idle());
        clock.advance_secs(90);
        session.record_activity();
        assert!(!session.is_unlocked());
    }

    #[test]
    fn without_an_idle_timeout_the_vault_stays_unlocked() {
        let policy = LockPolicy {
            idle_timeout: None,
            ..short_idle()
        };
        let (mut session, _, clock) = created(policy);
        clock.advance_secs(10_000_000);
        assert!(session.is_unlocked());
    }

    #[test]
    fn backgrounding_and_suspending_lock_only_when_the_policy_says_so() {
        let lock_on_both = LockPolicy {
            idle_timeout: None,
            on_background: true,
            on_suspend: true,
        };
        for event in [SystemEvent::Backgrounded, SystemEvent::Suspended] {
            let (mut session, _, _) = created(lock_on_both);
            session.notify(event);
            assert!(!session.is_unlocked(), "{event:?}");
        }
        let background_only = LockPolicy {
            on_suspend: false,
            ..lock_on_both
        };
        let (mut session, _, _) = created(background_only);
        session.notify(SystemEvent::Suspended);
        assert!(session.is_unlocked());
        session.notify(SystemEvent::Backgrounded);
        assert!(!session.is_unlocked());
    }

    fn new_totp(account: &str) -> NewEntry {
        NewEntry {
            otp_type: "totp".into(),
            issuer: "Example".into(),
            account: account.into(),
            secret: crate::secret::Secret::new(b"12345678901234567890".to_vec()),
            params: crate::otp::Params::new().with("digits", "8"),
            icon: None,
            tags: Vec::new(),
            notes: String::new(),
        }
    }

    fn registry() -> OtpRegistry {
        OtpRegistry::with_builtins()
    }

    #[test]
    fn entry_operations_are_saved_and_survive_a_restart() {
        let (mut session, storage, clock) = created(LockPolicy::default());
        let first = session.add_entry(new_totp("a"), &registry()).unwrap();
        let second = session.add_entry(new_totp("b"), &registry()).unwrap();
        assert_ne!(first, second);
        session.move_entry(second, 0).unwrap();
        let rename = EntryChanges {
            account: Some("renamed".into()),
            ..EntryChanges::default()
        };
        session.edit_entry(first, rename, &registry()).unwrap();
        session.delete_entry(second).unwrap();

        let mut restarted = session_over(&storage, &clock, LockPolicy::default());
        restarted.unlock(PASSWORD).unwrap();
        let data = restarted.data().unwrap();
        assert_eq!(data.entries.len(), 1);
        assert_eq!(data.entries[0].account, "renamed");
        assert_eq!(data.tombstones.len(), 1);
        assert_eq!(data.tombstones[0].id, second);
    }

    #[test]
    fn a_refused_operation_writes_nothing_and_is_not_activity() {
        let (mut session, storage, clock) = created(short_idle());
        let before = storage.bytes();
        clock.advance_secs(50);
        let mut bad = new_totp("a");
        bad.otp_type = "vendor-x".into();
        assert_eq!(
            session.add_entry(bad, &registry()),
            Err(SessionError::Entry(EntryError::UnknownType(
                "vendor-x".into()
            )))
        );
        assert_eq!(storage.bytes(), before);
        assert!(!session.is_dirty());
        clock.advance_secs(10);
        assert!(
            !session.is_unlocked(),
            "a refused operation must not postpone the lock"
        );
    }

    #[test]
    fn entry_operations_need_an_unlocked_vault() {
        let (mut session, _, _) = created(LockPolicy::default());
        let id = session.add_entry(new_totp("a"), &registry()).unwrap();
        session.lock();
        assert_eq!(
            session.add_entry(new_totp("b"), &registry()),
            Err(SessionError::Locked)
        );
        assert_eq!(session.delete_entry(id), Err(SessionError::Locked));
        assert_eq!(session.move_entry(id, 0), Err(SessionError::Locked));
        assert_eq!(
            session.advance_counter(id, &registry()),
            Err(SessionError::Locked)
        );
        assert_eq!(session.codes(&registry()).err(), Some(SessionError::Locked));
    }

    #[test]
    fn codes_follow_the_clock_and_do_not_count_as_activity() {
        let (mut session, _, clock) = created(short_idle());
        session.add_entry(new_totp("a"), &registry()).unwrap();
        let shown =
            |session: &mut TestSession| match &session.codes(&registry()).unwrap()[0].outcome {
                crate::vault::CodeOutcome::Ready(live) => live.clone(),
                other => panic!("unexpected {other:?}"),
            };
        let first = shown(&mut session);
        clock.advance_secs(30);
        let second = shown(&mut session);
        assert_ne!(first.code, second.code);
        assert_eq!(first.validity.map(|v| v.period_seconds), Some(30));
        clock.advance_secs(30);
        assert_eq!(session.codes(&registry()).err(), Some(SessionError::Locked));
    }

    #[test]
    fn an_import_is_one_atomic_saved_change() {
        let (mut session, storage, clock) = created(LockPolicy::default());
        let existing = session.add_entry(new_totp("old"), &registry()).unwrap();
        let (first, second) = (session.new_entry_id(), session.new_entry_id());
        assert_ne!(first, second);
        session
            .import(
                vec![
                    ImportOp::Add {
                        id: first,
                        new: new_totp("one"),
                    },
                    ImportOp::Replace {
                        id: existing,
                        with: new_totp("replaced"),
                    },
                    ImportOp::Add {
                        id: second,
                        new: new_totp("two"),
                    },
                ],
                &registry(),
            )
            .unwrap();
        let mut restarted = session_over(&storage, &clock, LockPolicy::default());
        restarted.unlock(PASSWORD).unwrap();
        let accounts: Vec<_> = restarted
            .data()
            .unwrap()
            .entries
            .iter()
            .map(|e| e.account.clone())
            .collect();
        assert_eq!(accounts, ["replaced", "one", "two"]);
    }

    #[test]
    fn a_refused_import_writes_nothing() {
        let (mut session, storage, _) = created(short_idle());
        let before = storage.bytes();
        let mut bad = new_totp("bad");
        bad.otp_type = "vendor-x".into();
        let id = session.new_entry_id();
        assert!(matches!(
            session.import(vec![ImportOp::Add { id, new: bad }], &registry()),
            Err(SessionError::Entry(EntryError::UnknownType(_)))
        ));
        assert_eq!(storage.bytes(), before);
        assert_eq!(entry_count(&mut session), 0);
    }

    #[test]
    fn importing_needs_an_unlocked_vault() {
        let (mut session, _, _) = created(LockPolicy::default());
        session.lock();
        assert_eq!(
            session.import(Vec::new(), &registry()),
            Err(SessionError::Locked)
        );
    }

    #[test]
    fn advancing_a_hotp_counter_is_saved() {
        let (mut session, storage, clock) = created(LockPolicy::default());
        let mut new = new_totp("a");
        new.otp_type = "hotp".into();
        let id = session.add_entry(new, &registry()).unwrap();
        assert_eq!(session.advance_counter(id, &registry()), Ok(1));
        let mut restarted = session_over(&storage, &clock, LockPolicy::default());
        restarted.unlock(PASSWORD).unwrap();
        let counter = restarted
            .data()
            .unwrap()
            .entry(id)
            .unwrap()
            .params
            .get("counter");
        assert_eq!(counter, Some("1"));
    }

    #[test]
    fn verifying_the_password_checks_it_without_writing_and_counts_as_activity() {
        let (mut session, storage, clock) = created(short_idle());
        let before = storage.bytes();
        assert_eq!(
            session.verify_password("wrong"),
            Err(SessionError::Vault(VaultError::UnwrapFailed))
        );
        clock.advance_secs(50);
        session.verify_password(PASSWORD).unwrap();
        clock.advance_secs(50);
        assert!(session.is_unlocked(), "a successful check is activity");
        assert_eq!(storage.bytes(), before);
        session.lock();
        assert_eq!(session.verify_password(PASSWORD), Err(SessionError::Locked));
    }

    #[test]
    fn the_default_policy_locks_after_five_minutes_and_on_background_and_suspend() {
        let policy = LockPolicy::default();
        assert_eq!(policy.idle_timeout, Some(Duration::from_secs(300)));
        assert!(policy.on_background && policy.on_suspend);
    }

    #[test]
    fn changing_the_password_replaces_the_slot_but_not_the_entries() {
        let (mut session, storage, clock) = created(LockPolicy::default());
        session.edit(|data| data.entries.push(entry(1))).unwrap();
        let before = storage.bytes().unwrap();

        session
            .change_password(PASSWORD, "a brand new password", kdf())
            .unwrap();
        let after = storage.bytes().unwrap();
        assert_ne!(after[12..109], before[12..109]);
        assert_eq!(
            after[109..],
            before[109..],
            "the encrypted body must be copied unchanged"
        );

        let mut restarted = session_over(&storage, &clock, LockPolicy::default());
        assert!(restarted.unlock(PASSWORD).is_err());
        restarted.unlock("a brand new password").unwrap();
        assert_eq!(entry_count(&mut restarted), 1);
    }

    #[test]
    fn the_session_keeps_working_after_a_password_change() {
        let (mut session, storage, clock) = created(LockPolicy::default());
        session.change_password(PASSWORD, "new one", kdf()).unwrap();
        session.edit(|data| data.entries.push(entry(2))).unwrap();
        let mut restarted = session_over(&storage, &clock, LockPolicy::default());
        restarted.unlock("new one").unwrap();
        assert_eq!(entry_count(&mut restarted), 1);
    }

    #[test]
    fn changing_the_password_needs_the_current_one_and_writes_nothing_otherwise() {
        let (mut session, storage, _) = created(LockPolicy::default());
        let before = storage.bytes();
        assert_eq!(
            session.change_password("not the password", "new one", kdf()),
            Err(SessionError::Vault(VaultError::UnwrapFailed))
        );
        assert_eq!(storage.bytes(), before);
    }

    #[test]
    fn a_failed_password_change_leaves_the_old_password_working() {
        let (mut session, storage, clock) = created(LockPolicy::default());
        let before = storage.bytes();
        storage.set_failing(true);
        assert!(matches!(
            session.change_password(PASSWORD, "new one", kdf()),
            Err(SessionError::Storage(_))
        ));
        storage.set_failing(false);
        assert_eq!(storage.bytes(), before);
        session.lock();
        let mut restarted = session_over(&storage, &clock, LockPolicy::default());
        restarted.unlock(PASSWORD).unwrap();
    }

    mod key_unlock {
        use super::*;

        #[test]
        fn exporting_the_key_needs_the_password_again() {
            let (mut session, _, _) = created(LockPolicy::default());
            assert_eq!(
                session.export_key("wrong").unwrap_err(),
                SessionError::Vault(VaultError::UnwrapFailed)
            );
            assert!(session.export_key(PASSWORD).is_ok());
        }

        #[test]
        fn a_locked_session_hands_out_no_key() {
            let (mut session, _, _) = created(LockPolicy::default());
            session.lock();
            assert_eq!(
                session.export_key(PASSWORD).unwrap_err(),
                SessionError::Locked
            );
        }

        #[test]
        fn the_exported_key_unlocks_a_locked_session_with_all_entries() {
            let (mut session, storage, clock) = created(LockPolicy::default());
            session.add_entry(new_totp("alice"), &registry()).unwrap();
            let key = session.export_key(PASSWORD).unwrap();
            let mut fresh = session_over(&storage, &clock, LockPolicy::default());
            assert!(!fresh.is_unlocked());
            fresh.unlock_with_key(&key).unwrap();
            assert_eq!(entry_count(&mut fresh), 1);
        }

        #[test]
        fn a_wrong_key_leaves_the_session_locked() {
            let (_, storage, clock) = created(LockPolicy::default());
            let mut fresh = session_over(&storage, &clock, LockPolicy::default());
            assert_eq!(
                fresh
                    .unlock_with_key(&[1_u8; VAULT_KEY_LENGTH])
                    .unwrap_err(),
                SessionError::Vault(VaultError::UnwrapFailed)
            );
            assert!(!fresh.is_unlocked());
        }

        #[test]
        fn unlocking_with_a_key_without_a_vault_says_so() {
            let (storage, clock) = (MemoryStorage::new(), FakeClock::new());
            let mut session = session_over(&storage, &clock, LockPolicy::default());
            assert_eq!(
                session
                    .unlock_with_key(&[0_u8; VAULT_KEY_LENGTH])
                    .unwrap_err(),
                SessionError::NoVault
            );
        }

        #[test]
        fn the_key_still_unlocks_after_the_password_was_changed() {
            let (mut session, storage, clock) = created(LockPolicy::default());
            let key = session.export_key(PASSWORD).unwrap();
            session
                .change_password(PASSWORD, "new password", kdf())
                .unwrap();
            let mut fresh = session_over(&storage, &clock, LockPolicy::default());
            fresh.unlock_with_key(&key).unwrap();
            assert!(fresh.is_unlocked());
        }

        #[test]
        fn using_the_key_does_not_touch_the_stored_vault() {
            let (mut session, storage, clock) = created(LockPolicy::default());
            let key = session.export_key(PASSWORD).unwrap();
            let before = storage.bytes();
            let mut fresh = session_over(&storage, &clock, LockPolicy::default());
            fresh.unlock_with_key(&key).unwrap();
            assert_eq!(storage.bytes(), before);
        }
    }
}
