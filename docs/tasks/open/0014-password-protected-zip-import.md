# TASK-0014: Import from a password-protected ZIP (WinAuth)

- Status: open (blocked on ADR-0014 acceptance)
- Created: 2026-11-24
- Updated: 2026-11-24
- Related ADRs: ADR-0014 (proposed), ADR-0004, ADR-0013

## Goal
The import wizard accepts a password-protected ZIP as WinAuth writes it: pick the `.zip`, enter its password, see the same preview as for a text file, apply as before. Wrong passwords, damaged or hostile archives fail with a readable message and change nothing.

## Acceptance criteria
- [ ] ADR-0014 accepted
- [ ] `mfa-formats`: archive reader (one entry, limits, zeroising password), feeding the existing link-list importer
- [ ] Shell: preview command for archives (base64 bytes + password); password never stored, logged or returned; no-secrets tripwire still green
- [ ] UI: wizard detects an archive by content, asks for the password, clears it afterwards
- [ ] Tests with invented archives (cases listed in ADR-0014)
- [ ] The user's real zip opens in an opt-in ignored test (counts only, nothing copied)
- [ ] CI green

## Decisions & notes (append-only, dated)
- 2026-11-24: Sample inspected by headers only: one entry, encrypted flag set, deflate, no AES marker, so classic ZipCrypto. Entry name and sizes seen; contents not read.

## Progress log (append-only, dated)
- 2026-11-24: Task created; ADR-0014 written as proposed.

## Next step
Wait for the user to accept (or change) ADR-0014; then stage 1 (`mfa-formats` archive reader).
