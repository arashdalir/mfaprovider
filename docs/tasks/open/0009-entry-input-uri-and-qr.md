# TASK-0009: Adding entries — otpauth URI, QR image, manual entry

- Status: in-progress
- Created: 2026-11-17
- Updated: 2026-11-24
- Related ADRs: ADR-0004, ADR-0006

## Goal
Parse `otpauth://` URIs (TOTP/HOTP/Steam variants) into entries, decode QR codes from an image file, and support pasted URI and manual entry. Camera QR scanning is Android-only and belongs to TASK-0012.

## Acceptance criteria
- [x] `otpauth://` parser in `mfa-formats` with fixtures for edge cases (missing issuer, encoded labels, counter, algorithm/digits/period)
- [x] QR decoding from PNG/JPEG in Rust (seed never touches JS); decoder failures are user-readable, never echo the secret
- [x] Paste-URI and manual-entry flows in the UI using the same core validation
- [x] Malformed or hostile input (huge images, invalid base32) rejected safely, covered by tests
- [ ] CI green

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: 0007 (and 0008 for UI wiring). The `mfa-formats` crate is created here and shared with 0010/0011.

- 2026-11-24: Plan: (1) `NewEntry::validate` in core so parser and add share validation; (2) `mfa-formats` crate with the otpauth parser, fixtures first; (3) ADR for QR image decoding (crate choice + hostile-input limits) for user review before adding dependencies; (4) shell commands + UI paste/image flows. Parser conventions chosen by Claude: duplicate query parameters rejected; `+` in values means space; `issuer` parameter wins over label prefix; `otpauth://steam/` is Steam (a `totp` URI with issuer Steam stays TOTP); unknown parameters ignored; URI limited to 2048 bytes.

## Progress log (append-only, dated)
- 2026-11-17: Task created.

- 2026-11-24: Done: `NewEntry::validate` (core), `mfa-formats` crate with `parse_otpauth` (24 tests: Google example, missing/encoded/unicode labels, issuer precedence, algorithm/digits/period, HOTP counter incl. u64 max, Steam, case-insensitivity, lenient secrets, unsupported type, bad params via core validation, duplicates, malformed/oversized input, multibyte scheme check, errors never contain the secret). Clippy, workspace tests, cargo-deny green locally.

- 2026-11-24: Wrote ADR-0013 (proposed): `rqrr` + `png` + `zune-jpeg` used directly (not `image`, not `rxing`, not a JS decoder), PNG/JPEG only, limits 10 MiB / 8192 px per side / 25 Mpx / 128 MiB PNG memory / 16 codes. Waiting for user review.

- 2026-11-24: User accepted ADR-0013 as written. Implementing `decode_qr_image` in mfa-formats.

- 2026-11-24: Done: `decode_qr_image` (`mfa-formats/src/qr.rs`, 17 tests): PNG (grey, grey+alpha, RGB, RGBA with transparency flattened onto white, palette, 16-bit) and JPEG read; several codes per image; hostile input: empty/foreign/random bytes, bare magic numbers, truncation at nine points, byte-flip fuzz (no panics), PNG headers claiming 100000x100000 / 8193 wide / 36 Mpx refused before any pixel buffer exists, zero-size PNG, JPEG claiming 60000x60000, over-10-MiB file; a real otpauth QR goes straight into `parse_otpauth`. zune-jpeg reports its own size limit as an untyped error, so its limits are lifted to the format maximum and our check decides (nothing is allocated before it). Deps added: rqrr (no default features), png, zune-jpeg; dev: qrcode, jpeg-encoder, crc32fast. cargo-deny green. Workspace: core 149, formats 41, storage 12.
- 2026-11-24: Design for the UI flows (seed must not enter JS): the shell keeps parsed links/QR codes as in-memory **drafts** (replaced by each new draft, dropped on lock/idle-lock); the UI only gets a `DraftView` (type, issuer, account, params — no secret), lets the user edit those, and `add_draft` stores it. Image bytes travel to Rust as a raw IPC body (not JSON); the webview never sees decoded text.

- 2026-11-24: Shell + UI done. Shell: `DraftView`/`DraftEditsRequest`, draft slot in `VaultService` (replaced by each new draft, dropped on lock, idle lock and tick, kept after a rejected edit or a failed image), commands `draft_from_link`, `draft_from_image` (base64, because raw IPC bodies do not exist on Android), `add_draft`, `discard_drafts`; registration lists + drift tests updated; 71 shell tests incl. QR PNGs generated in tests, several codes per image, non-link codes ignored, secret absent from draft JSON in base32/hex. UI: "Add account" chooser (manual / paste link / QR image), draft review form without any secret field (the secret never reaches JS), picker when an image holds several codes, 10 MiB pre-check; 124 UI tests. Core 149, formats 41.
- 2026-11-24: Honest limits: the QR/link flows were not clicked through in the real app (no way to drive the window); JPEG QR decoding tested only with images from one encoder (jpeg-encoder); photos over about 25 megapixels are refused by design (ADR-0013); Google `otpauth-migration://` codes are not handled here (TASK-0010) and show as "not an otpauth:// link".

## Next step
Wait for CI; then close. Camera QR scanning is TASK-0012.
