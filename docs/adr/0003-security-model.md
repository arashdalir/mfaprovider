# ADR-0003: Vault encryption and authentication model

- Status: accepted (2026-11-12; biometrics generalised to a per-platform provider model, MVP = Android only)
- Date: 2026-11-12
- Related tasks: TASK-0001

## Context
A biometric is a convenience for releasing a secret, not a secret itself.

## Decision
- **Master password is mandatory** on every platform and is always the fallback.
- Vault is encrypted with a random 256-bit **vault key (VK)**, wrapped by a key derived from the password via **Argon2id** (parameters tuned per device, stored in the header, never below OWASP minimums).
- Cipher: **XChaCha20-Poly1305**; authenticated header; versioned format.
- **Biometric unlock via a per-platform `BiometricProvider` interface in the Rust core.** A provider is allowed only where the OS can cryptographically bind a key to the biometric (not a mere "user present" check). The VK is additionally wrapped by that OS-bound key; this wrapped copy is device-local and never synced.
  - **MVP: Android** — Keystore key, user authentication required, strong biometrics, invalidated on enrollment change.
  - **After MVP: Windows Hello** (TPM-backed key), then **Touch ID / Face ID** on macOS/iOS when those ports exist.
  - **Linux: password only** — fingerprint stacks (fprintd) cannot hold a key.
- Auto-lock on timeout/background/suspend; zeroize VK on lock; clipboard auto-clear; Android `FLAG_SECURE`; `allowBackup=false`; no secrets in logs/crash reports; no network access in the app until sync is enabled.
- Periodically require the password even with biometrics (configurable).

## Consequences
+ Standard, reviewable design; biometrics removable without data loss.
- A forgotten password means an unrecoverable vault (encrypted export is the backup story).
- Argon2 needs tuning on low-end Android.
