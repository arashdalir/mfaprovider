# TASK-0010: Importers — Google migration, Aegis, 2FAS

- Status: in-progress
- Created: 2026-11-17
- Updated: 2026-11-24
- Related ADRs: ADR-0004, ADR-0006

## Goal
Importers behind one `Importer` interface in `mfa-formats` for Google Authenticator `otpauth-migration://`, Aegis (plain + encrypted) and 2FAS, with the preview-then-merge flow: new / duplicate / conflicting entries, per-entry skip, replace or keep-both. Duplicate = same issuer + account + secret.

## Acceptance criteria
- [ ] `Importer` interface; one isolated codec per source (interface, link-list and Google migration codecs done; Aegis, 2FAS open)
- [ ] Fixtures per format (including encrypted Aegis) with provenance noted; no real seeds in the repo
- [x] Merge planner returns a preview without mutating the vault; applying it is a separate step
- [x] UI import wizard: pick file → preview → apply (file only; URI paste via TASK-0009; not clicked through in the real app)
- [ ] Corrupt/unknown-version files fail cleanly; imports never log secrets
- [ ] CI green

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: 0007, 0009. Later importers (andOTP, Bitwarden, WinAuth) are post-MVP (ADR-0006).

- 2026-11-24: **Order changed at the user's request: WinAuth first.** The user's WinAuth export (not committed, never copied into the repo) is a plain text file of one `otpauth://totp/…` link per line (58 lines, CRLF, WinAuth adds `icon=WinAuth`, no issuer on most). It is the same format as a plaintext otpauth export, so the first codec is a generic **link-list importer** reusing `parse_otpauth`; Google migration, Aegis and 2FAS follow on the same interface. Planned in stages: (1) `Importer` trait + link-list importer + merge planner (core: atomic `apply_import`), (2) shell commands (`list_import_formats`, `import_preview`, `import_apply`, `import_discard`; pending batch dropped on lock like drafts), (3) UI wizard. Decisions by Claude: duplicate = same issuer + account (trimmed, case-insensitive) + secret; same name but different secret = conflict; the same line twice in one file = duplicate of the earlier one; bad lines are reported per line and never block the good ones; a file is read by the webview and sent as text (user-supplied input, consistent with the ADR-0012 clarification) — a Rust-side file dialog would keep seeds out of JS and can replace this later; files up to 5 MiB and 5000 entries.

## Progress log (append-only, dated)
- 2026-11-17: Task created.

- 2026-11-24: Stage 1 done. Core: `ImportOp` + `VaultData::apply_import` (all-or-nothing: validates every op, rejects reused/duplicate ids and double replaces, then applies) + `VaultSession::{import, new_entry_id}` (one save); +7 tests. `mfa-formats`: `Importer` trait + `all_importers`/`importer`, shared text decoding (UTF-8, BOM, UTF-16 LE/BE), `LinkListImporter` (id `otpauth-list`: blank/`#` lines ignored, bad lines reported per line number with fixed sentences, size 5 MiB / 5000 items), `merge` (`plan_merge` O(n) via name index: New / DuplicateOfVault / DuplicateOfEarlierItem / ConflictWithVault; `Decision` with defaults Add for new+conflict, Skip for duplicates; `build_ops` with replace checks). Tests: formats 70 (import 13, merge 16), core 156. **Real export check** (opt-in ignored test reading `MFA_SAMPLE_EXPORT`, prints counts only, nothing copied): the user's WinAuth file = 58 accounts read, 0 skipped, 58 new in an empty vault, 8 with issuer.

- 2026-11-24: Stage 2 done. Shell: DTOs (`ImportFormatView`, `ImportPreview`/`ImportItemView`, `ImportChoice`, `ImportResult`, `ImportDecision`, `ImportStatus`), pending import in `VaultService` (replaced by a new preview, dropped on lock/idle/tick), commands `list_import_formats`, `import_preview`, `import_apply`, `import_discard`; apply re-plans against the current vault, treats items without a choice as skipped, refuses unknown/repeated item ids and impossible replaces, and is one saved change; after a failed save the file is not offered again (the change is in memory and unsaved, so a retry would double-add). 25 new shell tests (96 total). **Deviation from the task text (Claude's decision):** a new status for the *same secret under a different name* (flagged `already_under_other_name`, skipped by default, replaceable) — otherwise an issuer-less WinAuth export would silently duplicate accounts added by hand with an issuer. The no-secrets tripwire flagged my first name for it (`SameSecret`); renamed rather than loosening the tripwire. A test RNG that wrapped after 256 bytes made generated ids collide; the core's atomic import refused it correctly with nothing changed, test RNG fixed.

- 2026-11-24: Stage 3 done (UI): `features/import` — `readTextFile` (UTF-8, UTF-16 with BOM, fatal decoding, 5 MiB), `ImportScreen` (format + file → preview → done), `ImportPreviewStep` (per-item checkbox for new accounts, select for duplicates/conflicts with Replace only where possible, bulk Skip all / Suggested choices, live "Will add/replace/skip" line, unreadable lines listed with line numbers, failures keep the review open, Cancel discards the pending file), "Import" button on the vault screen. UI tests: 144 (20 new). **WinAuth path is complete end to end**; the user's real export (58 accounts) imports cleanly in the core check.
- 2026-11-24: Local test-runner anomaly, not understood: after the stage-3 files were added, `vitest` with the default pool failed every file with "failed to find the current suite" (setup.ts), also at the previously green committed state, while `--pool=vmThreads` passes all 144. A `tauri dev` window and its Vite server were running on the machine at the time; cache moved aside, no effect. Not reproduced in CI so far; to be confirmed there. If CI shows it, investigate for real instead of changing the pool.
- 2026-11-24: Not done: Google `otpauth-migration://`, Aegis (plain + encrypted), 2FAS importers; the real-app click-through of the import wizard.

- 2026-11-24: CI green on ada69f0 (all five jobs), including the UI tests under the default vitest pool: the local "failed to find the current suite" anomaly did not reproduce on the clean CI machine, so it is treated as a local-environment problem (a `tauri dev` session was running); left as is, to be revisited only if it returns.

- 2026-11-26: **Google `otpauth-migration://` done** (core, shell, UI hint). `mfa-formats::migration`: `parse_migration_link` reads `data=` (percent-decoded; standard or URL-safe base64, padded or not; a raw `+` stays a plus) and a **minimal hand-written protobuf reader** (varints capped at 10 bytes, every length checked, unknown fields and fixed-size wire types skipped; no protobuf dependency — Claude's decision, recorded here instead of an ADR because it is ~100 lines, reversible and fully covered by tests). Fields used: secret, name, issuer, algorithm (unspecified/SHA-1 → default, SHA-256, SHA-512; MD5 skipped), digits (6 default, 8), type (HOTP with counter, TOTP), batch fields ignored. Unusable accounts are skipped with a fixed sentence and never fail the link; name `Issuer:account` is split when the issuer field is empty. `GoogleMigrationImporter` (id `google-migration`): one link per line, several links (batched exports) in one file. The Add-account link paste and QR image paths now accept migration links too (one draft per account). Tests: formats 97 (+14), shell 103 (+4); fixtures are invented byte-by-byte, no real export seen. **Not verified:** a real Google export (the user has not tried one); Google sometimes splits large exports into several QR codes, each must be scanned/pasted; the wizard in the real app.

## Next step
Wait for CI. Then next importers on the same interface: Google `otpauth-migration://` (protobuf; QR codes from TASK-0009 already decode to the link text), Aegis plain, Aegis encrypted, 2FAS. Each with invented fixtures only.
