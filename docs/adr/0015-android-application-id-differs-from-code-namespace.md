# ADR-0015: Android application id keeps the underscore; the code namespace does not

- Status: proposed
- Date: 2026-11-26
- Related tasks: TASK-0012
- Refines: ADR-0001 (stack)

## Context
The user's application id is `dev.area_51.android.mfaprovider`. Android allows underscores in an application id and in package names. Tauri 2 rejects them: it validates the `identifier` in `tauri.conf.json` as alphanumerics, hyphens and periods only, and it derives the generated Kotlin package, the JNI symbol names and the Rust-side binding from that identifier. Renaming the id for good would also move the desktop app-data folder, which the desktop build names after the identifier.

## Options considered
1. **Give up the underscore everywhere** (`dev.area51.android.mfaprovider`). Simplest; changes the user's id and the desktop data folder.
2. **Keep the id on desktop, use another id on Android only** by overriding `identifier`. The installed app is then not the user's id.
3. **Keep the user's id as the Android `applicationId`, use a valid identifier as the code namespace.** Gradle separates `namespace` (where code and `R` live) from `applicationId` (what the phone, stores and signing see). An Android-only config file (`tauri.android.conf.json`) sets the valid identifier; `applicationId` is edited in `gen/android/app/build.gradle.kts`.

## Decision
Option 3. Desktop keeps `identifier = dev.area_51.android.mfaprovider` in `tauri.conf.json` (unchanged data folder). `tauri.android.conf.json` sets `identifier = dev.area51.android.mfaprovider` so the generated Kotlin package, JNI names and Rust binding agree. `gen/android/app/build.gradle.kts` sets `applicationId = "dev.area_51.android.mfaprovider"` (debug builds add `.debug`). `gen/android` is committed (Tauri's advice) because both the application id and `MainActivity` are edited there; `tauri android init` must not be re-run over it.

## Consequences
- The app is installed under the user's real id; signing and any later store listing use it.
- Two spellings of the id exist in the repo on purpose; this ADR is the explanation.
- Re-running `tauri android init` would reset `applicationId` and `MainActivity.kt`; the check is a one-line grep of the generated file in the Android build step (to add when the Android CI job exists).
- If Tauri one day allows underscores, this ADR is superseded by option 1's simplicity.
