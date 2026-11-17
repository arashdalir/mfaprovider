# TASK-0012: Android build, Keystore biometric unlock, camera QR scan

- Status: open
- Created: 2026-11-17
- Updated: 2026-11-17
- Related ADRs: ADR-0002, ADR-0003, ADR-0006

## Goal
Make the app build and run on Android: toolchain and CI job, `BiometricProvider` with an Android Keystore implementation (key bound to strong biometrics, invalidated on enrollment change, device-local wrapped vault key), `FLAG_SECURE`, `allowBackup=false`, and camera QR scanning feeding the same entry-input path as TASK-0009.

## Acceptance criteria
- [ ] Android SDK/NDK setup documented; `tauri android build` works locally and in CI
- [ ] `BiometricProvider` interface in core; Android Keystore implementation as a small Kotlin Tauri plugin
- [ ] Biometric unlock works, falls back to master password, periodic password re-prompt (default 7 days, configurable)
- [ ] `FLAG_SECURE`, `allowBackup=false`, no secrets in logs/crash reports
- [ ] Camera QR scan (permission requested only when used)
- [ ] Tested on a real device or emulator; results recorded here
- [ ] Application ID is `dev.area_51.android.mfaprovider`

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: 0006, 0007, 0009. Largest uncertainty in the MVP (Tauri mobile is younger; custom plugin). Do a spike early if schedule allows; expect to need the user's device or an emulator image. Windows Hello is post-MVP (ADR-0003/0006).

## Progress log (append-only, dated)
- 2026-11-17: Task created.

## Next step
Spike: install Android SDK/NDK, run `pnpm tauri android init`, and record whether the empty skeleton builds and launches on an emulator.
