# TASK-0014: Import from a password-protected ZIP (WinAuth)

- Status: done
- Created: 2026-11-24
- Updated: 2026-11-24
- Related ADRs: ADR-0014 (accepted), ADR-0004, ADR-0013

## Goal
The import wizard accepts a password-protected ZIP as WinAuth writes it: pick the `.zip`, enter its password, see the same preview as for a text file, apply as before. Wrong passwords, damaged or hostile archives fail with a readable message and change nothing.

## Acceptance criteria
- [x] ADR-0014 accepted
- [x] `mfa-formats`: archive reader (one entry, limits, zeroising password), feeding the existing link-list importer
- [x] Shell: preview command for archives (base64 bytes + password); password never stored, logged or returned; no-secrets tripwire still green
- [x] UI: wizard detects an archive by content, asks for the password, clears it afterwards
- [x] Tests with invented archives (cases listed in ADR-0014)
- [ ] The user's real zip opens in an opt-in ignored test (counts only, nothing copied) — test written, needs the password from the user
- [ ] CI green

## Decisions & notes (append-only, dated)
- 2026-11-24: Sample inspected by headers only: one entry, encrypted flag set, deflate, no AES marker, so classic ZipCrypto. Entry name and sizes seen; contents not read.

## Progress log (append-only, dated)
- 2026-11-24: Task created; ADR-0014 written as proposed.

- 2026-11-24: Done in one pass. `mfa-formats::archive` (`is_zip`, `read_single_file`, `ArchiveError`; zeroising output; 12 tests with archives built in the test: right/wrong/missing password, damaged data, one file only, folders ignored, entry-count cap, declared-size bomb refused from header, read capped regardless of header). Shell: `import_preview(format, file_base64, password)` replaces the text version (one path for text and archives; UTF-8/UTF-16 decoding now only in Rust); new error code `password_required`; password held in `Zeroizing`, never stored or echoed (tested); 3 new service tests (99 total). UI: file is read as base64 (5 MiB cap), a password prompt appears only when the core says one is needed, wrong password keeps the prompt, choosing another file forgets password and file; `read-text-file` removed (4 new UI tests, 148 total). Dependency checks: `cargo deny` licences/bans/sources ok in both workspaces. **Not verified:** the real zip (needs the user to run the ignored test with their password: `MFA_SAMPLE_ZIP`, `MFA_SAMPLE_ZIP_PASSWORD`); a zip written by 7-Zip/AES is refused as "wrong password, or the file is damaged" (AES left out by ADR-0014); CI; the wizard in the real app. Known limit: `ZipArchive` reads the file into memory in the crate, and plaintext lives in a zeroising buffer only until the importer finishes; intermediate copies made by the decoder are not zeroised.

- 2026-11-26: Closed on the user's instruction. The user then tried a real password-protected zip with its password and reported it works.

## Next step
None.
