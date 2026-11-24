pub mod ipc;
pub mod platform;

use std::time::Duration;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&directory)?;
            app.manage(AppState::new(directory.join("vault.mfa")));
            spawn_idle_ticker(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::vault_status,
            commands::list_otp_types,
            commands::create_vault,
            commands::unlock,
            commands::lock,
            commands::record_activity,
            commands::set_idle_lock,
            commands::list_entries,
            commands::add_entry,
            commands::edit_entry,
            commands::delete_entry,
            commands::move_entry,
            commands::advance_counter,
            commands::reveal_secret,
            commands::change_password,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
