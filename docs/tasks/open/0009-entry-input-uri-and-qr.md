# TASK-0009: Adding entries — otpauth URI, QR image, manual entry

- Status: in-progress
- Created: 2026-11-17
- Updated: 2026-11-24
- Related ADRs: ADR-0004, ADR-0006

## Goal
Parse `otpauth://` URIs (TOTP/HOTP/Steam variants) into entries, decode QR codes from an image file, and support pasted URI and manual entry. Camera QR scanning is Android-only and belongs to TASK-0012.

## Acceptance criteria
- [ ] `otpauth://` parser in `mfa-formats` with fixtures for edge cases (missing issuer, encoded labels, counter, algorithm/digits/period)
- [ ] QR decoding from PNG/JPEG in Rust (seed never touches JS); decoder failures are user-readable, never echo the secret
- [ ] Paste-URI and manual-entry flows in the UI using the same core validation
- [ ] Malformed or hostile input (huge images, invalid base32) rejected safely, covered by tests
- [ ] CI green

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: 0007 (and 0008 for UI wiring). The `mfa-formats` crate is created here and shared with 0010/0011.

- 2026-11-24: Plan: (1) `NewEntry::validate` in core so parser and add share validation; (2) `mfa-formats` crate with the otpauth parser, fixtures first; (3) ADR for QR image decoding (crate choice + hostile-input limits) for user review before adding dependencies; (4) shell commands + UI paste/image flows. Parser conventions chosen by Claude: duplicate query parameters rejected; `+` in values means space; `issuer` parameter wins over label prefix; `otpauth://steam/` is Steam (a `totp` URI with issuer Steam stays TOTP); unknown parameters ignored; URI limited to 2048 bytes.

## Progress log (append-only, dated)
- 2026-11-17: Task created.

- 2026-11-24: Done: `NewEntry::validate` (core), `mfa-formats` crate with `parse_otpauth` (24 tests: Google example, missing/encoded/unicode labels, issuer precedence, algorithm/digits/period, HOTP counter incl. u64 max, Steam, case-insensitivity, lenient secrets, unsupported type, bad params via core validation, duplicates, malformed/oversized input, multibyte scheme check, errors never contain the secret). Clippy, workspace tests, cargo-deny green locally.

## Next step
Write ADR-0013 (QR image decoding: crate choice for PNG/JPEG decode + QR detection, dependency footprint via cargo-deny, hostile-input limits: file size, pixel count, decode time) as proposed, for the user to review before any image dependency is added. Then QR decoding, then shell commands + UI (paste URI, image file, manual entry).
