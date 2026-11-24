//! What each IPC command does, independent of Tauri so it can be tested directly.

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use mfa_core::otp::OtpRegistry;
use mfa_core::vault::{
    Clock, EntryChanges, KdfParams, LockPolicy, NewEntry, Rng, Storage, SystemEvent, VaultSession,
};

use mfa_formats::otpauth::parse_otpauth;
use mfa_formats::qr::{MAX_FILE_BYTES, decode_qr_image};

use super::dto::{
    DraftEditsRequest, DraftView, EntryChangesRequest, EntryView, NewEntryRequest, OtpTypeView,
    RevealedSecret, VaultStatus, apply_edits, draft_views, entry_views, otp_type_views,
    parse_entry_id,
};
use super::error::{IpcError, IpcErrorCode};

pub struct VaultService<S: Storage, C: Clock, R: Rng> {
    session: VaultSession<S, C, R>,
    registry: OtpRegistry,
    /// Accounts read from links or QR codes, awaiting confirmation. Dropped when the vault locks.
    drafts: Vec<NewEntry>,
}

impl<S: Storage, C: Clock, R: Rng> VaultService<S, C, R> {
    pub fn new(storage: S, clock: C, rng: R) -> Self {
        Self {
            session: VaultSession::new(storage, clock, rng, LockPolicy::default()),
            registry: OtpRegistry::with_builtins(),
            drafts: Vec::new(),
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

    /// The OTP types a form can offer; needs no unlocked vault.
    pub fn otp_types(&self) -> Vec<OtpTypeView> {
        otp_type_views(&self.registry)
    }

    /// Sets how long the vault may sit idle before it locks; `None` turns the idle lock off.
    /// Locking on background and suspend stays as the default policy says.
    pub fn set_idle_lock(&mut self, seconds: Option<u32>) -> Result<(), IpcError> {
        if seconds == Some(0) {
            return Err(IpcError::invalid_input(
                "the idle time must be at least one second",
            ));
        }
        self.session.set_policy(LockPolicy {
            idle_timeout: seconds.map(|s| Duration::from_secs(u64::from(s))),
            ..LockPolicy::default()
        });
        Ok(())
    }

    pub fn lock(&mut self) {
        self.session.lock();
        self.drafts.clear();
    }

    pub fn notify(&mut self, event: SystemEvent) {
        self.session.notify(event);
        self.drop_drafts_if_locked();
    }

    pub fn tick(&mut self) {
        self.session.tick();
        self.drop_drafts_if_locked();
    }

    fn drop_drafts_if_locked(&mut self) {
        if !self.session.is_unlocked() {
            self.drafts.clear();
        }
    }

    fn require_unlocked(&mut self) -> Result<(), IpcError> {
        self.drop_drafts_if_locked();
        if self.session.is_unlocked() {
            Ok(())
        } else {
            Err(IpcError::new(IpcErrorCode::Locked))
        }
    }

    /// Reads an `otpauth://` link into a draft, replacing any earlier drafts.
    pub fn draft_from_link(&mut self, text: &str) -> Result<Vec<DraftView>, IpcError> {
        self.require_unlocked()?;
        let draft = parse_otpauth(text, &self.registry)?;
        self.drafts = vec![draft];
        Ok(draft_views(&self.drafts))
    }

    /// Reads every authenticator QR code in a PNG or JPEG image (given as base64) into drafts,
    /// replacing any earlier drafts. Other QR codes in the image are ignored.
    pub fn draft_from_image(&mut self, image_base64: &str) -> Result<Vec<DraftView>, IpcError> {
        self.require_unlocked()?;
        let bytes = decode_base64_image(image_base64)?;
        let texts = decode_qr_image(&bytes)?;
        let mut drafts = Vec::new();
        let mut first_problem = None;
        for text in texts {
            match parse_otpauth(&text, &self.registry) {
                Ok(draft) => drafts.push(draft),
                Err(error) => {
                    first_problem.get_or_insert(error);
                }
            }
        }
        if drafts.is_empty() {
            return Err(first_problem.map_or_else(
                || IpcError::invalid_input("the QR code is not an authenticator link"),
                IpcError::from,
            ));
        }
        self.drafts = drafts;
        Ok(draft_views(&self.drafts))
    }

    /// Adds a draft with the user's edits; on success all drafts are dropped.
    pub fn add_draft(
        &mut self,
        draft_id: u32,
        edits: DraftEditsRequest,
    ) -> Result<String, IpcError> {
        self.require_unlocked()?;
        let draft = usize::try_from(draft_id)
            .ok()
            .and_then(|index| self.drafts.get(index))
            .ok_or(IpcError::new(IpcErrorCode::NotFound))?;
        let new = apply_edits(draft, edits);
        let id = self.session.add_entry(new, &self.registry)?.to_string();
        self.drafts.clear();
        Ok(id)
    }

    pub fn discard_drafts(&mut self) {
        self.drafts.clear();
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

/// Base64 text is about a third longer than the bytes it carries; anything that could not
/// hold an image within the decoder's file-size limit is refused before decoding.
fn decode_base64_image(text: &str) -> Result<Vec<u8>, IpcError> {
    if text.len() > MAX_FILE_BYTES / 3 * 4 + 8 {
        return Err(IpcError::invalid_input("the file is larger than 10 MiB"));
    }
    STANDARD
        .decode(text.trim())
        .map_err(|_| IpcError::invalid_input("the image could not be read"))
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
    fn otp_types_are_available_even_while_locked() {
        let (service, _, _) = service();
        assert_eq!(service.otp_types().len(), 3);
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
    fn the_idle_lock_time_can_be_changed_or_switched_off() {
        let (mut service, _, clock) = unlocked();
        service.set_idle_lock(Some(60)).unwrap();
        clock.advance_secs(59);
        assert!(service.status().unwrap().unlocked);
        clock.advance_secs(2);
        assert!(!service.status().unwrap().unlocked);

        let (mut service, _, clock) = unlocked();
        service.set_idle_lock(None).unwrap();
        clock.advance_secs(1_000_000);
        assert!(service.status().unwrap().unlocked);
    }

    #[test]
    fn a_zero_idle_time_is_refused_and_changes_nothing() {
        let (mut service, _, clock) = unlocked();
        assert_eq!(
            service.set_idle_lock(Some(0)).unwrap_err().code,
            IpcErrorCode::InvalidInput
        );
        clock.advance_secs(200);
        assert!(
            service.status().unwrap().unlocked,
            "the default five minutes still applies"
        );
    }

    #[test]
    fn changing_the_idle_time_keeps_locking_on_background() {
        let (mut service, _, _) = unlocked();
        service.set_idle_lock(None).unwrap();
        service.notify(SystemEvent::Backgrounded);
        assert!(!service.status().unwrap().unlocked);
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

    const LINK_SECRET: &str = "JBSWY3DPEHPK3PXP";

    fn link(account: &str) -> String {
        format!("otpauth://totp/Example:{account}?secret={LINK_SECRET}&issuer=Example&digits=8")
    }

    fn edits(account: &str) -> DraftEditsRequest {
        DraftEditsRequest {
            issuer: "Example".into(),
            account: account.into(),
            params: [("digits".to_owned(), "8".to_owned())].into(),
            tags: vec![],
            notes: String::new(),
        }
    }

    /// A PNG of a QR code per text, side by side, as base64.
    fn qr_png_base64(texts: &[&str]) -> String {
        let scale = 6;
        let codes: Vec<_> = texts
            .iter()
            .map(|text| qrcode::QrCode::new(text.as_bytes()).unwrap())
            .collect();
        let modules = codes.iter().map(qrcode::QrCode::width).max().unwrap();
        let cell = modules + 8;
        let (width, height) = (cell * scale * codes.len(), cell * scale);
        let mut pixels = vec![255_u8; width * height];
        for (index, code) in codes.iter().enumerate() {
            let colours = code.to_colors();
            for y in 0..height {
                for x in 0..cell * scale {
                    let (mx, my) = (x / scale, y / scale);
                    let inside =
                        (4..code.width() + 4).contains(&mx) && (4..code.width() + 4).contains(&my);
                    if inside && colours[(my - 4) * code.width() + (mx - 4)] == qrcode::Color::Dark
                    {
                        pixels[y * width + index * cell * scale + x] = 0;
                    }
                }
            }
        }
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(
            &mut out,
            u32::try_from(width).unwrap(),
            u32::try_from(height).unwrap(),
        );
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        STANDARD.encode(out)
    }

    #[test]
    fn a_pasted_link_becomes_a_draft_without_its_secret() {
        let (mut service, _, _) = unlocked();
        let drafts = service.draft_from_link(&link("alice")).unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(
            (
                drafts[0].draft_id,
                drafts[0].otp_type.as_str(),
                drafts[0].account.as_str()
            ),
            (0, "totp", "alice")
        );
        assert_eq!(
            drafts[0].params.get("digits").map(String::as_str),
            Some("8")
        );
        let json = serde_json::to_string(&drafts).unwrap();
        let hex: String = b"Hello!\xde\xad\xbe\xef"
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert!(!json.contains(LINK_SECRET) && !json.contains(&hex));
    }

    #[test]
    fn adding_a_draft_stores_the_secret_from_the_link_and_the_users_edits() {
        let (mut service, _, _) = unlocked();
        service.draft_from_link(&link("alice")).unwrap();
        let id = service
            .add_draft(
                0,
                DraftEditsRequest {
                    tags: vec!["work".into()],
                    ..edits("alice renamed")
                },
            )
            .unwrap();
        let views = service.list_entries().unwrap();
        assert_eq!(views.len(), 1);
        assert_eq!(
            (views[0].id.as_str(), views[0].account.as_str()),
            (id.as_str(), "alice renamed")
        );
        assert_eq!(views[0].tags, ["work"]);
        assert_eq!(
            service
                .reveal_secret(&id, PASSWORD)
                .ok()
                .unwrap()
                .secret_base32,
            LINK_SECRET
        );
    }

    #[test]
    fn a_rejected_edit_keeps_the_draft_for_another_try() {
        let (mut service, _, _) = unlocked();
        service.draft_from_link(&link("alice")).unwrap();
        let bad = DraftEditsRequest {
            params: [("digits".to_owned(), "99".to_owned())].into(),
            ..edits("alice")
        };
        assert_eq!(
            service.add_draft(0, bad).unwrap_err().code,
            IpcErrorCode::InvalidInput
        );
        assert!(service.add_draft(0, edits("alice")).is_ok());
    }

    #[test]
    fn a_draft_can_be_added_only_once_and_unknown_drafts_are_not_found() {
        let (mut service, _, _) = unlocked();
        service.draft_from_link(&link("alice")).unwrap();
        assert_eq!(
            service.add_draft(5, edits("x")).unwrap_err().code,
            IpcErrorCode::NotFound
        );
        service.add_draft(0, edits("alice")).unwrap();
        assert_eq!(
            service.add_draft(0, edits("alice")).unwrap_err().code,
            IpcErrorCode::NotFound
        );
    }

    #[test]
    fn a_new_draft_replaces_the_old_one_and_discarding_drops_it() {
        let (mut service, _, _) = unlocked();
        service.draft_from_link(&link("first")).unwrap();
        let second = service.draft_from_link(&link("second")).unwrap();
        assert_eq!(second[0].account, "second");
        service.discard_drafts();
        assert_eq!(
            service.add_draft(0, edits("x")).unwrap_err().code,
            IpcErrorCode::NotFound
        );
    }

    #[test]
    fn drafts_do_not_survive_locking_or_the_idle_lock() {
        let (mut service, _, clock) = unlocked();
        service.draft_from_link(&link("alice")).unwrap();
        service.lock();
        service.unlock(PASSWORD).unwrap();
        assert_eq!(
            service.add_draft(0, edits("x")).unwrap_err().code,
            IpcErrorCode::NotFound
        );

        service.draft_from_link(&link("alice")).unwrap();
        clock.advance_secs(301);
        service.tick();
        service.unlock(PASSWORD).unwrap();
        assert_eq!(
            service.add_draft(0, edits("x")).unwrap_err().code,
            IpcErrorCode::NotFound
        );
    }

    #[test]
    fn draft_operations_need_an_unlocked_vault() {
        let (mut service, _, _) = unlocked();
        service.lock();
        let locked = IpcErrorCode::Locked;
        assert_eq!(
            service.draft_from_link(&link("a")).unwrap_err().code,
            locked
        );
        assert_eq!(service.draft_from_image("AAAA").unwrap_err().code, locked);
        assert_eq!(service.add_draft(0, edits("a")).unwrap_err().code, locked);
    }

    #[test]
    fn a_bad_link_is_explained_without_repeating_its_secret() {
        let (mut service, _, _) = unlocked();
        let error = service
            .draft_from_link("otpauth://totp/me?secret=ZZtopSECRETvalue123")
            .unwrap_err();
        assert_eq!(error.code, IpcErrorCode::InvalidInput);
        assert!(!error.message.unwrap().contains("ZZtopSECRETvalue123"));
        assert_eq!(
            service
                .draft_from_link("hello")
                .unwrap_err()
                .message
                .as_deref(),
            Some("this is not an otpauth:// link")
        );
    }

    #[test]
    fn a_qr_image_becomes_drafts() {
        let (mut service, _, _) = unlocked();
        let drafts = service
            .draft_from_image(&qr_png_base64(&[&link("alice")]))
            .unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].account, "alice");
        let id = service.add_draft(0, edits("alice")).unwrap();
        assert_eq!(
            service
                .reveal_secret(&id, PASSWORD)
                .ok()
                .unwrap()
                .secret_base32,
            LINK_SECRET
        );
    }

    #[test]
    fn several_codes_in_one_image_give_several_drafts_and_other_codes_are_ignored() {
        let (mut service, _, _) = unlocked();
        let image = qr_png_base64(&[
            &link("alice"),
            "https://example.com/not-a-link",
            &link("bob"),
        ]);
        let drafts = service.draft_from_image(&image).unwrap();
        let mut accounts: Vec<_> = drafts.iter().map(|d| d.account.as_str()).collect();
        accounts.sort_unstable();
        assert_eq!(accounts, ["alice", "bob"]);
        assert_eq!(
            drafts.iter().map(|d| d.draft_id).collect::<Vec<_>>(),
            [0, 1]
        );
    }

    #[test]
    fn an_image_whose_only_code_is_not_a_link_is_refused_with_the_links_own_complaint() {
        let (mut service, _, _) = unlocked();
        let error = service
            .draft_from_image(&qr_png_base64(&["https://example.com"]))
            .unwrap_err();
        assert_eq!(
            error.message.as_deref(),
            Some("this is not an otpauth:// link")
        );
    }

    #[test]
    fn unreadable_images_are_refused_with_readable_messages() {
        let (mut service, _, _) = unlocked();
        let message = |service: &mut TestService, text: &str| {
            service.draft_from_image(text).unwrap_err().message.unwrap()
        };
        assert_eq!(
            message(&mut service, "not base64 !!"),
            "the image could not be read"
        );
        assert_eq!(
            message(&mut service, &STANDARD.encode(b"plain text")),
            "the file is not a PNG or JPEG image"
        );
        assert_eq!(
            message(&mut service, &"A".repeat(20_000_000)),
            "the file is larger than 10 MiB"
        );
    }

    #[test]
    fn a_failed_image_keeps_the_earlier_drafts() {
        let (mut service, _, _) = unlocked();
        service.draft_from_link(&link("alice")).unwrap();
        assert!(service.draft_from_image("garbage!!").is_err());
        assert!(service.add_draft(0, edits("alice")).is_ok());
    }
}
