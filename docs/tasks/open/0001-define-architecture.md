# TASK-0001: Define architecture and infrastructure

- Status: in-progress
- Created: 2026-11-12
- Updated: 2026-11-12
- Related ADRs: ADR-0001 .. ADR-0008

## Goal
Agree on stack, security model, vault format, import/export scope, extensibility, sync, repo layout and process — before writing any product code.

## Acceptance criteria
- [ ] ADR-0002..0008 each accepted or rejected by the user
- [ ] Repo skeleton (layout, CI, lint, test harness) specified in a follow-up task
- [ ] MVP scope written down as tasks

## Decisions & notes (append-only, dated)
- 2026-11-12: User delegated language/platform choice to Claude; allowed push-back. Linux/Windows/Android now, macOS/iOS later.
- 2026-11-12: User wants a web-style UI with shadcn/ui + Tailwind → ADR-0002 revised to Tauri 2 (Rust core + React UI); seeds must never enter JS.
- 2026-11-12: User clarified: only OTP codes in scope; biometrics Android-only, desktop = password → ADR-0003 revised.
- 2026-11-12: User wants HOTP + Steam Guard (+ "COTP", meaning unclear) and OTP types as installable extensions → ADR-0007 (WASM sandbox, built-ins via same trait, host post-MVP).
- 2026-11-12: User wants sync via user-chosen cloud file → ADR-0008 (phased providers); tombstones added to vault format (ADR-0004).

- 2026-11-12: User wants the project in a git repo → `git init -b main` done, `.gitignore` added (blocks vault/keystore/.env files); initial commit pending because no git identity is configured on this machine.
- 2026-11-12: "COTP" = counter-based OTP = HOTP. Open source confirmed; ADR-0009 proposes GPL-3.0-or-later. User open to other stacks; Claude re-evaluated and still recommends Tauri 2.

- 2026-11-12: User not a Rust developer but OK with Tauri/Rust if Claude is comfortable → ADR-0002 accepted. User chose AGPL-based, open-source, free-to-use → ADR-0009 revised to AGPL-3.0-or-later and accepted. Git identity still pending (user asked how to provide it; answer: name + email in chat, set repo-local).

## Progress log (append-only, dated)
- 2026-11-12: Created docs scaffolding (CLAUDE.md, ADR + task templates). Drafted ADRs 0002-0006.
- 2026-11-12: Revised 0002-0006 for Tauri/web UI; added ADR-0007 (extensions) and ADR-0008 (sync).

## Next step
Await git identity (name + email) from user, then make the initial commit. User still to review ADR-0003..0008 (security model, formats, layout, scope, extensions, sync) for accept/reject. Then open TASK-0002 (repo skeleton).
