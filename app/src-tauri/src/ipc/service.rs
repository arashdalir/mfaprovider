//! What each IPC command does, independent of Tauri so it can be tested directly.

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use mfa_core::otp::OtpRegistry;
use mfa_core::vault::{
    Clock, EntryChanges, KdfParams, LockPolicy, NewEntry, Rng, Storage, SystemEvent, VaultSession,
};
use zeroize::Zeroizing;

use mfa_formats::archive::{is_zip, read_single_file};
use mfa_formats::import::{MAX_IMPORT_BYTES, all_importers, importer};
use mfa_formats::merge::{Decision, MergeStatus, PlannedItem, build_ops, plan_merge};
use mfa_formats::otpauth::parse_otpauth;
use mfa_formats::qr::{MAX_FILE_BYTES, decode_qr_image};

use super::dto::{
    DraftEditsRequest, DraftView, EntryChangesRequest, EntryView, ImportChoice, ImportDecision,
    ImportFormatView, ImportItemView, ImportPreview, ImportResult, ImportStatus, NewEntryRequest,
    OtpTypeView, RevealedSecret, SkippedItemView, VaultStatus, apply_edits, draft_views,
    entry_views, otp_type_views, parse_entry_id,
};
use super::error::{IpcError, IpcErrorCode};

pub struct VaultService<S: Storage, C: Clock, R: Rng> {
    session: VaultSession<S, C, R>,
    registry: OtpRegistry,
    /// Accounts read from links or QR codes, awaiting confirmation. Dropped when the vault locks.
    drafts: Vec<NewEntry>,
    /// A file read for import, awaiting the user's choices. Dropped when the vault locks.
    pending_import: Option<PendingImport>,
}

struct PendingImport {
    definitions: Vec<NewEntry>,
}

impl<S: Storage, C: Clock, R: Rng> VaultService<S, C, R> {
    pub fn new(storage: S, clock: C, rng: R) -> Self {
        Self {
            session: VaultSession::new(storage, clock, rng, LockPolicy::default()),
            registry: OtpRegistry::with_builtins(),
            drafts: Vec::new(),
            pending_import: None,
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
        self.pending_import = None;
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
            self.pending_import = None;
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

    /// The file formats the import screen can offer; needs no unlocked vault.
    pub fn import_formats(&self) -> Vec<ImportFormatView> {
        all_importers()
            .iter()
            .map(|format| ImportFormatView {
                id: format.id().to_owned(),
                label: format.label().to_owned(),
            })
            .collect()
    }

    /// Reads a file's text with the chosen format and says what importing it would do. Nothing
    /// in the vault changes; the file's accounts wait here (replacing any earlier import).
    pub fn import_preview(
        &mut self,
        format: &str,
        file: &[u8],
        password: &str,
    ) -> Result<ImportPreview, IpcError> {
        self.require_unlocked()?;
        let format =
            importer(format).ok_or_else(|| IpcError::invalid_input("unknown file format"))?;
        let content = file_content(file, password)?;
        let batch = format
            .parse(&content, &self.registry)
            .map_err(|error| IpcError::invalid_input(error.to_string()))?;
        let skipped = batch
            .skipped
            .iter()
            .map(|item| SkippedItemView {
                position: saturating_position(item.position),
                reason: item.reason.clone(),
            })
            .collect();
        let (positions, definitions): (Vec<_>, Vec<_>) = batch
            .entries
            .into_iter()
            .map(|entry| (entry.position, entry.definition))
            .unzip();
        let items = self.item_views(&definitions, &positions)?;
        self.pending_import = Some(PendingImport { definitions });
        Ok(ImportPreview { items, skipped })
    }

    fn item_views(
        &mut self,
        definitions: &[NewEntry],
        positions: &[usize],
    ) -> Result<Vec<ImportItemView>, IpcError> {
        let data = self.session.data()?;
        let statuses = plan_merge(data, definitions);
        let name_of = |id| {
            data.entry(id)
                .map(|entry| format!("{} · {}", entry.issuer, entry.account))
        };
        Ok(definitions
            .iter()
            .zip(positions)
            .zip(&statuses)
            .enumerate()
            .map(|(index, ((definition, position), status))| {
                let (view_status, matches, can_replace) = match *status {
                    MergeStatus::New => (ImportStatus::New, None, false),
                    MergeStatus::DuplicateOfVault(id) => {
                        (ImportStatus::DuplicateInVault, name_of(id), true)
                    }
                    MergeStatus::ConflictWithVault(id) => {
                        (ImportStatus::Conflict, name_of(id), true)
                    }
                    MergeStatus::SameSecretUnderOtherName(id) => {
                        (ImportStatus::AlreadyUnderOtherName, name_of(id), true)
                    }
                    MergeStatus::DuplicateOfEarlierItem(earlier) => (
                        ImportStatus::DuplicateInFile,
                        Some(format!("line {}", positions[earlier])),
                        false,
                    ),
                };
                ImportItemView {
                    item_id: saturating_position(index),
                    position: saturating_position(*position),
                    otp_type: definition.otp_type.clone(),
                    issuer: definition.issuer.clone(),
                    account: definition.account.clone(),
                    status: view_status,
                    matches,
                    can_replace,
                    default_decision: decision_to_view(Decision::default_for(*status)),
                }
            })
            .collect())
    }

    /// Applies the user's choices to the previewed file as one saved change. Items without a
    /// choice are skipped. The comparison with the vault is redone first, so a vault that
    /// changed since the preview is respected. On success the pending file is dropped.
    pub fn import_apply(&mut self, choices: Vec<ImportChoice>) -> Result<ImportResult, IpcError> {
        self.require_unlocked()?;
        let pending = self
            .pending_import
            .as_ref()
            .ok_or(IpcError::new(IpcErrorCode::NotFound))?;
        let decisions = decisions_by_item(&choices, pending.definitions.len())?;
        let statuses = plan_merge(self.session.data()?, &pending.definitions);
        let items: Vec<PlannedItem> = pending
            .definitions
            .iter()
            .zip(statuses)
            .zip(&decisions)
            .map(|((definition, status), decision)| PlannedItem {
                definition: copy_definition(definition),
                status,
                decision: *decision,
            })
            .collect();
        let total = items.len();
        let session = &mut self.session;
        let ops = build_ops(items, || session.new_entry_id())
            .map_err(|error| IpcError::invalid_input(error.to_string()))?;
        let replaced = ops
            .iter()
            .filter(|op| matches!(op, mfa_core::vault::ImportOp::Replace { .. }))
            .count();
        let added = ops.len() - replaced;
        let outcome = self.session.import(ops, &self.registry);
        if outcome.is_ok() || self.session.is_dirty() {
            // Applied (saved, or in memory awaiting a save): never offer the same file again.
            self.pending_import = None;
        }
        outcome?;
        Ok(ImportResult {
            added: saturating_position(added),
            replaced: saturating_position(replaced),
            skipped: saturating_position(total - added - replaced),
        })
    }

    pub fn import_discard(&mut self) {
        self.pending_import = None;
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

fn saturating_position(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn decision_to_view(decision: Decision) -> ImportDecision {
    match decision {
        Decision::Skip => ImportDecision::Skip,
        Decision::Add => ImportDecision::Add,
        Decision::Replace => ImportDecision::Replace,
    }
}

fn decision_from_view(decision: ImportDecision) -> Decision {
    match decision {
        ImportDecision::Skip => Decision::Skip,
        ImportDecision::Add => Decision::Add,
        ImportDecision::Replace => Decision::Replace,
    }
}

/// One decision per item; items without a choice are skipped. Unknown or repeated item ids are
/// refused rather than guessed at.
fn decisions_by_item(choices: &[ImportChoice], count: usize) -> Result<Vec<Decision>, IpcError> {
    let mut decisions = vec![Decision::Skip; count];
    let mut seen = vec![false; count];
    for choice in choices {
        let index = usize::try_from(choice.item_id)
            .ok()
            .filter(|index| *index < count)
            .ok_or_else(|| {
                IpcError::invalid_input("the choice names an item that is not in the file")
            })?;
        if std::mem::replace(&mut seen[index], true) {
            return Err(IpcError::invalid_input("an item has more than one choice"));
        }
        decisions[index] = decision_from_view(choice.decision);
    }
    Ok(decisions)
}

/// A copy whose secret is wiped when it drops, so a refused import does not lose the file.
fn copy_definition(definition: &NewEntry) -> NewEntry {
    NewEntry {
        otp_type: definition.otp_type.clone(),
        issuer: definition.issuer.clone(),
        account: definition.account.clone(),
        secret: mfa_core::secret::Secret::new(definition.secret.as_bytes().to_vec()),
        params: definition.params.clone(),
        icon: definition.icon.clone(),
        tags: definition.tags.clone(),
        notes: definition.notes.clone(),
    }
}

/// The bytes an importer reads: the file itself, or the one file inside a ZIP archive (opened
/// with `password`, which is used here and nowhere else). Archives are recognised by content.
fn file_content(file: &[u8], password: &str) -> Result<Zeroizing<Vec<u8>>, IpcError> {
    if is_zip(file) {
        Ok(read_single_file(file, password.as_bytes())?)
    } else {
        Ok(Zeroizing::new(file.to_vec()))
    }
}

/// Base64 text is about a third longer than the bytes it carries; anything that could not
/// hold an image within the decoder's file-size limit is refused before decoding.
fn decode_base64_image(text: &str) -> Result<Vec<u8>, IpcError> {
    decode_base64_within(
        text,
        MAX_FILE_BYTES,
        (
            "the file is larger than 10 MiB",
            "the image could not be read",
        ),
    )
}

/// An import file (a text export or a ZIP archive) sent as base64.
pub(super) fn decode_base64_file(text: &str) -> Result<Vec<u8>, IpcError> {
    decode_base64_within(
        text,
        MAX_IMPORT_BYTES,
        (
            "the file is larger than 5 MiB",
            "the file could not be read",
        ),
    )
}

fn decode_base64_within(
    text: &str,
    max_bytes: usize,
    (too_large, unreadable): (&'static str, &'static str),
) -> Result<Vec<u8>, IpcError> {
    if text.len() > max_bytes / 3 * 4 + 8 {
        return Err(IpcError::invalid_input(too_large));
    }
    STANDARD
        .decode(text.trim())
        .map_err(|_| IpcError::invalid_input(unreadable))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use mfa_core::vault::MemoryStorage;

    use super::*;

    impl<S: Storage, C: Clock, R: Rng> VaultService<S, C, R> {
        fn preview_text(&mut self, format: &str, text: &str) -> Result<ImportPreview, IpcError> {
            self.import_preview(format, text.as_bytes(), "")
        }
    }
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

    /// Deterministic pseudo-random bytes that do not repeat within a test (a plain counter
    /// wraps after 256 bytes, which made generated entry ids collide).
    struct CountingRng(u64);

    impl Rng for CountingRng {
        fn fill_bytes(&mut self, destination: &mut [u8]) {
            for byte in destination {
                self.0 = self
                    .0
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                *byte = self.0.to_be_bytes()[0];
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

    const FORMAT: &str = "otpauth-list";
    const SECRET_X: &str = "JBSWY3DPEHPK3PXP";
    const SECRET_Y: &str = "MZXW6YTBOI======";

    fn export(lines: &[(&str, &str)]) -> String {
        lines
            .iter()
            .map(|(name, secret)| {
                format!("otpauth://totp/{name}?secret={secret}&digits=6&icon=WinAuth\r\n")
            })
            .collect()
    }

    fn choice(item_id: u32, decision: ImportDecision) -> ImportChoice {
        ImportChoice { item_id, decision }
    }

    fn accept_defaults(preview: &ImportPreview) -> Vec<ImportChoice> {
        preview
            .items
            .iter()
            .map(|i| choice(i.item_id, i.default_decision))
            .collect()
    }

    fn accounts(service: &mut TestService) -> Vec<String> {
        service
            .list_entries()
            .unwrap()
            .into_iter()
            .map(|e| e.account)
            .collect()
    }

    #[test]
    fn the_formats_are_listed_without_needing_the_vault() {
        let (service, _, _) = service();
        let formats = service.import_formats();
        assert_eq!(formats.len(), 1);
        assert_eq!(formats[0].id, FORMAT);
    }

    #[test]
    fn a_preview_lists_the_accounts_and_the_unreadable_lines_and_changes_nothing() {
        let (mut service, storage, _) = unlocked();
        let before = storage.bytes();
        let file = format!(
            "{}garbage line\r\n",
            export(&[("one", SECRET_X), ("Acme:two", SECRET_Y)])
        );
        let preview = service.preview_text(FORMAT, &file).unwrap();
        assert_eq!(preview.items.len(), 2);
        assert_eq!(
            (
                preview.items[0].item_id,
                preview.items[0].position,
                preview.items[0].account.as_str()
            ),
            (0, 1, "one")
        );
        assert_eq!(
            (
                preview.items[1].issuer.as_str(),
                preview.items[1].account.as_str()
            ),
            ("Acme", "two")
        );
        assert!(preview.items.iter().all(|i| i.status == ImportStatus::New
            && i.default_decision == ImportDecision::Add
            && !i.can_replace));
        assert_eq!(preview.skipped.len(), 1);
        assert_eq!(
            (
                preview.skipped[0].position,
                preview.skipped[0].reason.as_str()
            ),
            (3, "this is not an otpauth:// link")
        );
        assert_eq!(storage.bytes(), before);
        assert_eq!(accounts(&mut service).len(), 0);
    }

    #[test]
    fn a_preview_never_contains_a_secret() {
        let (mut service, _, _) = unlocked();
        let preview = service
            .preview_text(FORMAT, &export(&[("one", SECRET_X)]))
            .unwrap();
        let json = serde_json::to_string(&preview).unwrap();
        assert!(!json.contains(SECRET_X) && !json.contains("JBSWY3DP"));
    }

    #[test]
    fn applying_the_defaults_adds_everything_in_file_order() {
        let (mut service, _, _) = unlocked();
        let preview = service
            .preview_text(
                FORMAT,
                &export(&[("a", SECRET_X), ("b", SECRET_Y), ("c", SECRET_X)]),
            )
            .unwrap();
        let result = service.import_apply(accept_defaults(&preview)).unwrap();
        assert_eq!(
            result,
            ImportResult {
                added: 3,
                replaced: 0,
                skipped: 0
            }
        );
        assert_eq!(accounts(&mut service), ["a", "b", "c"]);
    }

    #[test]
    fn duplicates_are_recognised_against_the_vault_and_inside_the_file() {
        let (mut service, _, _) = unlocked();
        service.add_entry(totp_request("a")).unwrap();
        let file = export(&[
            ("Example:a", "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"),
            ("x", SECRET_X),
            ("x", SECRET_X),
        ]);
        let preview = service.preview_text(FORMAT, &file).unwrap();
        let statuses: Vec<_> = preview.items.iter().map(|i| i.status).collect();
        assert_eq!(
            statuses,
            [
                ImportStatus::DuplicateInVault,
                ImportStatus::New,
                ImportStatus::DuplicateInFile
            ]
        );
        assert_eq!(preview.items[0].matches.as_deref(), Some("Example · a"));
        assert_eq!(preview.items[2].matches.as_deref(), Some("line 2"));
        assert!(preview.items[0].can_replace && !preview.items[2].can_replace);
        let result = service.import_apply(accept_defaults(&preview)).unwrap();
        assert_eq!(
            result,
            ImportResult {
                added: 1,
                replaced: 0,
                skipped: 2
            }
        );
        assert_eq!(accounts(&mut service), ["a", "x"]);
    }

    #[test]
    fn a_conflict_can_replace_the_existing_entry_or_keep_both() {
        let (mut service, _, _) = unlocked();
        let existing = service.add_entry(totp_request("a")).unwrap();
        let file = export(&[("Example:a", SECRET_X)]);

        let preview = service.preview_text(FORMAT, &file).unwrap();
        assert_eq!(preview.items[0].status, ImportStatus::Conflict);
        assert_eq!(preview.items[0].default_decision, ImportDecision::Add);
        let result = service
            .import_apply(vec![choice(0, ImportDecision::Replace)])
            .unwrap();
        assert_eq!(
            result,
            ImportResult {
                added: 0,
                replaced: 1,
                skipped: 0
            }
        );
        assert_eq!(accounts(&mut service), ["a"]);
        assert_eq!(
            service
                .reveal_secret(&existing, PASSWORD)
                .ok()
                .unwrap()
                .secret_base32,
            SECRET_X
        );

        let again = service
            .preview_text(FORMAT, &export(&[("Example:a", SECRET_Y)]))
            .unwrap();
        assert_eq!(again.items[0].status, ImportStatus::Conflict);
        service
            .import_apply(vec![choice(0, ImportDecision::Add)])
            .unwrap();
        assert_eq!(accounts(&mut service), ["a", "a"]);
    }

    #[test]
    fn items_without_a_choice_are_skipped_so_a_partial_import_is_possible() {
        let (mut service, _, _) = unlocked();
        service
            .preview_text(
                FORMAT,
                &export(&[("a", SECRET_X), ("b", SECRET_Y), ("c", SECRET_X)]),
            )
            .unwrap();
        let result = service
            .import_apply(vec![
                choice(1, ImportDecision::Add),
                choice(2, ImportDecision::Skip),
            ])
            .unwrap();
        assert_eq!(
            result,
            ImportResult {
                added: 1,
                replaced: 0,
                skipped: 2
            }
        );
        assert_eq!(accounts(&mut service), ["b"]);
    }

    #[test]
    fn nonsense_choices_are_refused_and_the_file_stays_available() {
        let (mut service, _, _) = unlocked();
        service
            .preview_text(FORMAT, &export(&[("a", SECRET_X)]))
            .unwrap();
        let invalid = IpcErrorCode::InvalidInput;
        assert_eq!(
            service
                .import_apply(vec![choice(5, ImportDecision::Add)])
                .unwrap_err()
                .code,
            invalid
        );
        assert_eq!(
            service
                .import_apply(vec![
                    choice(0, ImportDecision::Add),
                    choice(0, ImportDecision::Skip)
                ])
                .unwrap_err()
                .code,
            invalid
        );
        assert_eq!(
            service
                .import_apply(vec![choice(0, ImportDecision::Replace)])
                .unwrap_err()
                .code,
            invalid,
            "nothing to replace"
        );
        assert_eq!(accounts(&mut service).len(), 0);
        assert!(
            service
                .import_apply(vec![choice(0, ImportDecision::Add)])
                .is_ok()
        );
    }

    #[test]
    fn two_items_replacing_one_entry_are_refused_before_anything_changes() {
        let (mut service, _, _) = unlocked();
        service.add_entry(totp_request("a")).unwrap();
        service
            .preview_text(
                FORMAT,
                &export(&[("Example:a", SECRET_X), ("Example:a", SECRET_Y)]),
            )
            .unwrap();
        let error = service
            .import_apply(vec![
                choice(0, ImportDecision::Replace),
                choice(1, ImportDecision::Replace),
            ])
            .unwrap_err();
        assert_eq!(error.code, IpcErrorCode::InvalidInput);
        assert_eq!(accounts(&mut service), ["a"]);
    }

    #[test]
    fn a_file_can_be_applied_only_once() {
        let (mut service, _, _) = unlocked();
        let preview = service
            .preview_text(FORMAT, &export(&[("a", SECRET_X)]))
            .unwrap();
        service.import_apply(accept_defaults(&preview)).unwrap();
        assert_eq!(
            service
                .import_apply(accept_defaults(&preview))
                .unwrap_err()
                .code,
            IpcErrorCode::NotFound
        );
        assert_eq!(accounts(&mut service).len(), 1);
    }

    #[test]
    fn a_new_preview_replaces_the_old_one_and_discarding_drops_it() {
        let (mut service, _, _) = unlocked();
        service
            .preview_text(FORMAT, &export(&[("old", SECRET_X)]))
            .unwrap();
        service
            .preview_text(FORMAT, &export(&[("new", SECRET_Y)]))
            .unwrap();
        service
            .import_apply(vec![choice(0, ImportDecision::Add)])
            .unwrap();
        assert_eq!(accounts(&mut service), ["new"]);
        service
            .preview_text(FORMAT, &export(&[("x", SECRET_X)]))
            .unwrap();
        service.import_discard();
        assert_eq!(
            service
                .import_apply(vec![choice(0, ImportDecision::Add)])
                .unwrap_err()
                .code,
            IpcErrorCode::NotFound
        );
    }

    #[test]
    fn the_pending_file_does_not_survive_locking_or_the_idle_lock() {
        let (mut service, _, clock) = unlocked();
        service
            .preview_text(FORMAT, &export(&[("a", SECRET_X)]))
            .unwrap();
        service.lock();
        service.unlock(PASSWORD).unwrap();
        assert_eq!(
            service
                .import_apply(vec![choice(0, ImportDecision::Add)])
                .unwrap_err()
                .code,
            IpcErrorCode::NotFound
        );

        service
            .preview_text(FORMAT, &export(&[("a", SECRET_X)]))
            .unwrap();
        clock.advance_secs(301);
        service.tick();
        service.unlock(PASSWORD).unwrap();
        assert_eq!(
            service
                .import_apply(vec![choice(0, ImportDecision::Add)])
                .unwrap_err()
                .code,
            IpcErrorCode::NotFound
        );
    }

    #[test]
    fn importing_needs_an_unlocked_vault() {
        let (mut service, _, _) = unlocked();
        service.lock();
        assert_eq!(
            service.preview_text(FORMAT, "x").unwrap_err().code,
            IpcErrorCode::Locked
        );
        assert_eq!(
            service.import_apply(Vec::new()).unwrap_err().code,
            IpcErrorCode::Locked
        );
    }

    #[test]
    fn unknown_formats_and_unreadable_files_are_explained() {
        let (mut service, _, _) = unlocked();
        assert_eq!(
            service
                .preview_text("nonsense", "x")
                .unwrap_err()
                .message
                .as_deref(),
            Some("unknown file format")
        );
        assert_eq!(
            service
                .preview_text(FORMAT, "")
                .unwrap_err()
                .message
                .as_deref(),
            Some("no accounts were found in the file")
        );
        let report = service.preview_text(FORMAT, "hello\nworld\n").unwrap();
        assert_eq!((report.items.len(), report.skipped.len()), (0, 2));
    }

    #[test]
    fn a_save_failure_surfaces_and_the_file_is_not_offered_again() {
        let (mut service, storage, _) = unlocked();
        let preview = service
            .preview_text(FORMAT, &export(&[("a", SECRET_X)]))
            .unwrap();
        storage.set_failing(true);
        assert_eq!(
            service
                .import_apply(accept_defaults(&preview))
                .unwrap_err()
                .code,
            IpcErrorCode::StorageFailed
        );
        storage.set_failing(false);
        assert_eq!(
            service
                .import_apply(accept_defaults(&preview))
                .unwrap_err()
                .code,
            IpcErrorCode::NotFound
        );
    }

    #[test]
    fn an_import_of_many_accounts_is_one_saved_step() {
        let (mut service, storage, _) = unlocked();
        let lines: Vec<(String, String)> = (0..58)
            .map(|n| (format!("acct{n}"), SECRET_X.to_owned()))
            .collect();
        let borrowed: Vec<(&str, &str)> = lines
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        let preview = service.preview_text(FORMAT, &export(&borrowed)).unwrap();
        let before = storage.bytes();
        service.import_apply(accept_defaults(&preview)).unwrap();
        assert_ne!(storage.bytes(), before);
        assert_eq!(accounts(&mut service).len(), 58);
    }

    #[test]
    fn an_account_under_another_name_is_flagged_skipped_by_default_and_replaceable() {
        let (mut service, _, _) = unlocked();
        let existing = service.add_entry(totp_request("a")).unwrap();
        let file = export(&[("renamed", "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ")]);
        let preview = service.preview_text(FORMAT, &file).unwrap();
        let item = &preview.items[0];
        assert_eq!(item.status, ImportStatus::AlreadyUnderOtherName);
        assert_eq!(item.matches.as_deref(), Some("Example · a"));
        assert!(item.can_replace);
        assert_eq!(item.default_decision, ImportDecision::Skip);

        service
            .import_apply(vec![choice(0, ImportDecision::Replace)])
            .unwrap();
        let entries = service.list_entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            (entries[0].id.as_str(), entries[0].account.as_str()),
            (existing.as_str(), "renamed")
        );
    }

    fn protected_archive(content: &str, password: &[u8]) -> Vec<u8> {
        use std::io::Write;
        use zip::CompressionMethod;
        use zip::unstable::write::FileOptionsExt;
        use zip::write::{FileOptions, ZipWriter};

        let options = FileOptions::<'_, '_, ()>::default()
            .compression_method(CompressionMethod::Deflated)
            .with_deprecated_encryption(password)
            .unwrap();
        let mut writer = ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer.start_file("export.txt", options).unwrap();
        writer.write_all(content.as_bytes()).unwrap();
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn a_password_protected_archive_previews_like_the_text_file_inside() {
        let (mut service, _, _) = unlocked();
        let zip = protected_archive(&export(&[("one", SECRET_X), ("two", SECRET_Y)]), b"pw-1");
        let from_zip = service.import_preview(FORMAT, &zip, "pw-1").unwrap();
        let from_text = service
            .preview_text(FORMAT, &export(&[("one", SECRET_X), ("two", SECRET_Y)]))
            .unwrap();
        assert_eq!(
            serde_json::to_string(&from_zip).unwrap(),
            serde_json::to_string(&from_text).unwrap()
        );
        service.import_apply(accept_defaults(&from_zip)).unwrap();
        assert_eq!(accounts(&mut service).len(), 2);
    }

    #[test]
    fn archive_problems_are_reported_without_changing_anything() {
        let (mut service, _, _) = unlocked();
        let zip = protected_archive(&export(&[("one", SECRET_X)]), b"pw-1");
        assert_eq!(
            service.import_preview(FORMAT, &zip, "").unwrap_err().code,
            IpcErrorCode::PasswordRequired
        );
        let wrong = service.import_preview(FORMAT, &zip, "pw-2").unwrap_err();
        assert_eq!(wrong.code, IpcErrorCode::InvalidInput);
        assert_eq!(
            wrong.message.as_deref(),
            Some("wrong password, or the file is damaged")
        );
        assert!(accounts(&mut service).is_empty());
    }

    #[test]
    fn a_wrong_password_keeps_an_earlier_preview_untouched_and_never_echoes_the_password() {
        let (mut service, _, _) = unlocked();
        service
            .preview_text(FORMAT, &export(&[("one", SECRET_X)]))
            .unwrap();
        let zip = protected_archive(&export(&[("two", SECRET_Y)]), b"pw-1");
        let failure = service
            .import_preview(FORMAT, &zip, "hunter2-guess")
            .unwrap_err();
        assert!(!format!("{failure:?}").contains("hunter2-guess"));
        let result = service
            .import_apply(vec![choice(0, ImportDecision::Add)])
            .unwrap();
        assert_eq!(result.added, 1);
        assert_eq!(accounts(&mut service), ["one"]);
    }
}
