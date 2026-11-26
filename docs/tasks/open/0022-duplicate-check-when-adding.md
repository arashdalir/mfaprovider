# TASK-0022: Warn when an added account is already in the vault

- Status: in-progress
- Created: 2026-11-26
- Updated: 2026-11-26
- Related ADRs: ADR-0004 (duplicate = same name + secret), TASK-0010 (merge planner reused)

## Goal
(User request, 2026-11-26.) Adding an account by hand, from a link, an image or the camera shows a warning when the vault already holds it. The warning does not forbid adding; the user decides.

## Acceptance criteria
- [x] Core/shell: a check that reuses `plan_merge` for one candidate and answers: same account (name and key), same name with another key, same key under another name, or nothing
- [x] Commands `check_duplicate(request)` and `check_draft_duplicate(id, edits)`; responses carry only the matching account's display name, never a secret
- [x] UI: drafts (link, image, camera) show the warning as soon as the review opens; every add checks again on submit and, if it matches, needs a second press ("Add anyway") before saving; editing a field clears the warning
- [x] Tests (shell: each kind + none + locked; UI: warning, second press, editing clears)
- [ ] Checked on the phone

## Decisions & notes (append-only, dated)
- 2026-11-26: It is a warning, not a block: same name with a different key is legitimate (two accounts at one service), and the user may want a deliberate copy.
- 2026-11-26: The check runs in the core (one definition of "same" for import and add).

## Progress log (append-only, dated)
- 2026-11-26: Task created.

- 2026-11-26: Done in code. Shell: `check_duplicate(request)` and `check_draft_duplicate(id, edits)` (registered in all four command lists), `DuplicateView {kind: same_account | same_name_other_key | same_key_other_name, matches: "Issuer · account"}`; they reuse `plan_merge`, so "same" means the same thing as in import (name trimmed and case-insensitive, key compared exactly); read-only, locked vault refuses. UI (`EntryForm`): a draft from a link, image or camera is checked when its review opens; manual adds are checked on Add; if it matches, a yellow notice names the kind and the existing account and the button becomes "Add anyway"; editing any field clears the notice and the next Add checks again; a failing check blocks the add and shows the error. Editing an existing account is not checked. Tests: shell 115 (+8, one module), UI 179 (+7). **Not verified:** on the phone; accounts with the same key inside a *single* import file are handled by the import preview, not here.

## Next step
Rebuild and install on the phone together with the next feature; user tries adding an account that already exists (e.g. scan the QR of an existing account's details).
