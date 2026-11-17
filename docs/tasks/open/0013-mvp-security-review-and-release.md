# TASK-0013: MVP security review, packaging and release

- Status: open
- Created: 2026-11-17
- Updated: 2026-11-17
- Related ADRs: ADR-0002, ADR-0003, ADR-0009

## Goal
Before the first release: threat-model review against the accepted ADRs, dependency and secret-handling audit, installers for Windows and Linux plus a signed Android build, and a release process.

## Acceptance criteria
- [ ] Written threat model (assets, attackers, mitigations) in `docs/`; gaps become tasks
- [ ] Review confirms: seeds never in JS, no secrets in logs/IPC/crash output, strict CSP and minimal capabilities, devtools off in release
- [ ] `cargo deny` and `pnpm audit` clean or each exception justified
- [ ] Windows and Linux installers built in CI; Android release build signed (keystore handled outside the repo)
- [ ] CI actions pinned to commit SHAs; SBOM or lockfile provenance noted
- [ ] README/CHANGELOG, tagged release, licence notices (AGPL source offer) verified

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: all of 0003–0012. Code signing for Windows is a cost/decision for the user; raise it when this task starts.

## Progress log (append-only, dated)
- 2026-11-17: Task created.

## Next step
Starts after the feature tasks. First action then: draft the threat model from ADR-0002/0003 and review it with the user.
