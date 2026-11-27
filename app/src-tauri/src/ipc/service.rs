//! What each IPC command does, independent of Tauri so it can be tested directly.

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use mfa_core::biometric::{BiometricProvider, NoBiometrics};
use mfa_core::otp::OtpRegistry;
use mfa_core::vault::{
    Clock, EntryChanges, KdfParams, LockPolicy, NewEntry, Rng, Storage, SystemEvent, VaultSession,
};
use zeroize::Zeroizing;

use mfa_formats::archive::{is_zip, read_single_file};
use mfa_formats::import::{MAX_IMPORT_BYTES, all_importers, importer};
use mfa_formats::merge::{Decision, MergeStatus, PlannedItem, build_ops, plan_merge};
use mfa_formats::migration::{is_migration_link, parse_migration_link};
use mfa_formats::otpauth::parse_otpauth;
use mfa_formats::qr::{MAX_FILE_BYTES, decode_qr_image};

use super::device_settings::{MemorySettings, SettingsStore};
use super::dto::{
    DraftEditsRequest, DraftView, DuplicateKind, DuplicateView, EntryChangesRequest, EntryView,
    ImportChoice, ImportDecision, ImportFormatView, ImportItemView, ImportPreview, ImportResult,
    ImportStatus, NewEntryRequest, OtpTypeView, RevealedSecret, SkippedItemView, VaultStatus,
    apply_edits, draft_views, entry_views, otp_type_views, parse_entry_id,
};
use super::error::{IpcError, IpcErrorCode};
use super::picker_excuse::{Now, PickerExcuse};

pub struct VaultService<S: Storage, C: Clock, R: Rng> {
    pub(super) session: VaultSession<S, C, R>,
    pub(super) clock: C,
    pub(super) biometrics: Box<dyn BiometricProvider>,
    pub(super) device: Box<dyn SettingsStore>,
    registry: OtpRegistry,
    /// Accounts read from links or QR codes, awaiting confirmation. Dropped when the vault locks.
    drafts: Vec<NewEntry>,
    /// A file read for import, awaiting the user's choices. Dropped when the vault locks.
    pending_import: Option<PendingImport>,
    picker_excuse: PickerExcuse,
}

struct PendingImport {
    definitions: Vec<NewEntry>,
}

impl<S: Storage, C: Clock + Clone, R: Rng> VaultService<S, C, R> {
    pub fn new(storage: S, clock: C, rng: R) -> Self {
        Self {
            session: VaultSession::new(storage, clock.clone(), rng, LockPolicy::default()),
            clock,
            biometrics: Box::new(NoBiometrics),
            device: Box::new(MemorySettings::default()),
            registry: OtpRegistry::with_builtins(),
            drafts: Vec::new(),
            pending_import: None,
            picker_excuse: PickerExcuse::default(),
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
        self.session.create(password, kdf)?;
        self.note_password_typed();
        Ok(())
    }

    pub fn unlock(&mut self, password: &str) -> Result<(), IpcError> {
        self.session.unlock(password)?;
        self.note_password_typed();
        Ok(())
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
        let now = self.now();
        if event == SystemEvent::Backgrounded && self.picker_excuse.covers_leaving(now) {
            return;
        }
        self.session.notify(event);
        self.drop_drafts_if_locked();
    }

    /// The user is about to leave for the system's file picker (ADR-0017).
    pub fn expect_file_picker(&mut self) {
        if self.session.is_unlocked() {
            let now = self.now();
            self.picker_excuse.grant(now);
        }
    }

    /// The app is on the screen again; an excuse never outlasts the trip it was given for.
    pub fn return_to_foreground(&mut self) {
        self.lock_if_picker_overstayed();
        self.picker_excuse.clear();
    }

    fn lock_if_picker_overstayed(&mut self) {
        if self.picker_excuse.is_overstayed(self.now()) {
            self.picker_excuse.clear();
            self.notify(SystemEvent::Backgrounded);
        }
    }

    fn now(&self) -> Now {
        Now {
            monotonic_ms: self.clock.monotonic_ms(),
            unix_ms: self.clock.unix_ms(),
        }
    }

    pub fn tick(&mut self) {
        self.lock_if_picker_overstayed();
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

    /// Reads an `otpauth://` link, or a Google Authenticator `otpauth-migration://` link
    /// (which may hold several accounts), into drafts, replacing any earlier drafts.
    pub fn draft_from_link(&mut self, text: &str) -> Result<Vec<DraftView>, IpcError> {
        self.require_unlocked()?;
        self.drafts = drafts_from_link(text, &self.registry)?;
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
            match drafts_from_link(&text, &self.registry) {
                Ok(found) => drafts.extend(found),
                Err(error) => {
                    first_problem.get_or_insert(error);
                }
            }
        }
        if drafts.is_empty() {
            return Err(first_problem.unwrap_or_else(|| {
                IpcError::invalid_input("the QR code is not an authenticator link")
            }));
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

    /// Whether the account about to be added is already in the vault.
    pub fn check_duplicate(
        &mut self,
        request: NewEntryRequest,
    ) -> Result<Option<DuplicateView>, IpcError> {
        let candidate = NewEntry::try_from(request)?;
        self.duplicate_of(&candidate)
    }

    /// The same for a draft with the edits applied.
    pub fn check_draft_duplicate(
        &mut self,
        draft_id: u32,
        edits: DraftEditsRequest,
    ) -> Result<Option<DuplicateView>, IpcError> {
        self.require_unlocked()?;
        let candidate = {
            let draft = usize::try_from(draft_id)
                .ok()
                .and_then(|index| self.drafts.get(index))
                .ok_or(IpcError::new(IpcErrorCode::NotFound))?;
            apply_edits(draft, edits)
        };
        self.duplicate_of(&candidate)
    }

    fn duplicate_of(&mut self, candidate: &NewEntry) -> Result<Option<DuplicateView>, IpcError> {
        let data = self.session.data()?;
        let Some(status) = plan_merge(data, std::slice::from_ref(candidate))
            .into_iter()
            .next()
        else {
            return Ok(None);
        };
        let view = |kind, id| {
            data.entry(id).map(|entry| DuplicateView {
                kind,
                matches: format!("{} · {}", entry.issuer, entry.account),
            })
        };
        Ok(match status {
            MergeStatus::DuplicateOfVault(id) => view(DuplicateKind::SameAccount, id),
            MergeStatus::ConflictWithVault(id) => view(DuplicateKind::SameNameOtherKey, id),
            MergeStatus::SameSecretUnderOtherName(id) => view(DuplicateKind::SameKeyOtherName, id),
            MergeStatus::New | MergeStatus::DuplicateOfEarlierItem(_) => None,
        })
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

    /// The seed of one entry, only after the device password is entered again.
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

fn drafts_from_link(text: &str, registry: &OtpRegistry) -> Result<Vec<NewEntry>, IpcError> {
    if !is_migration_link(text) {
        return Ok(vec![parse_otpauth(text, registry)?]);
    }
    let batch = parse_migration_link(text, registry)
        .map_err(|error| IpcError::invalid_input(error.to_string()))?;
    if batch.entries.is_empty() {
        let reason = batch
            .skipped
            .into_iter()
            .next()
            .unwrap_or_else(|| "the link holds no accounts".to_owned());
        return Err(IpcError::invalid_input(reason));
    }
    Ok(batch.entries)
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

    impl<S: Storage, C: Clock + Clone, R: Rng> VaultService<S, C, R> {
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
    fn the_trip_to_the_file_picker_does_not_lock() {
        let (mut service, _, clock) = unlocked();
        service.expect_file_picker();
        service.notify(SystemEvent::Backgrounded);
        clock.advance_secs(60);
        service.tick();
        service.return_to_foreground();
        assert!(service.status().unwrap().unlocked);
    }

    #[test]
    fn the_picker_excuse_is_used_up_by_one_trip() {
        let (mut service, _, _) = unlocked();
        service.expect_file_picker();
        service.notify(SystemEvent::Backgrounded);
        service.return_to_foreground();
        service.notify(SystemEvent::Backgrounded);
        assert!(!service.status().unwrap().unlocked);
    }

    #[test]
    fn a_second_departure_during_the_trip_locks() {
        let (mut service, _, _) = unlocked();
        service.expect_file_picker();
        service.notify(SystemEvent::Backgrounded);
        service.notify(SystemEvent::Backgrounded);
        assert!(!service.status().unwrap().unlocked);
    }

    #[test]
    fn staying_away_too_long_locks_without_the_app_coming_back() {
        let (mut service, _, clock) = unlocked();
        service.expect_file_picker();
        service.notify(SystemEvent::Backgrounded);
        clock.advance_secs(121);
        service.tick();
        assert!(!service.status().unwrap().unlocked);
    }

    #[test]
    fn coming_back_too_late_locks() {
        let (mut service, _, clock) = unlocked();
        service.expect_file_picker();
        service.notify(SystemEvent::Backgrounded);
        clock.advance_secs(121);
        service.return_to_foreground();
        assert!(!service.status().unwrap().unlocked);
    }

    #[test]
    fn an_old_excuse_covers_nothing() {
        let (mut service, _, clock) = unlocked();
        service.expect_file_picker();
        clock.advance_secs(121);
        service.notify(SystemEvent::Backgrounded);
        assert!(!service.status().unwrap().unlocked);
    }

    #[test]
    fn the_excuse_does_not_cover_suspending() {
        let (mut service, _, _) = unlocked();
        service.expect_file_picker();
        service.notify(SystemEvent::Suspended);
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
        let ids: Vec<_> = formats.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, [FORMAT, "google-migration", "freeotp"]);
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

    /// A Google Authenticator export link holding one TOTP account per `(name, secret)`.
    fn migration_link(accounts: &[(&str, &[u8])]) -> String {
        let mut message = Vec::new();
        for (name, secret) in accounts {
            let mut account = vec![0x0A, u8::try_from(secret.len()).unwrap()];
            account.extend(*secret);
            account.extend([0x12, u8::try_from(name.len()).unwrap()]);
            account.extend(name.as_bytes());
            account.extend([0x30, 0x02]);
            message.extend([0x0A, u8::try_from(account.len()).unwrap()]);
            message.extend(account);
        }
        format!(
            "otpauth-migration://offline?data={}",
            base64::engine::general_purpose::URL_SAFE.encode(message)
        )
    }

    #[test]
    fn a_google_export_link_becomes_one_draft_per_account_without_echoing_secrets() {
        let (mut service, _, _) = unlocked();
        let link = migration_link(&[("Acme:one", b"aaaaaaaaaa"), ("two", b"bbbbbbbbbb")]);
        let drafts = service.draft_from_link(&link).unwrap();
        let names: Vec<_> = drafts
            .iter()
            .map(|d| (d.issuer.as_str(), d.account.as_str()))
            .collect();
        assert_eq!(names, [("Acme", "one"), ("", "two")]);
        let json = serde_json::to_string(&drafts).unwrap();
        assert!(!json.contains("aaaaaaaaaa") && !json.contains("MFRGGZDF"));
        service.add_draft(0, edits("one")).unwrap();
        assert_eq!(accounts(&mut service), ["one"]);
    }

    #[test]
    fn a_google_export_without_usable_accounts_says_why_instead_of_failing_blindly() {
        let (mut service, _, _) = unlocked();
        let no_data = migration_link(&[]);
        assert_eq!(
            service
                .draft_from_link(&no_data)
                .unwrap_err()
                .message
                .as_deref(),
            Some("the link has no data")
        );
        let no_accounts = "otpauth-migration://offline?data=EAE%3D";
        assert_eq!(
            service
                .draft_from_link(no_accounts)
                .unwrap_err()
                .message
                .as_deref(),
            Some("the link holds no accounts")
        );
        let no_secret = migration_link(&[("one", b"")]);
        assert_eq!(
            service
                .draft_from_link(&no_secret)
                .unwrap_err()
                .message
                .as_deref(),
            Some("the account's settings are not valid")
        );
        let damaged = "otpauth-migration://offline?data=%%%";
        assert_eq!(
            service
                .draft_from_link(damaged)
                .unwrap_err()
                .message
                .as_deref(),
            Some("the link's data is damaged")
        );
    }

    #[test]
    fn a_qr_image_with_google_export_codes_gives_their_accounts() {
        let (mut service, _, _) = unlocked();
        let first = migration_link(&[("one", b"aaaaaaaaaa")]);
        let second = migration_link(&[("two", b"bbbbbbbbbb")]);
        let drafts = service
            .draft_from_image(&qr_png_base64(&[&first, &second]))
            .unwrap();
        assert_eq!(drafts.len(), 2);
    }

    #[test]
    fn google_export_files_can_be_imported_through_the_wizard_path() {
        let (mut service, _, _) = unlocked();
        let file = migration_link(&[("one", b"aaaaaaaaaa"), ("two", b"bbbbbbbbbb")]);
        let formats = service.import_formats();
        assert!(formats.iter().any(|f| f.id == "google-migration"));
        let preview = service
            .import_preview("google-migration", file.as_bytes(), "")
            .unwrap();
        assert_eq!(preview.items.len(), 2);
        service.import_apply(accept_defaults(&preview)).unwrap();
        assert_eq!(accounts(&mut service), ["one", "two"]);
    }

    mod duplicate_check {
        use super::*;
        use crate::ipc::dto::DuplicateKind;

        fn found(
            service: &mut TestService,
            request: NewEntryRequest,
        ) -> Option<(DuplicateKind, String)> {
            service
                .check_duplicate(request)
                .unwrap()
                .map(|view| (view.kind, view.matches))
        }

        #[test]
        fn nothing_is_reported_for_an_account_that_is_not_in_the_vault() {
            let (mut service, _, _) = unlocked();
            assert_eq!(found(&mut service, totp_request("alice")), None);
            service.add_entry(totp_request("alice")).unwrap();
            let mut other = totp_request("bob");
            other.secret_base32 = "JBSWY3DPEHPK3PXP".into();
            assert_eq!(found(&mut service, other), None);
        }

        #[test]
        fn the_same_name_and_key_is_the_same_account_whatever_the_case() {
            let (mut service, _, _) = unlocked();
            service.add_entry(totp_request("alice")).unwrap();
            assert_eq!(
                found(&mut service, totp_request("ALICE")),
                Some((DuplicateKind::SameAccount, "Example · alice".into()))
            );
        }

        #[test]
        fn the_same_name_with_another_key_is_reported_as_such() {
            let (mut service, _, _) = unlocked();
            service.add_entry(totp_request("alice")).unwrap();
            let mut request = totp_request("alice");
            request.secret_base32 = "JBSWY3DPEHPK3PXP".into();
            assert_eq!(
                found(&mut service, request),
                Some((DuplicateKind::SameNameOtherKey, "Example · alice".into()))
            );
        }

        #[test]
        fn the_same_key_under_another_name_is_reported_with_the_existing_name() {
            let (mut service, _, _) = unlocked();
            service.add_entry(totp_request("alice")).unwrap();
            assert_eq!(
                found(&mut service, totp_request("bob")),
                Some((DuplicateKind::SameKeyOtherName, "Example · alice".into()))
            );
        }

        #[test]
        fn checking_changes_nothing_and_needs_the_vault_unlocked() {
            let (mut service, storage, _) = unlocked();
            service.add_entry(totp_request("alice")).unwrap();
            let before = storage.bytes();
            let _ = found(&mut service, totp_request("alice"));
            assert_eq!(storage.bytes(), before);
            assert_eq!(accounts(&mut service), ["alice"]);
            service.lock();
            assert_eq!(
                service
                    .check_duplicate(totp_request("alice"))
                    .unwrap_err()
                    .code,
                IpcErrorCode::Locked
            );
        }

        #[test]
        fn the_answer_never_contains_the_key() {
            let (mut service, _, _) = unlocked();
            service.add_entry(totp_request("alice")).unwrap();
            let view = service.check_duplicate(totp_request("alice")).unwrap();
            let text = serde_json::to_string(&view).unwrap();
            assert!(!text.contains("GEZDGNBVGY3TQOJQ"), "{text}");
        }

        #[test]
        fn a_draft_is_checked_with_the_edits_applied() {
            let (mut service, _, _) = unlocked();
            service.draft_from_link(&link("alice")).unwrap();
            service.add_draft(0, edits("alice")).unwrap();
            service.draft_from_link(&link("alice")).unwrap();
            let same = service
                .check_draft_duplicate(0, edits("alice"))
                .unwrap()
                .unwrap();
            assert_eq!(
                (same.kind, same.matches.as_str()),
                (DuplicateKind::SameAccount, "Example · alice")
            );
            let renamed = service
                .check_draft_duplicate(0, edits("carol"))
                .unwrap()
                .unwrap();
            assert_eq!(renamed.kind, DuplicateKind::SameKeyOtherName);
        }

        #[test]
        fn an_unknown_draft_cannot_be_checked() {
            let (mut service, _, _) = unlocked();
            assert_eq!(
                service
                    .check_draft_duplicate(3, edits("x"))
                    .unwrap_err()
                    .code,
                IpcErrorCode::NotFound
            );
        }
    }

    mod biometrics {
        use std::sync::{Arc, Mutex};

        use mfa_core::biometric::{Availability, BiometricError, BiometricProvider, VaultKeyBytes};
        use zeroize::Zeroizing;

        use super::*;
        use crate::ipc::device_settings::{DeviceSettings, MemorySettings, SettingsStore};

        const DAY_SECONDS: u64 = 24 * 60 * 60;

        #[derive(Default)]
        struct FakeState {
            available: bool,
            wrapped: Option<[u8; 32]>,
            next_release: Option<BiometricError>,
            prompts: u32,
        }

        /// A phone: wraps the key it is given and gives it back unless told to misbehave.
        #[derive(Clone, Default)]
        struct FakePhone(Arc<Mutex<FakeState>>);

        impl FakePhone {
            fn with_biometrics() -> Self {
                let phone = Self::default();
                phone.0.lock().unwrap().available = true;
                phone
            }

            fn fail_next_prompt_with(&self, error: BiometricError) {
                self.0.lock().unwrap().next_release = Some(error);
            }

            fn prompts(&self) -> u32 {
                self.0.lock().unwrap().prompts
            }

            fn holds_a_wrap(&self) -> bool {
                self.0.lock().unwrap().wrapped.is_some()
            }

            fn hold_another_key(&self) {
                self.0.lock().unwrap().wrapped = Some([9; 32]);
            }
        }

        impl BiometricProvider for FakePhone {
            fn availability(&self) -> Availability {
                if self.0.lock().unwrap().available {
                    Availability::Available
                } else {
                    Availability::Unavailable
                }
            }

            fn enrol(&mut self, key: &[u8; 32]) -> Result<(), BiometricError> {
                self.0.lock().unwrap().wrapped = Some(*key);
                Ok(())
            }

            fn release(&mut self) -> Result<VaultKeyBytes, BiometricError> {
                let mut state = self.0.lock().unwrap();
                state.prompts += 1;
                if let Some(error) = state.next_release.take() {
                    return Err(error);
                }
                state
                    .wrapped
                    .map(Zeroizing::new)
                    .ok_or(BiometricError::Invalidated)
            }

            fn remove(&mut self) {
                self.0.lock().unwrap().wrapped = None;
            }
        }

        struct UnwritableSettings;

        impl SettingsStore for UnwritableSettings {
            fn load(&self) -> DeviceSettings {
                DeviceSettings::default()
            }

            fn save(&mut self, _: &DeviceSettings) -> std::io::Result<()> {
                Err(std::io::Error::other("read-only"))
            }
        }

        struct Device {
            service: TestService,
            phone: FakePhone,
            settings: MemorySettings,
            clock: TestClock,
        }

        fn device_with(phone: FakePhone) -> Device {
            let (storage, clock) = (MemoryStorage::new(), TestClock(Rc::new(Cell::new(0))));
            let settings = MemorySettings::default();
            let mut service = VaultService::new(storage, clock.clone(), CountingRng(1))
                .with_biometrics(Box::new(phone.clone()), Box::new(settings.clone()));
            service.create_vault(PASSWORD, KdfParams::MINIMUM).unwrap();
            Device {
                service,
                phone,
                settings,
                clock,
            }
        }

        fn device() -> Device {
            device_with(FakePhone::with_biometrics())
        }

        fn enabled_and_locked() -> Device {
            let mut device = device();
            device.service.add_entry(totp_request("alice")).unwrap();
            device.service.biometric_enable(PASSWORD).unwrap();
            device.service.lock();
            device
        }

        fn code(result: Result<(), IpcError>) -> IpcErrorCode {
            result.unwrap_err().code
        }

        #[test]
        fn a_device_without_biometrics_says_so_and_refuses_to_enable_them() {
            let mut device = device_with(FakePhone::default());
            let status = device.service.biometric_status();
            assert!(!status.available && !status.enabled);
            assert_eq!(
                code(device.service.biometric_enable(PASSWORD)),
                IpcErrorCode::BiometricUnavailable
            );
        }

        #[test]
        fn enabling_asks_for_the_password_again_and_enrols_nothing_when_it_is_wrong() {
            let mut device = device();
            assert_eq!(
                code(device.service.biometric_enable("wrong")),
                IpcErrorCode::WrongPassword
            );
            assert!(!device.phone.holds_a_wrap());
            assert!(!device.service.biometric_status().enabled);
        }

        #[test]
        fn enabling_needs_an_unlocked_vault() {
            let mut device = device();
            device.service.lock();
            assert_eq!(
                code(device.service.biometric_enable(PASSWORD)),
                IpcErrorCode::Locked
            );
        }

        #[test]
        fn once_enabled_the_biometric_unlocks_the_locked_vault_with_all_entries() {
            let mut device = enabled_and_locked();
            assert!(device.service.biometric_status().enabled);
            assert!(!device.service.status().unwrap().unlocked);
            device.service.unlock_with_biometric().unwrap();
            assert_eq!(accounts(&mut device.service), ["alice"]);
            assert_eq!(device.phone.prompts(), 1);
        }

        #[test]
        fn without_enabling_there_is_nothing_to_unlock_with() {
            let mut device = device();
            device.service.lock();
            assert_eq!(
                code(device.service.unlock_with_biometric()),
                IpcErrorCode::BiometricNotEnabled
            );
            assert_eq!(device.phone.prompts(), 0);
        }

        #[test]
        fn a_cancelled_or_failed_prompt_leaves_the_vault_locked_and_biometrics_on() {
            let mut device = enabled_and_locked();
            device
                .phone
                .fail_next_prompt_with(BiometricError::Cancelled);
            assert_eq!(
                code(device.service.unlock_with_biometric()),
                IpcErrorCode::BiometricCancelled
            );
            device.phone.fail_next_prompt_with(BiometricError::Failed);
            assert_eq!(
                code(device.service.unlock_with_biometric()),
                IpcErrorCode::BiometricFailed
            );
            assert!(!device.service.status().unwrap().unlocked);
            assert!(device.service.biometric_status().enabled);
            device.service.unlock_with_biometric().unwrap();
        }

        #[test]
        fn the_password_always_still_works() {
            let mut device = enabled_and_locked();
            device.phone.fail_next_prompt_with(BiometricError::Failed);
            let _ = device.service.unlock_with_biometric();
            device.service.unlock(PASSWORD).unwrap();
        }

        #[test]
        fn a_dropped_biometric_key_switches_biometrics_off_and_deletes_the_wrap() {
            let mut device = enabled_and_locked();
            device
                .phone
                .fail_next_prompt_with(BiometricError::Invalidated);
            assert_eq!(
                code(device.service.unlock_with_biometric()),
                IpcErrorCode::BiometricInvalidated
            );
            assert!(!device.service.biometric_status().enabled);
            assert!(!device.phone.holds_a_wrap());
            assert_eq!(
                code(device.service.unlock_with_biometric()),
                IpcErrorCode::BiometricNotEnabled
            );
        }

        #[test]
        fn a_wrapped_key_that_does_not_open_this_vault_counts_as_invalidated() {
            let mut device = enabled_and_locked();
            device.phone.hold_another_key();
            assert_eq!(
                code(device.service.unlock_with_biometric()),
                IpcErrorCode::BiometricInvalidated
            );
            assert!(!device.service.biometric_status().enabled);
            assert!(!device.service.status().unwrap().unlocked);
        }

        #[test]
        fn after_the_period_the_password_is_due_and_no_prompt_is_shown() {
            let mut device = enabled_and_locked();
            device.clock.advance_secs(7 * DAY_SECONDS + 1);
            assert!(device.service.biometric_status().password_due);
            assert_eq!(
                code(device.service.unlock_with_biometric()),
                IpcErrorCode::PasswordDue
            );
            assert_eq!(device.phone.prompts(), 0);
        }

        #[test]
        fn typing_the_password_starts_the_period_again() {
            let mut device = enabled_and_locked();
            device.clock.advance_secs(7 * DAY_SECONDS + 1);
            device.service.unlock(PASSWORD).unwrap();
            device.service.lock();
            assert!(!device.service.biometric_status().password_due);
            device.service.unlock_with_biometric().unwrap();
        }

        #[test]
        fn a_biometric_unlock_does_not_start_the_period_again() {
            let mut device = enabled_and_locked();
            device.clock.advance_secs(6 * DAY_SECONDS);
            device.service.unlock_with_biometric().unwrap();
            device.service.lock();
            device.clock.advance_secs(2 * DAY_SECONDS);
            assert!(device.service.biometric_status().password_due);
        }

        #[test]
        fn the_period_can_be_changed_within_limits() {
            let mut device = enabled_and_locked();
            device.service.set_biometric_reprompt_days(30).unwrap();
            assert_eq!(device.service.biometric_status().reprompt_days, 30);
            device.clock.advance_secs(20 * DAY_SECONDS);
            device.service.unlock_with_biometric().unwrap();
            for bad in [0, 366] {
                assert_eq!(
                    code(device.service.set_biometric_reprompt_days(bad)),
                    IpcErrorCode::InvalidInput
                );
            }
        }

        #[test]
        fn locking_keeps_the_setting_and_a_password_change_keeps_the_biometric_working() {
            let mut device = device();
            device.service.biometric_enable(PASSWORD).unwrap();
            device
                .service
                .change_password(PASSWORD, "another password", KdfParams::MINIMUM)
                .unwrap();
            device.service.lock();
            assert!(device.service.biometric_status().enabled);
            device.service.unlock_with_biometric().unwrap();
        }

        #[test]
        fn disabling_deletes_the_wrap_and_works_while_locked() {
            let mut device = enabled_and_locked();
            device.service.biometric_disable().unwrap();
            assert!(!device.phone.holds_a_wrap());
            assert!(!device.service.biometric_status().enabled);
            assert_eq!(
                code(device.service.unlock_with_biometric()),
                IpcErrorCode::BiometricNotEnabled
            );
        }

        #[test]
        fn enabling_is_undone_when_the_setting_cannot_be_saved() {
            let (storage, clock) = (MemoryStorage::new(), TestClock(Rc::new(Cell::new(0))));
            let phone = FakePhone::with_biometrics();
            let mut service = VaultService::new(storage, clock, CountingRng(1))
                .with_biometrics(Box::new(phone.clone()), Box::new(UnwritableSettings));
            service.create_vault(PASSWORD, KdfParams::MINIMUM).unwrap();
            assert_eq!(
                code(service.biometric_enable(PASSWORD)),
                IpcErrorCode::StorageFailed
            );
            assert!(!phone.holds_a_wrap());
        }

        #[test]
        fn the_settings_hold_no_key_or_secret() {
            let device = enabled_and_locked();
            let saved = serde_json::to_string(&device.settings.load()).unwrap();
            assert!(!saved.to_lowercase().contains("key"), "{saved}");
            assert!(!saved.contains("GEZDGNBV"), "{saved}");
        }
    }
}
