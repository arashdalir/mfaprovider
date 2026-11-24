# TASK-0007: Entry management and typed IPC

- Status: done
- Created: 2026-11-17
- Updated: 2026-11-24
- Related ADRs: ADR-0002, ADR-0004, ADR-0005

## Goal
Core operations on an unlocked vault (add, edit, delete with tombstone, reorder, tag/group, search) and live code computation with remaining-seconds, exposed to the UI through narrow, typed Tauri commands. The webview receives only computed codes and display metadata — never seeds.

## Acceptance criteria
- [x] Core: add/edit/delete/reorder/tag/search with tests; deletes create tombstones
- [x] `list_entries` returns metadata + current code + seconds remaining; no secret field exists in any IPC type
- [x] Explicit reveal/edit flow behind re-authentication (backend; seed returned as a string, so JS-side wiping is best-effort — see notes)
- [x] IPC types generated from Rust (ts-rs, ADR-0012) and checked in CI for drift
- [x] Tauri capabilities list only the commands in use
- [x] Rust unit tests + a test proving serialised IPC payloads contain no secret material

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

- 2026-11-24: Shell DTO layer done (`app/src-tauri/src/ipc/{dto,error,no_secrets}.rs`, 23 shell tests; core now 145 tests + base32 `Secret` codec and `EntryId::parse`). `ts-rs` 12.0.1 footprint verified: only `ts-rs-macros` + `termcolor` (+ windows helper) added to the shell tree, `cargo deny` clean; 64-bit timestamps overridden to `number` and tested against real `serde_json` output; generated TS committed in `app/src/ipc/generated/`; CI now runs shell `cargo test` and fails on drift. No-secrets proof: seed absent from serialised views in base32/hex/decimal/raw form, and a source tripwire; both verified to fail on a deliberately injected `seed_bytes: Vec<u8>` field.
- 2026-11-24: **Clarification of ADR-0012 point 2 (not a change):** "no secret field on any DTO" applies to data sent *to* the UI. Seeds also travel *from* the UI in `NewEntryRequest` and `EntryChangesRequest` (`secret_base32`, the user typing/scanning it in); the tripwire allows exactly those fields plus `RevealedSecret`.
- 2026-11-24: Still to do: Tauri commands + app state (production Clock/Rng, re-auth for reveal), typed `client.ts` + lint rule + command-name test, capabilities.

- 2026-11-24: Done: 13 Tauri commands (`ipc/commands.rs`, thin async wrappers over `ipc/service.rs`, 15 service tests), `platform.rs` (SystemClock, OsRng via `getrandom` 0.4, KDF tuning: target 500 ms, memory cap 256 MiB — device-tuned at vault creation and password change, stored in the file), core `verify_password` + `run_kdf_benchmark`, idle ticker thread, app manifest + capability listing exactly the commands (a test keeps commands.rs, lib.rs handler list, build.rs and capabilities identical), `client.ts` (one module calls `invoke`; oxlint `no-restricted-imports` verified to reject a rogue import; vitest checks client command names against commands.rs, verified to fail on a typo). Counts: core 147, shell 45, UI 8.
- 2026-11-24: Honest limits: (1) the commands and `setup` hook (app data dir, ticker thread) are compiled and unit-tested via the service but **not yet exercised in a running app** — TASK-0008 is the first real caller; (2) `reveal_secret` returns the seed as a JS string, which cannot be wiped — the UI must show it briefly and drop references; (3) no unlock rate limiting on desktop (Argon2 cost only); (4) backgrounded/suspend events are not yet wired to `notify` (TASK-0012 for Android; desktop keeps idle lock only); (5) `change_password` and `reveal_secret` are not rate-limited either.

- 2026-11-24: CI green on the pushed commit (all jobs). Task closed.

## Next step
None. Follow-ups: TASK-0008 (first real caller of the commands; smoke-run there), TASK-0012 (background/suspend events, Android), rate limiting (open question).
