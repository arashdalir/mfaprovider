# TASK-0018: Countdown ring instead of a bar; the code is the copy button

- Status: done
- Created: 2026-11-26
- Updated: 2026-11-26
- Related ADRs: none (UI-only; see TASK-0015 for hide/show)

## Goal
(User request, 2026-11-26, after trying the Android build.) In each entry row the time left shows as a ring that empties (seconds in the middle) instead of a bar, and the code itself is the copy button, also while masked, so the separate copy icon goes away.

## Acceptance criteria
- [x] Ring with remaining seconds in its centre; turns to the warning colour in the last 5 s; keeps the progressbar role, label and values for assistive technology
- [x] Masked code is the copy button (same accessible name "Copy code for …"); the copy icon is gone
- [x] Counter-based entries keep "Next code"
- [ ] Tests updated/added; UI tests green; checked on the phone

## Decisions & notes (append-only, dated)
- 2026-11-26: Copying while the code is masked stays allowed (as before with the icon); pressing the masked code copies without revealing it.

## Progress log (append-only, dated)
- 2026-11-26: Task created.

- 2026-11-26: Done in `EntryRow.tsx`: `CountdownRing` (SVG ring, seconds in the centre, warning colour at 5 s or less, same progressbar role and values), one code button for masked and shown states, copy icon removed. UI tests 151 (3 new, 1 changed). APK rebuilt and installed on the phone; not yet looked at by the user. Local prettier is not installed, so formatting was not machine-checked.

- 2026-11-26: Closed on the user's instruction.

## Next step
None.
