# ADR-0001: Record decisions as ADRs and tasks as files

- Status: accepted
- Date: 2026-11-12
- Related tasks: TASK-0001

## Context
Sessions can restart; context must survive in the repo.

## Decision
ADRs in `docs/adr/`, tasks in `docs/tasks/{open,done}/`, resume protocol in `CLAUDE.md`.

## Consequences
Small overhead per task; resumability and traceability.
