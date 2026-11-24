# TASK-0008: UI — onboarding, unlock, code list, entry forms

- Status: in-progress
- Created: 2026-11-17
- Updated: 2026-11-24
- Related ADRs: ADR-0002, ADR-0003, ADR-0006

## Goal
The React/shadcn UI for the MVP flows: create vault, unlock, code list with countdown and copy, search and tags, add/edit/delete, settings (auto-lock, clipboard clear). No business logic in components; feature folders; typed IPC client only.

## Acceptance criteria
- [x] Create-vault and unlock screens; lock button; auto-lock reflected in UI
- [x] Code list: live codes, countdown ring/bar, copy with clipboard auto-clear, search, tag filter, reorder
- [ ] Add/edit/delete forms generated from the type's parameter schema
- [x] "Unsupported type" entries shown, kept intact
- [ ] Component tests (Vitest + Testing Library) for each screen; keyboard-accessible; works at phone and desktop widths
- [ ] CSP stays strict; no remote assets; lint/typecheck/test green

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: 0007. Design direction (layout, theme) to be agreed with the user via a quick mock before building.

- 2026-11-24: User decided: code list as **compact rows** (tap row to copy); theme = **follow system plus manual light/dark/system switch** in settings. UI-level minimum master password length is Claude's default (8), not a user decision — revisit with the security review.

## Progress log (append-only, dated)
- 2026-11-17: Task created.

- 2026-11-24: Started. Asking the user to pick the code-list layout and theme before building (per the 2026-11-17 note).

- 2026-11-24: Step 1 done: injectable `VaultApi` context (tests use fakes, no module mocking), theme (system + manual switch, storage failures tolerated), `SessionGate` (loading → create | unlock → open; polls status each second so auto-lock shows; failed polls do not eject an open vault), create/unlock screens, error-message mapping. UI tests: 34. Testing Library cleanup added to test setup (was missing).

- 2026-11-24: Step 2 done: compact-row code list (`features/entries`): live codes refreshed each second, readable grouping, countdown bar (progressbar role), copy with 30 s clipboard auto-clear (restarts on new copy; clears even if unreadable, leaves a different clipboard value alone), search + tag filter (client-side over display metadata), up/down reorder (disabled while filtered), HOTP "Next code", unsupported/broken entries shown intact, activity reporter (click/key, max 1 per 5 s; code polling is not activity). UI tests: 63.
- 2026-11-24: Found a gap: forms "generated from the parameter schema" need the schema over IPC, which TASK-0007 did not expose. Adding a `list_otp_types` command (ids + param specs; integer bounds saturated to 2^53-1 for TypeScript).

## Next step
Step 3: `list_otp_types` command (shell + client + drift lists), then add/edit/delete forms generated from it, settings (auto-lock, clipboard clear), then a real smoke-run with `pnpm tauri dev`.
