# Session log: architecture discussion (2026-11-12)

Condensed record of the first session. The authoritative state lives in the ADRs and in `docs/tasks/open/`; this file only explains how we got here.

## Request
Multi-OS authenticator app (like WinAuth, but extended): show TOTPs, import/export (partial or complete), secured by password/biometrics. Windows, Linux, Android now; Apple platforms later. Clean-code rules; decisions as ADRs; one document per task so a restarted session can resume. Base folder: `d:/projects/mfa-provider`.

## Timeline of decisions
1. Claude proposed Rust core + Flutter UI, password-mandatory vault with Argon2id + XChaCha20-Poly1305, Android/Windows biometrics, rich import/export (ADR-0002..0006).
2. User wanted a web-style UI with shadcn/ui + Tailwind, clarified that only OTP codes matter (not push MFA) and that biometrics are Android-only.
   → Stack revised to Tauri 2 (Rust core + React UI); seeds never enter JavaScript (ADR-0002, 0003).
3. User asked about Rust on Android: yes, via NDK targets; Tauri automates it.
4. User wants HOTP and Steam Guard, and OTP types as installable extensions.
   → ADR-0007: built-ins implemented through the same trait; third-party types as sandboxed WASM, host after MVP.
5. User wants sync via a user-chosen cloud file.
   → ADR-0008: phased providers (file/folder + Android SAF, WebDAV, then direct Drive/OneDrive); vault format gets tombstones (ADR-0004).
6. User asked for a git repo → initialised on `main`, `.gitignore` blocks vault/keystore/.env files.
7. "COTP" = counter-based OTP = HOTP; the project uses the standard term HOTP. Open source confirmed; GPL-3.0-or-later proposed (ADR-0009).

## Pushback Claude raised (kept for context)
- Push approvals/number matching of Microsoft/Google cannot be replicated (user later clarified this was never in scope).
- Biometrics are a convenience, never the only lock; password stays mandatory.
- Linux/desktop biometrics unrealistic in v1.
- Extensions see seeds, hence the WASM sandbox; extension host deferred past MVP.

## Still open (see TASK-0001)
- Final go on Tauri 2 stack; licence choice (GPL-3.0-or-later vs Apache-2.0); git identity for the first commit.
