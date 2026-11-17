# TASK-0008: UI — onboarding, unlock, code list, entry forms

- Status: open
- Created: 2026-11-17
- Updated: 2026-11-17
- Related ADRs: ADR-0002, ADR-0003, ADR-0006

## Goal
The React/shadcn UI for the MVP flows: create vault, unlock, code list with countdown and copy, search and tags, add/edit/delete, settings (auto-lock, clipboard clear). No business logic in components; feature folders; typed IPC client only.

## Acceptance criteria
- [ ] Create-vault and unlock screens; lock button; auto-lock reflected in UI
- [ ] Code list: live codes, countdown ring/bar, copy with clipboard auto-clear, search, tag filter, reorder
- [ ] Add/edit/delete forms generated from the type's parameter schema
- [ ] "Unsupported type" entries shown, kept intact
- [ ] Component tests (Vitest + Testing Library) for each screen; keyboard-accessible; works at phone and desktop widths
- [ ] CSP stays strict; no remote assets; lint/typecheck/test green

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: 0007. Design direction (layout, theme) to be agreed with the user via a quick mock before building.

## Progress log (append-only, dated)
- 2026-11-17: Task created.

## Next step
After TASK-0007: sketch the screens (list, add, unlock) as a mock for the user to approve, then build from the generated IPC client.
