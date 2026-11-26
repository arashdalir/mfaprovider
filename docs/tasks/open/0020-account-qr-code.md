# TASK-0020: QR code of an account in its details

- Status: in-progress
- Created: 2026-11-26
- Updated: 2026-11-26
- Related ADRs: ADR-0002 (seeds stay out of JS except the explicit reveal), ADR-0013

## Goal
(User request, 2026-11-26.) In an account's details, after "Show secret key…" (master password asked again), a QR code of that account is shown next to the key so another device's camera can scan it. It hides together with the key (30 s, Hide, leaving the screen).

## Acceptance criteria
- [x] Core builds the `otpauth://` link for an entry (all types and parameters the registry knows; Steam handled or refused clearly) with round-trip test through `parse_otpauth`
- [x] The QR image is drawn in Rust (no JS QR library) and returned only by the existing reveal command, as an SVG string; the link itself never reaches JS
- [x] UI shows it in the reveal panel with an accessible label, hidden with the key
- [x] Tests; no-secrets tripwire still green (reveal stays the only seed-bearing response)
- [ ] Checked on the phone: scanned by another device

## Decisions & notes (append-only, dated)
- 2026-11-26: Claude's decision: render the QR in Rust (`qrcode` crate, already a dev-dependency of `mfa-formats`, moved to a normal dependency) so the otpauth link, which contains the seed, stays out of JavaScript; only the picture crosses IPC, like the base32 text does today.

## Progress log (append-only, dated)
- 2026-11-26: Task created.

- 2026-11-26: Done in code. `mfa-formats::otpauth::to_otpauth(&Entry) -> Zeroizing<String>` (the inverse of `parse_otpauth`: percent-encoded label, `issuer`, stored params, `counter=0` for counter accounts; `totp`, `hotp`, `steam`; other types give `NoLinkForm`) with 4 test groups including round trips of Google's example, SHA-512/8 digits/60 s, HOTP, Steam and non-ASCII names. `mfa-formats::qr_encode::qr_svg(text)` (crate `qrcode` with the `svg` feature moved from dev to normal dependencies; black on white with quiet zone, at least 256 px). `RevealedSecret` gained `qr_svg: Option<String>` (the only response that carries seed material still is the reveal; the link itself never crosses IPC; a test checks the picture contains neither the key nor "otpauth"). UI: `RevealSecret` shows the picture (data-URL `<img>`, allowed by the existing CSP `img-src data:`), hidden with the key. Tests: formats 114, shell 105, UI 161; clippy and `cargo deny` clean. **Not verified:** that another phone's camera actually scans it (the QR matrix comes from the `qrcode` crate and is not decoded back in a test); how it looks on the phone.

## Next step
Rebuild and install on the phone; the user scans the code with another device.
