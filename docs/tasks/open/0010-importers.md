# TASK-0010: Importers — Google migration, Aegis, 2FAS

- Status: in-progress
- Created: 2026-11-17
- Updated: 2026-11-24
- Related ADRs: ADR-0004, ADR-0006

## Goal
Importers behind one `Importer` interface in `mfa-formats` for Google Authenticator `otpauth-migration://`, Aegis (plain + encrypted) and 2FAS, with the preview-then-merge flow: new / duplicate / conflicting entries, per-entry skip, replace or keep-both. Duplicate = same issuer + account + secret.

## Acceptance criteria
- [ ] `Importer` interface; one isolated codec per source
- [ ] Fixtures per format (including encrypted Aegis) with provenance noted; no real seeds in the repo
- [ ] Merge planner returns a preview without mutating the vault; applying it is a separate step
- [ ] UI import wizard: pick file/URI → preview → apply
- [ ] Corrupt/unknown-version files fail cleanly; imports never log secrets
- [ ] CI green

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: 0007, 0009. Later importers (andOTP, Bitwarden, WinAuth) are post-MVP (ADR-0006).

- 2026-11-24: **Order changed at the user's request: WinAuth first.** The user's WinAuth export (not committed, never copied into the repo) is a plain text file of one `otpauth://totp/…` link per line (58 lines, CRLF, WinAuth adds `icon=WinAuth`, no issuer on most). It is the same format as a plaintext otpauth export, so the first codec is a generic **link-list importer** reusing `parse_otpauth`; Google migration, Aegis and 2FAS follow on the same interface. Planned in stages: (1) `Importer` trait + link-list importer + merge planner (core: atomic `apply_import`), (2) shell commands (`list_import_formats`, `import_preview`, `import_apply`, `import_discard`; pending batch dropped on lock like drafts), (3) UI wizard. Decisions by Claude: duplicate = same issuer + account (trimmed, case-insensitive) + secret; same name but different secret = conflict; the same line twice in one file = duplicate of the earlier one; bad lines are reported per line and never block the good ones; a file is read by the webview and sent as text (user-supplied input, consistent with the ADR-0012 clarification) — a Rust-side file dialog would keep seeds out of JS and can replace this later; files up to 5 MiB and 5000 entries.

## Progress log (append-only, dated)
- 2026-11-17: Task created.

## Next step
After TASK-0009: define the `Importer` trait and merge-planner types, then implement the Google migration decoder (protobuf) test-first.
