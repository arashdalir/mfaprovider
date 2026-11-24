use tauri_build::{AppManifest, Attributes};

/// Every IPC command the UI may call. Must match `ipc/commands.rs`, `lib.rs` and
/// `capabilities/default.json` (a test checks this).
const COMMANDS: &[&str] = &[
    "vault_status",
    "list_otp_types",
    "create_vault",
    "unlock",
    "lock",
    "record_activity",
    "set_idle_lock",
    "list_entries",
    "add_entry",
    "edit_entry",
    "delete_entry",
    "move_entry",
    "advance_counter",
    "reveal_secret",
    "change_password",
];

fn main() {
    tauri_build::try_build(Attributes::new().app_manifest(AppManifest::new().commands(COMMANDS)))
        .expect("failed to run tauri-build");
}
