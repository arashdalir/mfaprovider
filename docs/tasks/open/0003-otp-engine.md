# TASK-0003: OTP engine — OtpType trait, registry, HOTP and TOTP

- Status: open
- Created: 2026-11-17
- Updated: 2026-11-17
- Related ADRs: ADR-0004, ADR-0006, ADR-0007

## Goal
In `mfa-core`, define the `OtpType` interface (id, parameter schema, `generate(secret, params, input) -> Code`, input = time or counter) and a registry mapping `otp_type` ids to implementations. Implement HOTP (RFC 4226) and TOTP (RFC 6238) through it. No I/O: clock is passed in.

## Acceptance criteria
- [ ] `OtpType` trait + `OtpRegistry` with lookup by id; unknown ids return a distinct "unsupported" result (never an error that drops the entry)
- [ ] HOTP passes all RFC 4226 test vectors
- [ ] TOTP passes RFC 6238 vectors for SHA-1/256/512
- [ ] Parameters validated by schema: algorithm, digits (6–8), period, counter
- [ ] Secrets held in zeroizing types; no `Debug`/`Display` that prints a secret
- [ ] `fmt`, `clippy -D warnings`, tests green in CI

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: none (first MVP task). Blocks 0004, 0007. "COTP" = HOTP everywhere (ADR-0006).

## Progress log (append-only, dated)
- 2026-11-17: Task created.

## Next step
Write failing tests from the RFC 4226 Appendix D vectors, then implement HOTP in `core/mfa-core`.
