# TASK-0012: Android build, Keystore biometric unlock, camera QR scan

- Status: in-progress
- Created: 2026-11-17
- Updated: 2026-11-26
- Related ADRs: ADR-0002, ADR-0003, ADR-0006

## Goal
Make the app build and run on Android: toolchain and CI job, `BiometricProvider` with an Android Keystore implementation (key bound to strong biometrics, invalidated on enrollment change, device-local wrapped vault key), `FLAG_SECURE`, `allowBackup=false`, and camera QR scanning feeding the same entry-input path as TASK-0009.

## Acceptance criteria
- [ ] Android SDK/NDK setup documented; `tauri android build` works locally and in CI
- [x] `BiometricProvider` interface in core; Android Keystore implementation as a small Kotlin Tauri plugin
- [ ] Biometric unlock works, falls back to master password, periodic password re-prompt (default 7 days, configurable)
- [ ] `FLAG_SECURE`, `allowBackup=false` (done, in gen/android), lock when the app goes to the background (done 2026-11-26, see log), no secrets in logs/crash reports (not yet reviewed)
- [x] Camera QR scan (permission requested only when used)
- [ ] Tested on a real device or emulator; results recorded here
- [x] Application ID is `dev.area_51.android.mfaprovider`

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: 0006, 0007, 0009. Largest uncertainty in the MVP (Tauri mobile is younger; custom plugin). Do a spike early if schedule allows; expect to need the user's device or an emulator image. Windows Hello is post-MVP (ADR-0003/0006).

## Progress log (append-only, dated)
- 2026-11-17: Task created.

- 2026-11-26: Spike started at the user's request. State found on this machine: Android Studio installed with a partial SDK in `%LOCALAPPDATA%AndroidSdk` (platforms 33/35/36/36.1/37, build-tools 34–37, emulator, platform-tools; **no** NDK, **no** cmdline-tools/sdkmanager, **no** system image, no AVD); JDK only as Android Studio's bundled `jbr`; `ANDROID_HOME`/`JAVA_HOME` unset; no Rust Android targets; WHPX usable so an x86_64 emulator can run; no physical device attached. CI of c9ea0fa is green. Plan: add Rust targets, fetch Google's command-line tools into the SDK, install an NDK and one x86_64 system image, create an AVD, `tauri android init`, build, run, record each result here.

- 2026-11-26: Spike results so far. Installed (outside the repo, in the user's Android SDK): cmdline-tools (from Google, sha256 90ae805d…b04a), NDK r29 (29.0.14206865), system image android-35 google_apis x86_64; Rust targets aarch64/armv7/i686/x86_64-linux-android. `tauri android init` works. **Findings:** (1) `tauri android build` refuses the version `0.0.0` (Android needs >= 0.0.1): `tauri.conf.json` version set to `0.0.1`. (2) **Tauri rejects the chosen application ID `dev.area_51.android.mfaprovider`** at build time (identifiers may only contain letters, digits, hyphens and periods; Android itself allows underscores); `init` accepts it, `build` does not, and the generated Kotlin/JNI package names follow the identifier, so an underscore cannot simply be patched in later. Needs a decision from the user. For the spike only, a throwaway id `dev.area51.android.mfaprovider` is passed with `--config` (nothing committed). (3) **The whole Rust app, including mfa-core/formats/storage, compiles for `x86_64-linux-android`** (dev profile, 53 s). (4) Build then stops at the Gradle step: Tauri symlinks `libapp_lib.so` into `jniLibs`, and Windows refuses symlinks without **Developer Mode** (an OS setting only the user can switch on). `gen/` is untracked and not committed yet.

- 2026-11-26: **Spike ran on the user's real phone (Pixel 10a, Android 17, arm64) over Wi-Fi adb.** With Windows Developer Mode on, `tauri android build --debug --target aarch64 --apk --config {identifier: dev.area51.android.mfaprovider}` succeeds; the app installs (package name gets a `.debug` suffix), `MainActivity` starts and the process stays alive; no panic or crash in logcat, only WebView/DNS info lines. Lessons: (1) a debug build with symbols makes a 154 MB APK, and Gradle kept ~130 MB of stale data in the zip across rebuilds; building with `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_STRIP=symbols` after deleting `gen/android/app/build` gives 21 MB. (2) Wi-Fi adb here is only ~0.1 MB/s, so streamed installs timed out or arrived damaged ("Failed to load asset path"); `adb install --no-streaming` worked (231 s). USB would take seconds. (3) The phone runs private-space and work-profile users, so `adb shell monkey` / `resolve-activity` default to user 11 and fail; use `--user 0` and `am start -n <pkg>/dev.area51.android.mfaprovider.MainActivity`. **Not verified:** what the UI looks like and whether create-vault, entries, import and the camera work on the device (the user has to look); no biometrics, camera QR or background-lock events yet.

- 2026-11-26: Biometrics (TASK-0023), camera scan (TASK-0021) and the real application id are done and confirmed by the user on the Pixel. **Found a gap:** Settings promised "the vault also locks when the app goes to the background", and the core supports it (`SystemEvent::Backgrounded`), but nothing in the shell reported the event, so it never happened. Fixed: new command `report_backgrounded` (in all four command lists), and `SessionGate` calls it when the document becomes hidden while unlocked; the unlock screen then follows within the status poll. 2 UI tests (189). Side effect to know: any system screen that covers the app (the file picker during import, a permission dialog) hides the page and so locks the vault by the same rule. Not yet checked on the phone.
- 2026-11-26: **The UI-only background lock did not work on Windows** (the user reported it; I had only tested it with a simulated event). Measured with the real window: minimising gives native events `Resized(0x0)` with `is_minimized()=true` and then `Focused(false)`. Fix: the shell now locks from those window events (`lock_when_minimized` in lib.rs; clicking another window without minimising does not lock). The UI `visibilitychange` path stays for Android, where it is still unverified. The lock itself after a real minimise is not verified by us (we cannot unlock the user's vault); only that the events arrive.

- 2026-11-27: Phone test of `27283d6` (user): the camera scan does not lock the vault, the file picker does (so a file import could not be finished). The background lock and fingerprint checks were not reported separately. Fix per ADR-0017 (proposed): the two file inputs call `expect_file_picker`, the shell excuses one departure for at most 2 minutes (mobile only), `report_foregrounded` ends it. 7 shell tests (147) and 2 UI tests (191). Not yet verified on the phone.

- 2026-11-27: Phone test of `c4ef1b2` (user): import now works as expected. CI green. Not reported: QR from an image, locking after going to the home screen, and locking again after a picker.

- 2026-11-27: The user accepted ADR-0015, ADR-0016 and ADR-0017. `gen/android` committed (checked for local paths and secrets; Tauri's ignore files keep the machine-specific ones out).

- 2026-11-27: Added the CI job `Android debug APK` (builds arm64, checks the APK's application id) and `docs/android.md`; README status line updated.

## Next step
Confirm the Android CI job is green. Still to check on the phone: read a QR image through the picker (no lock); go to the home screen and back, also after using a picker (locks). Secrets-in-logs review and a signed release build go with TASK-0013.
