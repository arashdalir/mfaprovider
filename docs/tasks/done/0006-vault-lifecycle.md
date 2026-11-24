# TASK-0006: Vault lifecycle — create, unlock, lock, change password

- Status: done
- Created: 2026-11-17
- Updated: 2026-11-24
- Related ADRs: ADR-0003, ADR-0005

## Goal
A `Vault` session type in `mfa-core` with create / unlock / lock / change-password, backed by injected storage, clock and RNG traits. The vault key exists in memory only while unlocked and is zeroized on lock. Auto-lock policy (timeout, background, suspend) is core logic driven by injected time events.

## Acceptance criteria
- [x] Create vault with master password; unlock; lock drops the key and entries (tests verify the locked state; the wiping itself is `zeroize`'s job on drop and is not observable — see notes)
- [x] Change master password re-wraps the vault key without re-encrypting entries (test asserts the body bytes are identical before and after)
- [x] Auto-lock after configurable idle timeout via injected clock (plus background/suspend events)
- [x] Atomic save (temp file + rename) through the storage trait; in-memory fake for tests (`FileStorage` in `mfa-storage` implements it on a real filesystem)
- [x] Wrong password: constant-behaviour failure, no secret-dependent logging (single `UnwrapFailed` error; the core has no logging at all; failed attempts leave the stored file untouched)
- [x] CI green — run 45386459415 green on all jobs, including Rust core on Ubuntu, which runs the Unix-only paths (directory fsync, 0600 mode, `/` path test)

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: 0005. Blocks 0007. Biometric unlock is a separate `BiometricProvider` (TASK-0012); this task only leaves the extension seam in the key-wrapping design (ADR-0003).

- 2026-11-24: Design. `VaultSession<S: Storage, C: Clock, R: Rng>` in `vault/session.rs`; locked = no unlocked state. `Storage` trait (`read -> Option<bytes>`, `write` must be atomic) + `MemoryStorage` (shared handle, failure injection) in core; the real `FileStorage` (temp file + fsync + rename, 0600 on Unix) goes in a new crate `core/mfa-storage` so atomic replace is tested on a real filesystem on Windows and Linux in CI. This adds a crate not listed in ADR-0005's layout (does not contradict it). `Clock` has `unix_ms()` for timestamps and `monotonic_ms()` for idle timing (wall-clock jumps must not trigger or defer a lock).
- 2026-11-24: Auto-lock rules. Idle time counts only user activity (`record_activity`, `edit`, `unlock`); **reading codes via `data()` does not reset it**, because the UI polls codes every second and would otherwise never lock. `tick()` is called by a shell timer; every operation also enforces the timeout itself. Policy: `idle_timeout` (default 5 min, `None` = never), `on_background`, `on_suspend` (both default true) via `notify(SystemEvent)`.
- 2026-11-24: Failed saves. `edit` mutates memory, then persists; if persisting fails the session stays unlocked with `is_dirty() == true` and `save()` retries. Memory may be ahead of disk until then; auto-lock still locks and drops unsaved edits (security over convenience). `VaultData` is deliberately not `Clone`, so rollback is not offered.
- 2026-11-24: `change_password(current, new, kdf)` re-verifies the current password (re-derives it) so an unattended unlocked session cannot silently change it, then uses `OpenedVault::rewrap` — the encrypted body bytes are copied unchanged.
- 2026-11-24: Limit on the "lock zeroizes key (verified by test)" criterion: tests verify that lock drops the unlocked state (key and data) and that every operation then fails with `Locked`; the wiping itself is done by `zeroize` on drop and cannot be observed from safe Rust, so it is not independently verified.
- 2026-11-24: Not in this task: throttling/lockout of repeated wrong passwords (belongs with the shell/UI, TASK-0007/0008); `reauthenticate` for reveal/export (added when needed).

## Progress log (append-only, dated)
- 2026-11-17: Task created.
- 2026-11-24: Design recorded; starting with the core session, then `mfa-storage`.
- 2026-11-24: Implemented `vault/{clock,storage,session}.rs` (`Clock`, `Storage`, `MemoryStorage`, `VaultSession`, `LockPolicy`, `SystemEvent`, `SessionError`) with 26 new core tests (commit ef32c78). Added crate `core/mfa-storage` (`FileStorage`: temp file → fsync → rename, directory fsync and mode 0600 on Unix) with 8 unit tests on real temp directories plus 4 cross-crate integration tests (`tests/session_roundtrip.rs`) that drive a real session over a real file using only the public API, including a check that the file on disk contains neither seed, account name nor password. 115 tests pass locally.
- 2026-11-24: Fixed along the way: clippy flagged platform-specific code in `FileStorage` (unused import / `unnecessary_wraps` on Windows) — restructured with `cfg` blocks so Windows and Linux both lint clean; `cargo deny` flagged the path dependency on `mfa-core` as a wildcard — fixed by giving it an explicit `version`, no policy change.
- 2026-11-24: Known gaps carried forward: production `Clock` (system time + `Instant`) and `Rng` (OS randomness via `getrandom`) belong in the Tauri shell (TASK-0007); a crash-injection test of the real atomic rename is not possible portably — atomicity rests on `fs::rename` replace semantics plus the tests above; wrong-password throttling is TASK-0007/0008.

- 2026-11-24: CI run 45386459415 for d3350e5 green on all jobs (core and UI on Ubuntu and Windows, dependency audit). Task complete.

## Next step
None — done. Next: TASK-0007 (entry management and typed IPC); it needs the ADR on IPC type generation (specta vs ts-rs) first.
