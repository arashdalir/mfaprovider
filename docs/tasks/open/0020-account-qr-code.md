# TASK-0020: QR code of an account in its details

- Status: open
- Created: 2026-11-26
- Updated: 2026-11-26
- Related ADRs: ADR-0002 (seeds stay out of JS except the explicit reveal), ADR-0013

## Goal
(User request, 2026-11-26.) In an account's details, after "Show secret key…" (master password asked again), a QR code of that account is shown next to the key so another device's camera can scan it. It hides together with the key (30 s, Hide, leaving the screen).

## Acceptance criteria
- [ ] Core builds the `otpauth://` link for an entry (all types and parameters the registry knows; Steam handled or refused clearly) with round-trip test through `parse_otpauth`
- [ ] The QR image is drawn in Rust (no JS QR library) and returned only by the existing reveal command, as an SVG string; the link itself never reaches JS
- [ ] UI shows it in the reveal panel with an accessible label, hidden with the key
- [ ] Tests; no-secrets tripwire still green (reveal stays the only seed-bearing response)
- [ ] Checked on the phone: scanned by another device

## Decisions & notes (append-only, dated)
- 2026-11-26: Claude's decision: render the QR in Rust (`qrcode` crate, already a dev-dependency of `mfa-formats`, moved to a normal dependency) so the otpauth link, which contains the seed, stays out of JavaScript; only the picture crosses IPC, like the base32 text does today.

## Progress log (append-only, dated)
- 2026-11-26: Task created.

## Next step
Find how an entry's parameters map back to otpauth query parameters; write `otpauth::to_uri` with tests.
