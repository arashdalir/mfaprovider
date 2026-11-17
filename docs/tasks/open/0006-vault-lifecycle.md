# TASK-0006: Vault lifecycle — create, unlock, lock, change password

- Status: open
- Created: 2026-11-17
- Updated: 2026-11-17
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

## Progress log (append-only, dated)
- 2026-11-17: Task created.

## Next step
After TASK-0005: define the storage, clock and RNG traits and write the create/unlock tests with in-memory fakes.
