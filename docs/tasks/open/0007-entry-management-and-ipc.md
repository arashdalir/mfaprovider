# TASK-0007: Entry management and typed IPC

- Status: open
- Created: 2026-11-17
- Updated: 2026-11-17
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

## Progress log (append-only, dated)
- 2026-11-17: Task created.

## Next step
After TASK-0006: write the ADR for the IPC type-generation approach, then implement entry operations in the core with tests first.
