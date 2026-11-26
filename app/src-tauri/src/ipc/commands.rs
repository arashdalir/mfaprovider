//! Tauri commands: thin wrappers that lock the shared service and delegate to it.
//! Argon2 runs inside some of them, so they are `async` to stay off the UI thread.

use std::sync::{Mutex, MutexGuard, PoisonError};

use mfa_core::biometric::NoBiometrics;
use mfa_storage::FileStorage;
use tauri::State;
use zeroize::Zeroizing;

use super::device_settings::FileSettings;
use super::dto::{
    BiometricStatus, DraftEditsRequest, DraftView, DuplicateView, EntryChangesRequest, EntryView,
    ImportChoice, ImportFormatView, ImportPreview, ImportResult, NewEntryRequest, OtpTypeView,
    RevealedSecret, VaultStatus,
};
use super::error::{IpcError, IpcErrorCode};
use super::service::{VaultService, decode_base64_file};
use crate::platform::{OsRng, SystemClock, tuned_kdf_params};

pub type Service = VaultService<FileStorage, SystemClock, OsRng>;

pub struct AppState(Mutex<Service>);

impl AppState {
    pub fn new(vault_path: std::path::PathBuf) -> Self {
        let settings = FileSettings::new(vault_path.with_file_name("device.json"));
        let service =
            VaultService::new(FileStorage::new(vault_path), SystemClock::default(), OsRng)
                .with_biometrics(Box::new(NoBiometrics), Box::new(settings));
        Self(Mutex::new(service))
    }

    /// If another thread panicked while holding the lock, the vault is locked before use.
    pub fn service(&self) -> MutexGuard<'_, Service> {
        self.0.lock().unwrap_or_else(|poisoned: PoisonError<_>| {
            let mut service = poisoned.into_inner();
            service.lock();
            service
        })
    }
}

#[tauri::command]
pub async fn vault_status(state: State<'_, AppState>) -> Result<VaultStatus, IpcError> {
    state.service().status()
}

#[tauri::command]
pub async fn list_otp_types(state: State<'_, AppState>) -> Result<Vec<OtpTypeView>, IpcError> {
    Ok(state.service().otp_types())
}

#[tauri::command]
pub async fn create_vault(state: State<'_, AppState>, password: String) -> Result<(), IpcError> {
    let kdf = tuned_kdf_params();
    state.service().create_vault(&password, kdf)
}

#[tauri::command]
pub async fn unlock(state: State<'_, AppState>, password: String) -> Result<(), IpcError> {
    state.service().unlock(&password)
}

#[tauri::command]
pub async fn biometric_status(state: State<'_, AppState>) -> Result<BiometricStatus, IpcError> {
    Ok(state.service().biometric_status())
}

#[tauri::command]
pub async fn biometric_enable(
    state: State<'_, AppState>,
    password: String,
) -> Result<(), IpcError> {
    let password = Zeroizing::new(password);
    state.service().biometric_enable(&password)
}

#[tauri::command]
pub async fn biometric_disable(state: State<'_, AppState>) -> Result<(), IpcError> {
    state.service().biometric_disable()
}

#[tauri::command]
pub async fn unlock_with_biometric(state: State<'_, AppState>) -> Result<(), IpcError> {
    state.service().unlock_with_biometric()
}

#[tauri::command]
pub async fn set_biometric_reprompt_days(
    state: State<'_, AppState>,
    days: u32,
) -> Result<(), IpcError> {
    state.service().set_biometric_reprompt_days(days)
}

#[tauri::command]
pub async fn lock(state: State<'_, AppState>) -> Result<(), IpcError> {
    state.service().lock();
    Ok(())
}

#[tauri::command]
pub async fn record_activity(state: State<'_, AppState>) -> Result<(), IpcError> {
    state.service().record_activity();
    Ok(())
}

/// `seconds` of `null` turns the idle lock off.
#[tauri::command]
pub async fn set_idle_lock(
    state: State<'_, AppState>,
    seconds: Option<u32>,
) -> Result<(), IpcError> {
    state.service().set_idle_lock(seconds)
}

#[tauri::command]
pub async fn list_entries(state: State<'_, AppState>) -> Result<Vec<EntryView>, IpcError> {
    state.service().list_entries()
}

#[tauri::command]
pub async fn add_entry(
    state: State<'_, AppState>,
    request: NewEntryRequest,
) -> Result<String, IpcError> {
    state.service().add_entry(request)
}

#[tauri::command]
pub async fn edit_entry(
    state: State<'_, AppState>,
    id: String,
    request: EntryChangesRequest,
) -> Result<(), IpcError> {
    state.service().edit_entry(&id, request)
}

#[tauri::command]
pub async fn delete_entry(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    state.service().delete_entry(&id)
}

#[tauri::command]
pub async fn move_entry(
    state: State<'_, AppState>,
    id: String,
    position: u32,
) -> Result<(), IpcError> {
    let position =
        usize::try_from(position).map_err(|_| IpcError::new(IpcErrorCode::InvalidInput))?;
    state.service().move_entry(&id, position)
}

#[tauri::command]
pub async fn advance_counter(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    state.service().advance_counter(&id)
}

#[tauri::command]
pub async fn reveal_secret(
    state: State<'_, AppState>,
    id: String,
    password: String,
) -> Result<RevealedSecret, IpcError> {
    state.service().reveal_secret(&id, &password)
}

#[tauri::command]
pub async fn change_password(
    state: State<'_, AppState>,
    current: String,
    replacement: String,
) -> Result<(), IpcError> {
    let kdf = tuned_kdf_params();
    state.service().change_password(&current, &replacement, kdf)
}

#[tauri::command]
pub async fn draft_from_link(
    state: State<'_, AppState>,
    text: String,
) -> Result<Vec<DraftView>, IpcError> {
    state.service().draft_from_link(&text)
}

/// `image` is the file's bytes as base64 (raw IPC bodies are not available on Android).
#[tauri::command]
pub async fn draft_from_image(
    state: State<'_, AppState>,
    image: String,
) -> Result<Vec<DraftView>, IpcError> {
    state.service().draft_from_image(&image)
}

#[tauri::command]
pub async fn add_draft(
    state: State<'_, AppState>,
    id: u32,
    edits: DraftEditsRequest,
) -> Result<String, IpcError> {
    state.service().add_draft(id, edits)
}

#[tauri::command]
pub async fn check_duplicate(
    state: State<'_, AppState>,
    request: NewEntryRequest,
) -> Result<Option<DuplicateView>, IpcError> {
    state.service().check_duplicate(request)
}

#[tauri::command]
pub async fn check_draft_duplicate(
    state: State<'_, AppState>,
    id: u32,
    edits: DraftEditsRequest,
) -> Result<Option<DuplicateView>, IpcError> {
    state.service().check_draft_duplicate(id, edits)
}

#[tauri::command]
pub async fn discard_drafts(state: State<'_, AppState>) -> Result<(), IpcError> {
    state.service().discard_drafts();
    Ok(())
}

#[tauri::command]
pub async fn list_import_formats(
    state: State<'_, AppState>,
) -> Result<Vec<ImportFormatView>, IpcError> {
    Ok(state.service().import_formats())
}

/// `file` is the file's bytes as base64: a text export or a ZIP archive, which needs `password`.
/// Nothing in the vault changes until `import_apply`.
#[tauri::command]
pub async fn import_preview(
    state: State<'_, AppState>,
    format: String,
    file: String,
    password: Option<String>,
) -> Result<ImportPreview, IpcError> {
    let password = Zeroizing::new(password.unwrap_or_default());
    let bytes = Zeroizing::new(decode_base64_file(&file)?);
    state.service().import_preview(&format, &bytes, &password)
}

#[tauri::command]
pub async fn import_apply(
    state: State<'_, AppState>,
    choices: Vec<ImportChoice>,
) -> Result<ImportResult, IpcError> {
    state.service().import_apply(choices)
}

#[tauri::command]
pub async fn import_discard(state: State<'_, AppState>) -> Result<(), IpcError> {
    state.service().import_discard();
    Ok(())
}
