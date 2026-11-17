# TASK-0001: Define architecture and infrastructure

- Status: done
- Created: 2026-11-12
- Updated: 2026-11-17
- Related ADRs: ADR-0001 .. ADR-0010

## Goal
Agree on stack, security model, vault format, import/export scope, extensibility, sync, repo layout and process — before writing any product code.

## Acceptance criteria
- [x] ADR-0002..0009 each accepted or rejected by the user (all accepted 2026-11-12)
- [x] Repo skeleton (layout, CI, lint, test harness) specified in a follow-up task (TASK-0002, done)
- [x] MVP scope written down as tasks (TASK-0003 .. TASK-0013, created 2026-11-17)

## Decisions & notes (append-only, dated)
- 2026-11-12: User delegated language/platform choice to Claude; allowed push-back. Linux/Windows/Android now, macOS/iOS later.
- 2026-11-12: User wants a web-style UI with shadcn/ui + Tailwind → ADR-0002 revised to Tauri 2 (Rust core + React UI); seeds must never enter JS.
- 2026-11-12: User clarified: only OTP codes in scope; biometrics Android-only, desktop = password → ADR-0003 revised.
- 2026-11-12: User wants HOTP + Steam Guard (+ "COTP", meaning unclear) and OTP types as installable extensions → ADR-0007 (WASM sandbox, built-ins via same trait, host post-MVP).
- 2026-11-12: User wants sync via user-chosen cloud file → ADR-0008 (phased providers); tombstones added to vault format (ADR-0004).

- 2026-11-12: User wants the project in a git repo → `git init -b main` done, `.gitignore` added (blocks vault/keystore/.env files); initial commit pending because no git identity is configured on this machine.
- 2026-11-12: "COTP" = counter-based OTP = HOTP. Open source confirmed; ADR-0009 proposes GPL-3.0-or-later. User open to other stacks; Claude re-evaluated and still recommends Tauri 2.

- 2026-11-12: User not a Rust developer but OK with Tauri/Rust if Claude is comfortable → ADR-0002 accepted. User chose AGPL-based, open-source, free-to-use → ADR-0009 revised to AGPL-3.0-or-later and accepted. Git identity still pending (user asked how to provide it; answer: name + email in chat, set repo-local).

- 2026-11-12: ADR-0003 accepted with change: biometrics via per-platform provider interface; MVP Android only, Windows Hello after MVP, Apple later, Linux password-only. Remaining to review: ADR-0004..0008 (going through them one by one with the user).

- 2026-11-12: ADR-0004 accepted as written (MVP importers: otpauth URI/QR, Google migration, Aegis, 2FAS). Remaining: ADR-0005..0008.

- 2026-11-12: ADR-0005 accepted (stale licence line fixed to AGPL-3.0-or-later; CI = GitHub Actions). Remaining: ADR-0006..0008.

- 2026-11-12: ADR-0006 accepted with edits (Windows Hello added to post-MVP list; open questions closed). Remaining: ADR-0007, ADR-0008.

- 2026-11-12: ADR-0007 accepted as written. Remaining: ADR-0008.

- 2026-11-12: ADR-0008 accepted as written. All ADRs 0001-0009 are now accepted.

## Progress log (append-only, dated)
- 2026-11-12: Created docs scaffolding (CLAUDE.md, ADR + task templates). Drafted ADRs 0002-0006.
- 2026-11-12: Revised 0002-0006 for Tauri/web UI; added ADR-0007 (extensions) and ADR-0008 (sync).

## Next step
None — done. 2026-11-17: MVP broken into TASK-0003..0013 (dependency order is in each task's notes). Start with TASK-0003 (OTP engine) and TASK-0005 (vault format; begins with a format spec for user review). ADR-0010's second login factor remains open but is post-MVP.
