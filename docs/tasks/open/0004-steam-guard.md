# TASK-0004: Steam Guard OTP type

- Status: in-progress
- Created: 2026-11-17
- Updated: 2026-11-18
- Related ADRs: ADR-0006, ADR-0007

## Goal
Add Steam Guard (5-character alphabet code derived from TOTP/HMAC-SHA1) as a built-in `OtpType`, registered through the same registry as TOTP/HOTP.

## Acceptance criteria
- [x] `steam` type registered via `OtpRegistry`, 5-char output over Steam's alphabet, 30 s period
- [x] Verified against known-answer fixtures (documented source of each vector in the test) — see the caveat in the notes
- [x] Parameter schema rejects unsupported digits/period/algorithm (schema is empty; every parameter is rejected)
- [ ] CI green — green locally (31 tests, fmt, clippy); CI run pending the push

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: 0003. Steam has no formal RFC; fixtures must come from an independent implementation, recorded in the test file.
- 2026-11-18: Algorithm (from the community-documented Steam Guard scheme): HMAC-SHA1 over `time / 30`, RFC 4226 dynamic truncation to a 31-bit integer, then 5 characters of the alphabet `23456789BCDFGHJKMNPQRTVWXY`, taking `value % 26` and dividing by 26 each step (least-significant first). Algorithm, length and period are fixed → empty parameter schema.
- 2026-11-18: **Vector provenance caveat.** I could not fetch official Steam vectors. The 10 test vectors are derived independently of the Rust code from RFC 4226 Appendix D's published truncated integers (their last six digits equal the RFC's HOTP codes, which cross-checks them) converted with Steam's base-26 rule by a separate script. That validates truncation, the alphabet mapping and the period, but not Steam's exact real-world conventions (e.g. character order); a fixture from a real Steam account/another implementation should be added when available — until then, treat real-account verification as outstanding.
- 2026-11-18: Refactor: extracted `truncated_value` from `hotp_code` in `otp/hotp.rs`; HOTP/TOTP behaviour unchanged (all earlier RFC tests still pass).

## Progress log (append-only, dated)
- 2026-11-17: Task created.
- 2026-11-18: Implemented `otp/steam.rs`, registered `steam` as a built-in, 6 new tests; 31 tests green locally, `fmt --check` and `clippy -D warnings` clean.

## Next step
Push, confirm CI green. Then, if the user can supply a real Steam shared secret + a code observed at a known time (or another implementation's output), add it as a fixture and close the caveat; otherwise close the task with the caveat recorded and move to TASK-0003's closure and TASK-0005.
