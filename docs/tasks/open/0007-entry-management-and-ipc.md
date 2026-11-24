# TASK-0007: Entry management and typed IPC

- Status: in-progress
- Created: 2026-11-17
- Updated: 2026-11-24
- Related ADRs: ADR-0002, ADR-0004, ADR-0005

## Goal
Core operations on an unlocked vault (add, edit, delete with tombstone, reorder, tag/group, search) and live code computation with remaining-seconds, exposed to the UI through narrow, typed Tauri commands. The webview receives only computed codes and display metadata — never seeds.

## Acceptance criteria
- [ ] Core: add/edit/delete/reorder/tag/search with tests; deletes create tombstones
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

## Next step
User reviews ADR-0012. After acceptance: implement the core entry operations (add/edit/delete with tombstone/reorder/tag/search) test-first in `mfa-core`, deciding the reorder representation first; then the shell DTO layer, generated types, typed client and the no-secrets tests.
