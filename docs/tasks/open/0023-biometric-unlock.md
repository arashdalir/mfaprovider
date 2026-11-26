# TASK-0023: Biometric unlock on Android

- Status: in-progress
- Created: 2026-11-26
- Updated: 2026-11-26
- Related ADRs: ADR-0003 (accepted), ADR-0016 (proposed), ADR-0011; part of TASK-0012

## Goal
(User request, 2026-11-26.) On the user's phone the vault can be unlocked with the fingerprint/face the phone already knows, falling back to the master password, with the password required again after a configurable period (default 7 days). Design in ADR-0003 and ADR-0016.

## Acceptance criteria
- [ ] Core: `export_key(password)` and `unlock_with_key(key)` with tests (right key, wrong key, locked, after password change, no change to the vault bytes)
- [x] Shell: `BiometricProvider` trait, `Unavailable` implementation, commands (`biometric_status`, `biometric_enable`, `biometric_disable`, `unlock_with_biometric`), device-local state with the password re-prompt period; fake provider in tests (enable needs the password; cancelled/failed prompt falls back; expired period refuses; lock keeps the setting)
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

- 2026-11-26: CI green on 645087e (all five jobs) with TASK-0016/0018/0019/0020/0021/0022 and the core layer.
- 2026-11-26: Shell layer done. `mfa-core::biometric`: `BiometricProvider` (availability, enrol, release, remove), `BiometricError` (Unavailable, Cancelled, Failed, Invalidated, Platform), `NoBiometrics`. Shell: `ipc::device_settings` (`DeviceSettings {biometric_enabled, last_password_ms, reprompt_days = 7}` in `device.json` next to the vault, atomic write, damaged file = defaults so biometrics stays off; `MemorySettings` for tests), `ipc::biometric_service` (`biometric_status`, `biometric_enable(password)`, `biometric_disable`, `unlock_with_biometric`, `set_biometric_reprompt_days` 1-365), commands registered in all four lists, `BiometricStatus` DTO, new error codes (`password_due`, `biometric_unavailable/not_enabled/cancelled/failed/invalidated`). Rules: enabling re-verifies the password and is rolled back if the setting cannot be saved; typing the password (create/unlock/enable) restarts the period, a biometric unlock does not; a dropped OS key or a wrapped key that does not open this vault switches biometrics off and deletes the wrap; nothing is prompted once the password is due. `VaultService` now needs `C: Clone` (it keeps a clock for the period); `SystemClock` is `Clone`; `serde_json` moved to normal dependencies of the shell. `AppState` uses `NoBiometrics` until the Android plugin exists, so **biometrics report "not available" on every platform for now**. 17 tests with a fake phone (cancel, fail, invalidate, other key, due period, period change, password change, disable while locked, unwritable settings, no key in the settings); shell 137, clippy clean.

## Next step
UI: Settings switch (asks the password), unlock-screen button, messages per error code (`error-message.ts`), then the Android Kotlin plugin and a test on the user's phone.
