# TASK-0008: UI — onboarding, unlock, code list, entry forms

- Status: done
- Created: 2026-11-17
- Updated: 2026-11-24
- Related ADRs: ADR-0002, ADR-0003, ADR-0006

## Goal
The React/shadcn UI for the MVP flows: create vault, unlock, code list with countdown and copy, search and tags, add/edit/delete, settings (auto-lock, clipboard clear). No business logic in components; feature folders; typed IPC client only.

## Acceptance criteria
- [x] Create-vault and unlock screens; lock button; auto-lock reflected in UI
- [x] Code list: live codes, countdown ring/bar, copy with clipboard auto-clear, search, tag filter, reorder
- [x] Add/edit/delete forms generated from the type's parameter schema
- [x] "Unsupported type" entries shown, kept intact
- [x] Component tests (Vitest + Testing Library) for each screen, querying by role and label
- [ ] Verified by eye: keyboard-only use and phone + desktop widths (not yet checked in a real window)
- [ ] CSP stays strict; no remote assets; lint/typecheck/test green (lint, typecheck, tests green; runtime CSP not observed)

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: 0007. Design direction (layout, theme) to be agreed with the user via a quick mock before building.

- 2026-11-24: User decided: code list as **compact rows** (tap row to copy); theme = **follow system plus manual light/dark/system switch** in settings. UI-level minimum master password length is Claude's default (8), not a user decision — revisit with the security review.

## Progress log (append-only, dated)
- 2026-11-17: Task created.

- 2026-11-24: Started. Asking the user to pick the code-list layout and theme before building (per the 2026-11-17 note).

- 2026-11-24: Step 1 done: injectable `VaultApi` context (tests use fakes, no module mocking), theme (system + manual switch, storage failures tolerated), `SessionGate` (loading → create | unlock → open; polls status each second so auto-lock shows; failed polls do not eject an open vault), create/unlock screens, error-message mapping. UI tests: 34. Testing Library cleanup added to test setup (was missing).

- 2026-11-24: Step 2 done: compact-row code list (`features/entries`): live codes refreshed each second, readable grouping, countdown bar (progressbar role), copy with 30 s clipboard auto-clear (restarts on new copy; clears even if unreadable, leaves a different clipboard value alone), search + tag filter (client-side over display metadata), up/down reorder (disabled while filtered), HOTP "Next code", unsupported/broken entries shown intact, activity reporter (click/key, max 1 per 5 s; code polling is not activity). UI tests: 63.
- 2026-11-24: Found a gap: forms "generated from the parameter schema" need the schema over IPC, which TASK-0007 did not expose. Adding a `list_otp_types` command (ids + param specs; integer bounds saturated to 2^53-1 for TypeScript).

- 2026-11-24: Step 3 done: shell `list_otp_types` command (+ `OtpRegistry::types`, `EntryView.params`; all registration lists and the drift tests updated), add/edit/delete form (`features/entries/EntryForm`) generated from the schema: edit sends only changed fields (a test caught that default-filled params would otherwise be rewritten), unsupported types editable for metadata only and never get params sent, delete needs inline confirmation, `noValidate` so our own messages show consistently. UI tests: 91, shell: 52, core: 148.

- 2026-11-24: Step 4 done: reveal-secret flow in the edit form (password re-entry, hides after 30 s or on request, dropped on unmount; Enter in its box cannot submit the enclosing form), settings screen (idle lock 1/5/15/30 min or never, clipboard clear 10-120 s, theme) persisted in localStorage with validation; the UI re-applies the idle time to the core after each unlock via new `set_idle_lock` command (core forgets it on restart; zero is refused). Shell tests 55, UI 111.

- 2026-11-24: Smoke-run (`pnpm tauri dev`, Vite moved to port 5174 by a command-line override because 5173 is used by another project on this machine; no repo change): the real app launched, window "MFA Provider" opened and stayed responsive, setup hook created the app data dir, idle-ticker thread ran without a panic. **Not verified:** any click-through of the real UI (no way to drive the window from here), phone-width layout, and CSP behaviour at runtime. Built page checked statically: no inline script/style, no remote assets (only same-origin JS/CSS), no `dangerouslySetInnerHTML`/`eval`.

- 2026-11-24: CI green on 6a76559. **Closed by the user's decision with two items unchecked**: (1) keyboard-only and phone/desktop-width use were never checked in a real window; (2) the "CSP stays strict" line was not ticked — statically the built page has no inline script/style and no remote assets, but runtime CSP behaviour was not observed. Neither the click-through of the real UI (create, add, copy, edit, reveal, lock/unlock) was done by anyone yet. Any defect found later becomes its own task.

## Next step
None. Follow-ups: first manual run of the real app by the user; TASK-0009 adds the paste-URI/QR flows to these screens.
