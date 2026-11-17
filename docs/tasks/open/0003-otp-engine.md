# TASK-0003: OTP engine — OtpType trait, registry, HOTP and TOTP

- Status: in-progress
- Created: 2026-11-17
- Updated: 2026-11-17
- Related ADRs: ADR-0004, ADR-0006, ADR-0007

## Goal
In `mfa-core`, define the `OtpType` interface (id, parameter schema, `generate(secret, params, input) -> Code`, input = time or counter) and a registry mapping `otp_type` ids to implementations. Implement HOTP (RFC 4226) and TOTP (RFC 6238) through it. No I/O: clock is passed in.

## Acceptance criteria
- [x] `OtpType` trait + `OtpRegistry` with lookup by id; unknown ids return a distinct "unsupported" result (never an error that drops the entry)
- [x] HOTP passes all RFC 4226 test vectors
- [x] TOTP passes RFC 6238 vectors for SHA-1/256/512
- [x] Parameters validated by schema: algorithm, digits (6–8), period, counter
- [x] Secrets held in zeroizing types; no `Debug`/`Display` that prints a secret
- [ ] `fmt`, `clippy -D warnings`, tests green in CI — green locally (25 tests, `cargo deny` ok); CI run pending the next push

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: none (first MVP task). Blocks 0004, 0007. "COTP" = HOTP everywhere (ADR-0006).

- 2026-11-17: Design: `Secret` (zeroizing, redacted Debug, no Clone), `Params` (string map per ADR-0004) validated against a static `ParamSpec` schema, `OtpInput::{Time, Counter}`, `OtpType` trait, `OtpRegistry::get(id) -> Option<&dyn OtpType>` (None = unsupported; the caller keeps the entry). HOTP schema: algorithm, digits, counter (stored value; caller passes it as the input). TOTP schema: algorithm, digits, period. Crates: RustCrypto `hmac`, `sha1`, `sha2`, `zeroize`, `thiserror`.

## Progress log (append-only, dated)
- 2026-11-17: Task created.
- 2026-11-17: Started implementation in `core/mfa-core`.
- 2026-11-17: Implemented `secret`, `otp::{algorithm, params, otp_type, hotp, totp, registry}`. 25 unit tests green locally, including all RFC 4226 Appendix D and RFC 6238 vectors (SHA-1/256/512) and an HMAC RFC 2202 check; `cargo fmt --check`, `clippy -D warnings`, `cargo deny` pass.
- 2026-11-17: Notes for later tasks: `Resolved::integer/choice` panic only on a programmer error (name not in the schema); `Hotp` takes the stored counter as `OtpInput::Counter` from the caller; seconds-remaining for the UI countdown is left to TASK-0007; `DuplicateTypeId` prevents extensions from shadowing built-ins.

## Next step
Push, confirm CI is green on the Linux and Windows jobs, then tick the last criterion and move this task to `done/`. Next task: TASK-0004 (Steam Guard), which reuses `Shared`/`hotp_code` plumbing.
