# TASK-0006: Vault lifecycle — create, unlock, lock, change password

- Status: in-progress
- Created: 2026-11-17
- Updated: 2026-11-24
- Related ADRs: ADR-0003, ADR-0005

## Goal
A `Vault` session type in `mfa-core` with create / unlock / lock / change-password, backed by injected storage, clock and RNG traits. The vault key exists in memory only while unlocked and is zeroized on lock. Auto-lock policy (timeout, background, suspend) is core logic driven by injected time events.

## Acceptance criteria
- [ ] Create vault with master password; unlock; lock zeroizes key (verified by test)
- [ ] Change master password re-wraps the vault key without re-encrypting entries
- [ ] Auto-lock after configurable idle timeout via injected clock
- [ ] Atomic save (temp file + rename) through the storage trait; in-memory fake for tests
- [ ] Wrong password: constant-behaviour failure, no secret-dependent logging
- [ ] CI green

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

## Next step
Implement `vault/{clock,storage,session}.rs` test-first with `MemoryStorage` and a fake clock; then create `core/mfa-storage` with `FileStorage` and filesystem tests; commit after each green step.
