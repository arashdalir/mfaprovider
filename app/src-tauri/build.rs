use tauri_build::{AppManifest, Attributes};

/// Every IPC command the UI may call. Must match `ipc/commands.rs`, `lib.rs` and
/// `capabilities/default.json` (a test checks this).
const COMMANDS: &[&str] = &[
    "vault_status",
    "list_otp_types",
    "create_vault",
    "unlock",
    "biometric_status",
    "biometric_enable",
    "biometric_disable",
    "unlock_with_biometric",
    "set_biometric_reprompt_days",
    "lock",
    "record_activity",
    "report_backgrounded",
    "report_foregrounded",
    "expect_file_picker",
    "set_idle_lock",
    "list_entries",
    "add_entry",
    "edit_entry",
    "delete_entry",
    "move_entry",
    "advance_counter",
    "reveal_secret",
    "change_password",
    "draft_from_link",
    "draft_from_image",
    "add_draft",
    "check_duplicate",
    "check_draft_duplicate",
    "discard_drafts",
    "list_import_formats",
    "import_preview",
    "import_apply",
    "import_discard",
];

fn main() {
    tauri_build::try_build(Attributes::new().app_manifest(AppManifest::new().commands(COMMANDS)))
        .expect("failed to run tauri-build");
}
