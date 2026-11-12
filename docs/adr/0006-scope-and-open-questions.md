# ADR-0006: MVP scope, non-goals, open questions

- Status: proposed (revised 2026-11-12)
- Date: 2026-11-12
- Related tasks: TASK-0001

## MVP (proposed)
Create/lock/unlock vault; add via QR scan (Android), image file, pasted URI or manual entry; live code view with countdown, copy, search, tags; edit/delete/reorder; **OTP types: TOTP, HOTP, Steam Guard** (all through the OTP-type interface, ADR-0007); import (otpauth, Google migration, Aegis, 2FAS); encrypted full/partial export; auto-lock; Android biometric unlock; password unlock on desktop.

## Post-MVP, in order
1. Sync phase 1 (file/folder provider) — ADR-0008
2. Extension host (WASM OTP types) — ADR-0007
3. More importers; WebDAV; direct Google Drive/OneDrive
4. macOS, iOS

## Non-goals
Push-based approvals; browser extension/autofill; any server component.

## Clarified by user (2026-11-12)
- Only OTP codes are in scope (not push/other MFA methods); biometrics are Android-only.

- Counter-based OTP is standardised as **HOTP** (RFC 4226); that is the term used everywhere in code, UI and docs.
- Open source confirmed; user is open to other language/UI combos (Claude re-evaluated, still recommends Tauri 2).

## Open questions
1. Final go on Tauri 2 stack (ADR-0002)?
2. Licence choice: GPL-3.0-or-later (recommended) vs Apache-2.0 (ADR-0009).
