//! The same command list appears in four places; this keeps them identical so a command can
//! neither be callable without a declared permission nor be permitted without existing.

use std::collections::BTreeSet;

fn set(names: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    names.into_iter().collect()
}

fn defined_commands() -> BTreeSet<String> {
    let source = include_str!("commands.rs");
    let mut names = Vec::new();
    let mut after_attribute = false;
    for line in source.lines() {
        if after_attribute {
            let name = line
                .trim()
                .strip_prefix("pub async fn ")
                .and_then(|rest| rest.split('(').next());
            names.extend(name.map(str::to_owned));
        }
        after_attribute = line.trim() == "#[tauri::command]";
    }
    set(names)
}

fn registered_handlers() -> BTreeSet<String> {
    set(include_str!("../lib.rs")
        .lines()
        .filter_map(|line| line.trim().strip_prefix("commands::"))
        .filter_map(|rest| rest.strip_suffix(','))
        .filter(|name| name.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
        .map(str::to_owned))
}

fn manifest_commands() -> BTreeSet<String> {
    let source = include_str!("../../build.rs");
    let list = source
        .split("const COMMANDS")
        .nth(1)
        .and_then(|rest| rest.split("];").next())
        .expect("build.rs declares COMMANDS");
    set(list.split('"').skip(1).step_by(2).map(str::to_owned))
}

fn permitted_commands() -> BTreeSet<String> {
    let json: serde_json::Value =
        serde_json::from_str(include_str!("../../capabilities/default.json")).unwrap();
    set(json["permissions"]
        .as_array()
        .expect("permissions is a list")
        .iter()
        .map(|permission| {
            let text = permission.as_str().expect("permissions are strings");
            text.strip_prefix("allow-")
                .unwrap_or_else(|| panic!("`{text}` is not an app command permission"))
                .replace('-', "_")
        }))
}

#[test]
fn the_four_command_lists_are_identical() {
    let defined = defined_commands();
    assert!(
        defined.len() >= 10,
        "the scan found too few commands: {defined:?}"
    );
    assert_eq!(registered_handlers(), defined, "lib.rs handler list");
    assert_eq!(manifest_commands(), defined, "build.rs COMMANDS");
    assert_eq!(permitted_commands(), defined, "capabilities/default.json");
}

#[test]
fn the_default_capability_grants_no_core_or_plugin_permissions() {
    let json: serde_json::Value =
        serde_json::from_str(include_str!("../../capabilities/default.json")).unwrap();
    let all_app_commands = json["permissions"].as_array().unwrap().iter().all(|p| {
        p.as_str()
            .is_some_and(|s| s.starts_with("allow-") && !s.contains(':'))
    });
    assert!(all_app_commands);
}
