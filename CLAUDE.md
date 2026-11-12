# mfa-provider — working agreement

Cross-platform authenticator (TOTP/HOTP) app. Security first.

## Session resume protocol (read this first after any restart)
1. Read every file in `docs/tasks/open/` — each is the source of truth for its task (status, decisions, next step).
2. Read `docs/adr/README.md` for the ADR index; only open ADRs relevant to the task.
3. Continue from the "Next step" in the task file. Update the task file *before and after* each meaningful step.

## Rules
- Every task gets a file in `docs/tasks/open/NNNN-slug.md` (template: `docs/tasks/TEMPLATE.md`) at creation, before any work starts. Move to `done/` when finished.
- Every non-trivial decision becomes an ADR (`docs/adr/TEMPLATE.md`). Status: proposed → accepted | rejected | superseded. Never edit an accepted ADR's decision; supersede it.
- Clean code: small functions, intention-revealing names, no dead code, no comments that restate code, dependencies point inward (domain knows nothing of UI/storage), tests with every behavior.
- Never log, print, or persist secrets/TOTP seeds in plaintext outside the encrypted vault (except explicit, user-confirmed plaintext export).
