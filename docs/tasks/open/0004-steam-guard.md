# TASK-0004: Steam Guard OTP type

- Status: open
- Created: 2026-11-17
- Updated: 2026-11-17
- Related ADRs: ADR-0006, ADR-0007

## Goal
Add Steam Guard (5-character alphabet code derived from TOTP/HMAC-SHA1) as a built-in `OtpType`, registered through the same registry as TOTP/HOTP.

## Acceptance criteria
- [ ] `steam` type registered via `OtpRegistry`, 5-char output over Steam's alphabet, 30 s period
- [ ] Verified against known-answer fixtures (documented source of each vector in the test)
- [ ] Parameter schema rejects unsupported digits/period/algorithm
- [ ] CI green

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: 0003. Steam has no formal RFC; fixtures must come from an independent implementation, recorded in the test file.

## Progress log (append-only, dated)
- 2026-11-17: Task created.

## Next step
After TASK-0003 lands: collect known-answer vectors (secret, timestamp, expected code) from a trusted reference implementation and write them as tests first.
