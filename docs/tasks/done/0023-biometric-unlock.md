# TASK-0023: Biometric unlock on Android

- Status: done
- Created: 2026-11-26
- Updated: 2026-11-26
- Related ADRs: ADR-0003 (accepted), ADR-0016 (proposed), ADR-0011; part of TASK-0012

## Goal
(User request, 2026-11-26.) On the user's phone the vault can be unlocked with the fingerprint/face the phone already knows, falling back to the master password, with the password required again after a configurable period (default 7 days). Design in ADR-0003 and ADR-0016.

## Acceptance criteria
- [x] Core: `export_key(password)` and `unlock_with_key(key)` with tests (right key, wrong key, locked, after password change, no change to the vault bytes)
- [x] Shell: `BiometricProvider` trait, `Unavailable` implementation, commands (`biometric_status`, `biometric_enable`, `biometric_disable`, `unlock_with_biometric`), device-local state with the password re-prompt period; fake provider in tests (enable needs the password; cancelled/failed prompt falls back; expired period refuses; lock keeps the setting)
- [x] UI: Settings switch "Unlock with fingerprint/face" (asks the password), unlock screen button, clear messages for cancelled/failed/invalidated/expired
- [x] Android plugin (Kotlin): Keystore AES-GCM key bound to strong biometrics, invalidated on enrolment change, wrapped VK in app-private storage; `allowBackup=false`, `FLAG_SECURE`
- [x] Tested on the user's phone; results recorded here (partial, see log)
- [x] CI green (076b3d3)

## Decisions & notes (append-only, dated)
- 2026-11-26: Official `tauri-plugin-biometric` is a prompt only (no key binding), which ADR-0003 rules out; so a small own plugin (ADR-0016, proposed).
- 2026-11-26: The VK is not changed by a password change, so biometric unlock survives a password change.

## Progress log (append-only, dated)
- 2026-11-26: Task created; ADR-0016 written as proposed.

- 2026-11-26: Core layer done. `container::open_with_key(key, file)` (wrong key = `UnwrapFailed`, same as a wrong password), `OpenedVault::key_copy()` (zeroising), `VAULT_KEY_LENGTH`, `VaultSession::unlock_with_key(key)` and `VaultSession::export_key(password)` (re-verifies the password first; refuses while locked). 11 tests: right key opens like the password, wrong key refused, key survives a password change, nothing written to storage, locked/no-vault cases. mfa-core 167 tests, clippy clean. Nothing of this is reachable from the UI yet.

- 2026-11-26: CI green on 645087e (all five jobs) with TASK-0016/0018/0019/0020/0021/0022 and the core layer.
- 2026-11-26: Shell layer done. `mfa-core::biometric`: `BiometricProvider` (availability, enrol, release, remove), `BiometricError` (Unavailable, Cancelled, Failed, Invalidated, Platform), `NoBiometrics`. Shell: `ipc::device_settings` (`DeviceSettings {biometric_enabled, last_password_ms, reprompt_days = 7}` in `device.json` next to the vault, atomic write, damaged file = defaults so biometrics stays off; `MemorySettings` for tests), `ipc::biometric_service` (`biometric_status`, `biometric_enable(password)`, `biometric_disable`, `unlock_with_biometric`, `set_biometric_reprompt_days` 1-365), commands registered in all four lists, `BiometricStatus` DTO, new error codes (`password_due`, `biometric_unavailable/not_enabled/cancelled/failed/invalidated`). Rules: enabling re-verifies the password and is rolled back if the setting cannot be saved; typing the password (create/unlock/enable) restarts the period, a biometric unlock does not; a dropped OS key or a wrapped key that does not open this vault switches biometrics off and deletes the wrap; nothing is prompted once the password is due. `VaultService` now needs `C: Clone` (it keeps a clock for the period); `SystemClock` is `Clone`; `serde_json` moved to normal dependencies of the shell. `AppState` uses `NoBiometrics` until the Android plugin exists, so **biometrics report "not available" on every platform for now**. 17 tests with a fake phone (cancel, fail, invalidate, other key, due period, period change, password change, disable while locked, unwritable settings, no key in the settings); shell 137, clippy clean.

- 2026-11-26: UI done: Settings switch (asks the master password, then offers the re-prompt period), unlock-screen button (hidden when off or when the password is due, which shows a hint instead), a message per new error code; 8 UI tests, 187 in total. Android plugin written: `keystore.rs` (`AndroidBiometrics`, android-only, maps the plugin's reject codes to `BiometricError`; 3 platform-independent tests) and `gen/android/.../KeystorePlugin.kt` (AES-GCM Keystore key, user auth required per use, strong biometrics only, invalidated on enrolment; prompt with a CryptoObject; wrap = IV + ciphertext in `filesDir/biometric.wrap`; invalidation deletes key and wrap). Also `allowBackup=false`, `FLAG_SECURE`, dependency `androidx.biometric:biometric:1.1.0`. The APK builds (Rust and Kotlin compile); `AppState::new` now takes the provider (Android plugin, or `NoBiometrics` on desktop). Note: the plugin bridge carries the key as base64 in a JSON string, so a short-lived copy exists outside zeroising buffers on both sides; accepted for now (memory only), revisit in the security review (TASK-0013). The Kotlin source lives in `gen/android`, which is still uncommitted until ADR-0015 is accepted.

- 2026-11-26: Phone test (Pixel 10a, debug build): the user reported that the fingerprint switch in Settings works, and asked to close the task. Not reported, so **not verified by us**: the unlock-screen button after locking, cancelling the prompt, a wrong finger, the invalidation after adding a fingerprint, and the password re-prompt after the period. CI green on 076b3d3 (c3c6e05 failed typecheck for missing error messages; fixed by the UI commit).

## Next step
None. If the unverified phone cases misbehave, reopen as a new task.
