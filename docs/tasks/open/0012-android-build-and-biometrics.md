# TASK-0012: Android build, Keystore biometric unlock, camera QR scan

- Status: in-progress
- Created: 2026-11-17
- Updated: 2026-11-26
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

- 2026-11-26: Spike started at the user's request. State found on this machine: Android Studio installed with a partial SDK in `%LOCALAPPDATA%AndroidSdk` (platforms 33/35/36/36.1/37, build-tools 34–37, emulator, platform-tools; **no** NDK, **no** cmdline-tools/sdkmanager, **no** system image, no AVD); JDK only as Android Studio's bundled `jbr`; `ANDROID_HOME`/`JAVA_HOME` unset; no Rust Android targets; WHPX usable so an x86_64 emulator can run; no physical device attached. CI of c9ea0fa is green. Plan: add Rust targets, fetch Google's command-line tools into the SDK, install an NDK and one x86_64 system image, create an AVD, `tauri android init`, build, run, record each result here.

- 2026-11-26: Spike results so far. Installed (outside the repo, in the user's Android SDK): cmdline-tools (from Google, sha256 90ae805d…b04a), NDK r29 (29.0.14206865), system image android-35 google_apis x86_64; Rust targets aarch64/armv7/i686/x86_64-linux-android. `tauri android init` works. **Findings:** (1) `tauri android build` refuses the version `0.0.0` (Android needs >= 0.0.1): `tauri.conf.json` version set to `0.0.1`. (2) **Tauri rejects the chosen application ID `dev.area_51.android.mfaprovider`** at build time (identifiers may only contain letters, digits, hyphens and periods; Android itself allows underscores); `init` accepts it, `build` does not, and the generated Kotlin/JNI package names follow the identifier, so an underscore cannot simply be patched in later. Needs a decision from the user. For the spike only, a throwaway id `dev.area51.android.mfaprovider` is passed with `--config` (nothing committed). (3) **The whole Rust app, including mfa-core/formats/storage, compiles for `x86_64-linux-android`** (dev profile, 53 s). (4) Build then stops at the Gradle step: Tauri symlinks `libapp_lib.so` into `jniLibs`, and Windows refuses symlinks without **Developer Mode** (an OS setting only the user can switch on). `gen/` is untracked and not committed yet.

## Next step
User enables Windows Developer Mode and decides the application id; then rerun `tauri android build --debug --target x86_64 --apk --config {identifier: dev.area51...}`, create an AVD from the android-35 image, install and launch, record results.
