# TASK-0010: Importers — Google migration, Aegis, 2FAS

- Status: open
- Created: 2026-11-17
- Updated: 2026-11-17
- Related ADRs: ADR-0004, ADR-0006

## Goal
Importers behind one `Importer` interface in `mfa-formats` for Google Authenticator `otpauth-migration://`, Aegis (plain + encrypted) and 2FAS, with the preview-then-merge flow: new / duplicate / conflicting entries, per-entry skip, replace or keep-both. Duplicate = same issuer + account + secret.

## Acceptance criteria
- [ ] `Importer` interface; one isolated codec per source
- [ ] Fixtures per format (including encrypted Aegis) with provenance noted; no real seeds in the repo
- [ ] Merge planner returns a preview without mutating the vault; applying it is a separate step
- [ ] UI import wizard: pick file/URI → preview → apply
- [ ] Corrupt/unknown-version files fail cleanly; imports never log secrets
- [ ] CI green

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: 0007, 0009. Later importers (andOTP, Bitwarden, WinAuth) are post-MVP (ADR-0006).

## Progress log (append-only, dated)
- 2026-11-17: Task created.

## Next step
After TASK-0009: define the `Importer` trait and merge-planner types, then implement the Google migration decoder (protobuf) test-first.
