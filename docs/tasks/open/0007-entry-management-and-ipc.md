# TASK-0007: Entry management and typed IPC

- Status: in-progress
- Created: 2026-11-17
- Updated: 2026-11-24
- Related ADRs: ADR-0002, ADR-0004, ADR-0005

## Goal
Core operations on an unlocked vault (add, edit, delete with tombstone, reorder, tag/group, search) and live code computation with remaining-seconds, exposed to the UI through narrow, typed Tauri commands. The webview receives only computed codes and display metadata — never seeds.

## Acceptance criteria
- [x] Core: add/edit/delete/reorder/tag/search with tests; deletes create tombstones
- [ ] `list_entries` returns metadata + current code + seconds remaining; no secret field exists in any IPC type
- [ ] Explicit reveal/edit flow behind re-authentication, wiped after use
- [ ] IPC types generated from Rust (specta/ts-rs) and checked in CI for drift
- [ ] Tauri capabilities list only the commands in use
- [ ] Rust unit tests + a test proving serialised IPC payloads contain no secret material

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: 0003, 0006. Blocks 0008, 0009, 0010, 0011. Choice of type-generation crate (specta vs ts-rs) is an ADR-worthy decision; write it before coding.

- 2026-11-24: Research (crates.io, 2026-11-24) **reversed Claude's earlier lean towards specta**: `tauri-specta` is still 2.0.0-rc.29 on `specta` 2.0.0-rc.28 (exact-pinned, three separately versioned pre-1.0 crates, updated within the last three weeks), while `ts-rs` is stable (12.0.1, MSRV 1.78). ADR-0012 (proposed) recommends `ts-rs` DTOs + a hand-written typed client, revisit when `tauri-specta` reaches a stable 2.0. Open points to verify at implementation time: `ts-rs` transitive dependencies and its 64-bit integer mapping.
- 2026-11-24: Still open for this task (not yet decided): entry reordering representation (ADR-0011 left it open: list order vs sort key) — decide when the core operations are written; production `Clock` (system time + `Instant`) and `Rng` (`getrandom`) live in the shell.

## Progress log (append-only, dated)
- 2026-11-17: Task created.
- 2026-11-24: Wrote ADR-0012 (proposed) for user review. No code yet.

- 2026-11-24: User accepted ADR-0012 as written. Starting core entry operations.
- 2026-11-24: Decided: entry order = list order of `VaultData.entries` (as ADR-0011 already specifies; no sort key, no new ADR). `OtpType` cannot change on an existing entry (delete + add). Entries of unsupported types: metadata editable, params not. Search covers issuer, account and tags only (not notes). Pure ops live in `vault/entries.rs` (16 tests); clippy and all 131 workspace tests green locally.
- 2026-11-24: Done: core add/edit/delete(tombstone)/move/search/tags on `VaultData`. Remaining in core: session wrappers that save only on success, live codes with remaining seconds (needs a period accessor on `OtpType`), HOTP counter advance.

- 2026-11-24: Done in core: `OtpType::timing` (counter vs period) + `live_code` (code + seconds remaining; 8 tests, RFC/Steam vectors), `VaultData::advance_counter` (HOTP), `VaultData::codes` (per-entry Ready/Unsupported/Failed, one bad entry never hides others), `EntryId::random` (UUIDv4), `VaultSession::{try_edit, add_entry, edit_entry, delete_entry, move_entry, advance_counter, codes}` (refused operation = no save, not activity). 139 core tests + clippy green locally. Core acceptance items checked off; the IPC items remain.

## Next step
Shell side: verify `ts-rs` footprint (`cargo tree`, `cargo deny`) and 64-bit mapping, then the DTO layer in `app/src-tauri/src/ipc/`, generated types, typed client and the no-secrets tests.
