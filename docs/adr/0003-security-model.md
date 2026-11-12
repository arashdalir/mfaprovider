# ADR-0003: Vault encryption and authentication model

- Status: proposed (revised 2026-11-12: biometrics are Android-only; desktop = password)
- Date: 2026-11-12
- Related tasks: TASK-0001

## Context
A biometric is a convenience for releasing a secret, not a secret itself.

## Decision (proposed)
- **Master password is mandatory** on every platform and is always the fallback.
- Vault is encrypted with a random 256-bit **vault key (VK)**, wrapped by a key derived from the password via **Argon2id** (parameters tuned per device, stored in the header, never below OWASP minimums).
- Cipher: **XChaCha20-Poly1305**; authenticated header; versioned format.
- **Android only: biometric unlock.** VK is additionally wrapped by an Android Keystore key (user authentication required, strong biometrics, invalidated on enrollment change). This wrapped copy is device-local and never synced.
- **Windows/Linux/macOS: password only.** (Optional OS-keystore convenience can be a later ADR.)
- Auto-lock on timeout/background/suspend; zeroize VK on lock; clipboard auto-clear; Android `FLAG_SECURE`; `allowBackup=false`; no secrets in logs/crash reports; no network access in the app until sync is enabled.
- Periodically require the password even with biometrics (configurable).

## Consequences
+ Standard, reviewable design; biometrics removable without data loss.
- A forgotten password means an unrecoverable vault (encrypted export is the backup story).
- Argon2 needs tuning on low-end Android.
