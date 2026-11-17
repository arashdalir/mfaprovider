# TASK-0009: Adding entries — otpauth URI, QR image, manual entry

- Status: open
- Created: 2026-11-17
- Updated: 2026-11-17
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

## Progress log (append-only, dated)
- 2026-11-17: Task created.

## Next step
Create the `mfa-formats` crate and write otpauth URI parsing tests first.
