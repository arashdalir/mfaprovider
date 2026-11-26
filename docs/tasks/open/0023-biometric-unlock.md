# TASK-0023: Biometric unlock on Android

- Status: in-progress
- Created: 2026-11-26
- Updated: 2026-11-26
- Related ADRs: ADR-0003 (accepted), ADR-0016 (proposed), ADR-0011; part of TASK-0012

## Goal
(User request, 2026-11-26.) On the user's phone the vault can be unlocked with the fingerprint/face the phone already knows, falling back to the master password, with the password required again after a configurable period (default 7 days). Design in ADR-0003 and ADR-0016.

## Acceptance criteria
- [ ] Core: `export_key(password)` and `unlock_with_key(key)` with tests (right key, wrong key, locked, after password change, no change to the vault bytes)
- [ ] Shell: `BiometricProvider` trait, `Unavailable` implementation, commands (`biometric_status`, `biometric_enable`, `biometric_disable`, `unlock_with_biometric`), device-local state with the password re-prompt period; fake provider in tests (enable needs the password; cancelled/failed prompt falls back; expired period refuses; lock keeps the setting)
- [ ] UI: Settings switch "Unlock with fingerprint/face" (asks the password), unlock screen button, clear messages for cancelled/failed/invalidated/expired
- [ ] Android plugin (Kotlin): Keystore AES-GCM key bound to strong biometrics, invalidated on enrolment change, wrapped VK in app-private storage; `allowBackup=false`, `FLAG_SECURE`
- [ ] Tested on the user's phone; results recorded here
- [ ] CI green

## Decisions & notes (append-only, dated)
- 2026-11-26: Official `tauri-plugin-biometric` is a prompt only (no key binding), which ADR-0003 rules out; so a small own plugin (ADR-0016, proposed).
- 2026-11-26: The VK is not changed by a password change, so biometric unlock survives a password change.

## Progress log (append-only, dated)
- 2026-11-26: Task created; ADR-0016 written as proposed.

- 2026-11-26: Core layer done. `container::open_with_key(key, file)` (wrong key = `UnwrapFailed`, same as a wrong password), `OpenedVault::key_copy()` (zeroising), `VAULT_KEY_LENGTH`, `VaultSession::unlock_with_key(key)` and `VaultSession::export_key(password)` (re-verifies the password first; refuses while locked). 11 tests: right key opens like the password, wrong key refused, key survives a password change, nothing written to storage, locked/no-vault cases. mfa-core 167 tests, clippy clean. Nothing of this is reachable from the UI yet.

## Next step
Shell layer: `BiometricProvider` trait (+ `Unavailable`), device-local state (enabled, last password unlock), the four commands, fake-provider tests.
