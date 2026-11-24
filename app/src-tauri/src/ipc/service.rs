//! What each IPC command does, independent of Tauri so it can be tested directly.

use mfa_core::otp::OtpRegistry;
use mfa_core::vault::{
    Clock, EntryChanges, KdfParams, LockPolicy, NewEntry, Rng, Storage, SystemEvent, VaultSession,
};

use super::dto::{
    EntryChangesRequest, EntryView, NewEntryRequest, RevealedSecret, VaultStatus, entry_views,
    parse_entry_id,
};
use super::error::{IpcError, IpcErrorCode};

pub struct VaultService<S: Storage, C: Clock, R: Rng> {
    session: VaultSession<S, C, R>,
    registry: OtpRegistry,
}

impl<S: Storage, C: Clock, R: Rng> VaultService<S, C, R> {
    pub fn new(storage: S, clock: C, rng: R) -> Self {
        Self {
            session: VaultSession::new(storage, clock, rng, LockPolicy::default()),
            registry: OtpRegistry::with_builtins(),
        }
    }

    pub fn status(&mut self) -> Result<VaultStatus, IpcError> {
        Ok(VaultStatus {
            exists: self.session.vault_exists()?,
            unlocked: self.session.is_unlocked(),
        })
    }

    pub fn create_vault(&mut self, password: &str, kdf: KdfParams) -> Result<(), IpcError> {
        if password.is_empty() {
            return Err(IpcError::invalid_input("the password is empty"));
        }
        Ok(self.session.create(password, kdf)?)
    }

    pub fn unlock(&mut self, password: &str) -> Result<(), IpcError> {
        Ok(self.session.unlock(password)?)
    }

    pub fn lock(&mut self) {
        self.session.lock();
    }

    pub fn notify(&mut self, event: SystemEvent) {
        self.session.notify(event);
    }

    pub fn tick(&mut self) {
        self.session.tick();
    }

    pub fn record_activity(&mut self) {
        self.session.record_activity();
    }

    pub fn list_entries(&mut self) -> Result<Vec<EntryView>, IpcError> {
        let codes = self.session.codes(&self.registry)?;
        Ok(entry_views(self.session.data()?, codes))
    }

    pub fn add_entry(&mut self, request: NewEntryRequest) -> Result<String, IpcError> {
        let new = NewEntry::try_from(request)?;
        Ok(self.session.add_entry(new, &self.registry)?.to_string())
    }

    pub fn edit_entry(&mut self, id: &str, request: EntryChangesRequest) -> Result<(), IpcError> {
        let id = parse_entry_id(id)?;
        let changes = EntryChanges::try_from(request)?;
        Ok(self.session.edit_entry(id, changes, &self.registry)?)
    }

    pub fn delete_entry(&mut self, id: &str) -> Result<(), IpcError> {
        Ok(self.session.delete_entry(parse_entry_id(id)?)?)
    }

    pub fn move_entry(&mut self, id: &str, position: usize) -> Result<(), IpcError> {
        Ok(self.session.move_entry(parse_entry_id(id)?, position)?)
    }

    pub fn advance_counter(&mut self, id: &str) -> Result<(), IpcError> {
        let id = parse_entry_id(id)?;
        self.session.advance_counter(id, &self.registry)?;
        Ok(())
    }

    /// The seed of one entry, only after the master password is entered again.
    pub fn reveal_secret(&mut self, id: &str, password: &str) -> Result<RevealedSecret, IpcError> {
        let id = parse_entry_id(id)?;
        self.session.verify_password(password)?;
        let entry = self
            .session
            .data()?
            .entry(id)
            .ok_or(IpcError::new(IpcErrorCode::NotFound))?;
        Ok(RevealedSecret::of(entry))
    }

    pub fn change_password(
        &mut self,
        current: &str,
        new: &str,
        kdf: KdfParams,
    ) -> Result<(), IpcError> {
        if new.is_empty() {
            return Err(IpcError::invalid_input("the new password is empty"));
        }
        Ok(self.session.change_password(current, new, kdf)?)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use mfa_core::vault::MemoryStorage;

    use super::*;
    use crate::ipc::dto::CodeState;

    const PASSWORD: &str = "correct horse battery staple";

    #[derive(Clone)]
    struct TestClock(Rc<Cell<u64>>);

    impl TestClock {
        fn advance_secs(&self, seconds: u64) {
            self.0.set(self.0.get() + seconds * 1000);
        }
    }

    impl Clock for TestClock {
        fn unix_ms(&self) -> i64 {
            i64::try_from(1_700_000_000_000 + self.0.get()).unwrap()
        }

        fn monotonic_ms(&self) -> u64 {
            self.0.get()
        }
    }

    struct CountingRng(u8);

    impl Rng for CountingRng {
        fn fill_bytes(&mut self, destination: &mut [u8]) {
            for byte in destination {
                *byte = self.0;
                self.0 = self.0.wrapping_add(1);
            }
        }
    }

    type TestService = VaultService<MemoryStorage, TestClock, CountingRng>;

    fn service() -> (TestService, MemoryStorage, TestClock) {
        let (storage, clock) = (MemoryStorage::new(), TestClock(Rc::new(Cell::new(0))));
        let service = VaultService::new(storage.clone(), clock.clone(), CountingRng(1));
        (service, storage, clock)
    }

    fn unlocked() -> (TestService, MemoryStorage, TestClock) {
        let (mut service, storage, clock) = service();
        service.create_vault(PASSWORD, KdfParams::MINIMUM).unwrap();
        (service, storage, clock)
    }

    fn totp_request(account: &str) -> NewEntryRequest {
        NewEntryRequest {
            otp_type: "totp".into(),
            issuer: "Example".into(),
            account: account.into(),
            secret_base32: "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ".into(),
            params: [("digits".to_owned(), "8".to_owned())].into(),
            icon: None,
            tags: vec![],
            notes: String::new(),
        }
    }

    fn no_changes() -> EntryChangesRequest {
        EntryChangesRequest {
            issuer: None,
            account: None,
            secret_base32: None,
            params: None,
            icon: None,
            clear_icon: false,
            tags: None,
            notes: None,
        }
    }

    #[test]
    fn status_follows_the_vault_lifecycle() {
        let (mut service, _, _) = service();
        assert_eq!(
            service.status().unwrap(),
            VaultStatus {
                exists: false,
                unlocked: false
            }
        );
        service.create_vault(PASSWORD, KdfParams::MINIMUM).unwrap();
        assert_eq!(
            service.status().unwrap(),
            VaultStatus {
                exists: true,
                unlocked: true
            }
        );
        service.lock();
        assert_eq!(
            service.status().unwrap(),
            VaultStatus {
                exists: true,
                unlocked: false
            }
        );
    }

    #[test]
    fn an_empty_password_cannot_create_a_vault() {
        let (mut service, storage, _) = service();
        assert_eq!(
            service
                .create_vault("", KdfParams::MINIMUM)
                .unwrap_err()
                .code,
            IpcErrorCode::InvalidInput
        );
        assert_eq!(storage.bytes(), None);
    }

    #[test]
    fn a_wrong_password_is_reported_as_such() {
        let (mut service, _, _) = unlocked();
        service.lock();
        assert_eq!(
            service.unlock("wrong").unwrap_err().code,
            IpcErrorCode::WrongPassword
        );
        service.unlock(PASSWORD).unwrap();
    }

    #[test]
    fn added_entries_are_listed_with_codes_in_order() {
        let (mut service, _, _) = unlocked();
        let first = service.add_entry(totp_request("a")).unwrap();
        service.add_entry(totp_request("b")).unwrap();
        let views = service.list_entries().unwrap();
        assert_eq!(views.len(), 2);
        assert_eq!(
            (views[0].id.as_str(), views[0].account.as_str()),
            (first.as_str(), "a")
        );
        assert!(matches!(&views[1].code, CodeState::Ready { text, .. } if text.len() == 8));
    }

    #[test]
    fn listing_codes_follows_the_clock() {
        let (mut service, _, clock) = unlocked();
        service.add_entry(totp_request("a")).unwrap();
        let text = |service: &mut TestService| match &service.list_entries().unwrap()[0].code {
            CodeState::Ready { text, .. } => text.clone(),
            other => panic!("unexpected {other:?}"),
        };
        let first = text(&mut service);
        clock.advance_secs(30);
        assert_ne!(first, text(&mut service));
    }

    #[test]
    fn edit_move_and_delete_work_through_string_ids() {
        let (mut service, _, _) = unlocked();
        let first = service.add_entry(totp_request("a")).unwrap();
        let second = service.add_entry(totp_request("b")).unwrap();
        service.move_entry(&second, 0).unwrap();
        service
            .edit_entry(
                &first,
                EntryChangesRequest {
                    account: Some("renamed".into()),
                    ..no_changes()
                },
            )
            .unwrap();
        service.delete_entry(&second).unwrap();
        let views = service.list_entries().unwrap();
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].account, "renamed");
    }

    #[test]
    fn bad_ids_secrets_and_unknown_entries_are_input_errors() {
        let (mut service, _, _) = unlocked();
        assert_eq!(
            service.delete_entry("not-an-id").unwrap_err().code,
            IpcErrorCode::InvalidInput
        );
        let missing = "00000000-0000-0000-0000-000000000000";
        assert_eq!(
            service.delete_entry(missing).unwrap_err().code,
            IpcErrorCode::NotFound
        );
        let bad = NewEntryRequest {
            secret_base32: "!!".into(),
            ..totp_request("a")
        };
        assert_eq!(
            service.add_entry(bad).unwrap_err().code,
            IpcErrorCode::InvalidInput
        );
        let unknown = NewEntryRequest {
            otp_type: "vendor-x".into(),
            ..totp_request("a")
        };
        assert_eq!(
            service.add_entry(unknown).unwrap_err().code,
            IpcErrorCode::Unsupported
        );
    }

    #[test]
    fn everything_but_status_needs_an_unlocked_vault() {
        let (mut service, _, _) = unlocked();
        let id = service.add_entry(totp_request("a")).unwrap();
        service.lock();
        let locked = IpcErrorCode::Locked;
        assert_eq!(service.list_entries().unwrap_err().code, locked);
        assert_eq!(
            service.add_entry(totp_request("b")).unwrap_err().code,
            locked
        );
        assert_eq!(
            service.edit_entry(&id, no_changes()).unwrap_err().code,
            locked
        );
        assert_eq!(service.delete_entry(&id).unwrap_err().code, locked);
        assert_eq!(service.move_entry(&id, 0).unwrap_err().code, locked);
        assert_eq!(service.advance_counter(&id).unwrap_err().code, locked);
        assert_eq!(
            service.reveal_secret(&id, PASSWORD).err().unwrap().code,
            locked
        );
    }

    #[test]
    fn revealing_a_seed_needs_the_password_again() {
        let (mut service, _, _) = unlocked();
        let id = service.add_entry(totp_request("a")).unwrap();
        assert_eq!(
            service.reveal_secret(&id, "wrong").err().unwrap().code,
            IpcErrorCode::WrongPassword
        );
        let revealed = service.reveal_secret(&id, PASSWORD).unwrap();
        assert_eq!(revealed.secret_base32, "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
    }

    #[test]
    fn revealing_an_unknown_entry_is_not_found_after_the_password_check() {
        let (mut service, _, _) = unlocked();
        let missing = "00000000-0000-0000-0000-000000000000";
        assert_eq!(
            service.reveal_secret(missing, PASSWORD).err().unwrap().code,
            IpcErrorCode::NotFound
        );
    }

    #[test]
    fn hotp_entries_advance_on_request() {
        let (mut service, _, _) = unlocked();
        let request = NewEntryRequest {
            otp_type: "hotp".into(),
            ..totp_request("a")
        };
        let id = service.add_entry(request).unwrap();
        let before = service.list_entries().unwrap();
        service.advance_counter(&id).unwrap();
        let after = service.list_entries().unwrap();
        let text = |views: &[EntryView]| match &views[0].code {
            CodeState::Ready {
                text,
                period_seconds,
                ..
            } => {
                assert_eq!(*period_seconds, None);
                text.clone()
            }
            other => panic!("unexpected {other:?}"),
        };
        assert_ne!(text(&before), text(&after));
    }

    #[test]
    fn idle_locking_and_activity_come_from_the_session() {
        let (mut service, _, clock) = unlocked();
        clock.advance_secs(200);
        service.record_activity();
        clock.advance_secs(200);
        assert!(service.status().unwrap().unlocked);
        clock.advance_secs(100);
        service.tick();
        assert!(!service.status().unwrap().unlocked);
    }

    #[test]
    fn backgrounding_locks_by_default() {
        let (mut service, _, _) = unlocked();
        service.notify(SystemEvent::Backgrounded);
        assert!(!service.status().unwrap().unlocked);
    }

    #[test]
    fn a_storage_failure_surfaces_as_a_storage_error() {
        let (mut service, storage, _) = unlocked();
        storage.set_failing(true);
        assert_eq!(
            service.add_entry(totp_request("a")).unwrap_err().code,
            IpcErrorCode::StorageFailed
        );
    }

    #[test]
    fn changing_the_password_needs_the_current_one() {
        let (mut service, _, _) = unlocked();
        assert_eq!(
            service
                .change_password("wrong", "new password", KdfParams::MINIMUM)
                .unwrap_err()
                .code,
            IpcErrorCode::WrongPassword
        );
        assert_eq!(
            service
                .change_password(PASSWORD, "", KdfParams::MINIMUM)
                .unwrap_err()
                .code,
            IpcErrorCode::InvalidInput
        );
        service
            .change_password(PASSWORD, "new password", KdfParams::MINIMUM)
            .unwrap();
        service.lock();
        service.unlock("new password").unwrap();
    }
}
