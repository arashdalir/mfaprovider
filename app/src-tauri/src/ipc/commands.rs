//! Tauri commands: thin wrappers that lock the shared service and delegate to it.
//! Argon2 runs inside some of them, so they are `async` to stay off the UI thread.

use std::sync::{Mutex, MutexGuard, PoisonError};

use mfa_storage::FileStorage;
use tauri::State;

use super::dto::{
    EntryChangesRequest, EntryView, NewEntryRequest, OtpTypeView, RevealedSecret, VaultStatus,
};
use super::error::{IpcError, IpcErrorCode};
use super::service::VaultService;
use crate::platform::{OsRng, SystemClock, tuned_kdf_params};

pub type Service = VaultService<FileStorage, SystemClock, OsRng>;

pub struct AppState(Mutex<Service>);

impl AppState {
    pub fn new(vault_path: std::path::PathBuf) -> Self {
        let service =
            VaultService::new(FileStorage::new(vault_path), SystemClock::default(), OsRng);
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
pub async fn lock(state: State<'_, AppState>) -> Result<(), IpcError> {
    state.service().lock();
    Ok(())
}

#[tauri::command]
pub async fn record_activity(state: State<'_, AppState>) -> Result<(), IpcError> {
    state.service().record_activity();
    Ok(())
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
