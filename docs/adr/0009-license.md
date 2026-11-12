# ADR-0009: Open-source licence

- Status: accepted (2026-11-12; user: "AGPL based is okay", software must be open-source and free-to-use)
- Date: 2026-11-12
- Related tasks: TASK-0001

## Context
A secrets app earns trust through auditability. Targets today: Windows, Linux, Android. Possible later: macOS, iOS (App Store terms conflict with GPL-family licences).

## Options considered
1. GPL-3.0-or-later — strong copyleft; no closed forks.
2. **AGPL-3.0-or-later** — GPL plus network copyleft; closes the "hosted modified fork" loophole, which matters if a sync/relay service is ever added (ADR-0008).
3. Apache-2.0 / MIT — permissive; easiest App Store path; allows closed forks.

## Decision
AGPL-3.0-or-later. Contributor sign-off (DCO) from the start, so relicensing/dual-licensing stays possible if iOS becomes a goal. Dependency licences are checked in CI (`cargo deny`, JS licence checker) for AGPL compatibility.

## Consequences
+ Forks and hosted derivatives stay open; free to use.
- iOS distribution would need relicensing or dual-licensing of own code.
- Some corporate users avoid AGPL dependencies.
