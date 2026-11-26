# ADR-0016: Android biometric unlock — own Kotlin plugin, key-bound Keystore wrap, core unlocks with a key

- Status: accepted
- Date: 2026-11-26
- Related tasks: TASK-0023 (also TASK-0012)
- Refines: ADR-0003 (biometric provider model), ADR-0002 (Kotlin plugin for Keystore/biometric), ADR-0011 (one password slot, biometric wraps outside the file)

## Context
ADR-0003 decided *what* biometric unlock is: the vault key (VK) is additionally wrapped by an OS key that is cryptographically bound to the biometric; the wrapped copy is device-local, never synced; the master password stays mandatory and is the fallback; the password is required again periodically. It did not decide how this reaches the app on Android. Tauri's official `biometric` plugin only shows a prompt (`authenticate`) and returns "yes"; a prompt result is not a cryptographic binding, which ADR-0003 explicitly rules out ("not a mere user-present check").

## Options considered
1. **Official `tauri-plugin-biometric`** (prompt only). Quick, but anyone who can call the plugin or patch the result gets the key; no binding.
2. **Own small Tauri plugin in Kotlin** (`androidx.biometric` `BiometricPrompt` with a `CryptoObject`): an AES-GCM key in the Android Keystore with `setUserAuthenticationRequired(true)`, strong biometrics only, `setInvalidatedByBiometricEnrollment(true)`. The VK is encrypted with it; decrypting needs a successful biometric on that very cipher. Rust passes the VK in once to enrol and receives it back after a successful prompt.
3. **Store the VK in `EncryptedSharedPreferences`/Keystore without a per-use prompt**. Not bound to the biometric; rejected by ADR-0003.

## Decision
Option 2, in three layers, each testable on its own:
1. **Core (`mfa-core`)**: `VaultSession::export_key(password)` (re-verifies the password, returns the VK in a zeroising buffer, only while unlocked) and `VaultSession::unlock_with_key(key)`. A wrong key and a damaged header are indistinguishable (`UnwrapFailed`). The VK is unchanged by a password change (only the slot is rewrapped), so the wrapped copy stays valid; changing the password does not remove biometric unlock.
2. **Shell (`app/src-tauri`)**: a `BiometricProvider` trait (`available`, `enrol(key)`, `unlock() -> key`, `remove`) with an `Unavailable` implementation for desktop and a Tauri-plugin implementation on Android; commands `biometric_status`, `biometric_enable(password)`, `biometric_disable`, `unlock_with_biometric`. The shell keeps device-local state: whether biometrics is enabled and when the password was last typed; after 7 days (configurable) `unlock_with_biometric` refuses and the UI asks for the password. After a failed or cancelled prompt the UI falls back to the password.
3. **Android plugin (Kotlin)**: the Keystore key, the prompt, the wrapped-VK blob in app-private storage. Key invalidation (new fingerprint enrolled) removes the blob; the UI then asks for the password and offers to enable again. `allowBackup=false` and `FLAG_SECURE` ship with it (TASK-0012).

The VK crosses the Rust/Kotlin boundary in memory only, as bytes; nothing is logged; the UI never sees it.

## Consequences
- A thief with an unlocked phone and the user's enrolled biometrics can open the vault; that is the accepted trade-off of any biometric unlock (ADR-0003). Without them, the wrapped blob is useless.
- New biometric enrolment on the phone silently disables the shortcut (by design).
- Windows Hello and Touch ID later implement the same trait.
- Needs a real device to test the prompt (the user's phone), so layers 1 and 2 are covered by automated tests with a fake provider and layer 3 by a manual check recorded in the task.
