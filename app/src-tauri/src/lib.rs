pub mod ipc;
#[cfg(any(target_os = "android", test))]
mod keystore;
pub mod platform;

use std::time::Duration;

use mfa_core::biometric::{BiometricProvider, NoBiometrics};
use mfa_core::vault::SystemEvent;
use tauri::Manager;

use ipc::commands::{self, AppState};

const IDLE_CHECK_INTERVAL: Duration = Duration::from_secs(1);

/// Locks an idle vault even when the UI is not asking for anything (ADR-0003).
fn spawn_idle_ticker(handle: tauri::AppHandle) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(IDLE_CHECK_INTERVAL);
            handle.state::<AppState>().service().tick();
        }
    });
}

/// Minimising shows up as a resize to nothing and a lost focus; both are checked because the
/// order differs between platforms. Merely clicking another window does not lock.
fn lock_when_minimized(window: &tauri::Window, event: &tauri::WindowEvent) {
    let leaving = matches!(
        event,
        tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Focused(false)
    );
    if !leaving || !window.is_minimized().unwrap_or(false) {
        return;
    }
    if let Some(state) = window.try_state::<AppState>() {
        state.service().notify(SystemEvent::Backgrounded);
    }
}

#[cfg(target_os = "android")]
fn biometric_provider(app: &tauri::App) -> Box<dyn BiometricProvider> {
    Box::new(keystore::AndroidBiometrics::from_app(app.handle()))
}

#[cfg(not(target_os = "android"))]
fn biometric_provider(_app: &tauri::App) -> Box<dyn BiometricProvider> {
    Box::new(NoBiometrics)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "android")]
    let builder = builder.plugin(keystore::init());
    builder
        .on_window_event(lock_when_minimized)
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&directory)?;
            app.manage(AppState::new(
                directory.join("vault.mfa"),
                biometric_provider(app),
            ));
            spawn_idle_ticker(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::vault_status,
            commands::list_otp_types,
            commands::create_vault,
            commands::unlock,
            commands::biometric_status,
            commands::biometric_enable,
            commands::biometric_disable,
            commands::unlock_with_biometric,
            commands::set_biometric_reprompt_days,
            commands::lock,
            commands::record_activity,
            commands::report_backgrounded,
            commands::report_foregrounded,
            commands::expect_file_picker,
            commands::set_idle_lock,
            commands::list_entries,
            commands::add_entry,
            commands::edit_entry,
            commands::delete_entry,
            commands::move_entry,
            commands::advance_counter,
            commands::reveal_secret,
            commands::change_password,
            commands::draft_from_link,
            commands::draft_from_image,
            commands::add_draft,
            commands::check_duplicate,
            commands::check_draft_duplicate,
            commands::discard_drafts,
            commands::list_import_formats,
            commands::import_preview,
            commands::import_apply,
            commands::import_discard,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
