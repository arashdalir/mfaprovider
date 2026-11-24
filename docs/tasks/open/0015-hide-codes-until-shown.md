# TASK-0015: Hide one-time codes until the user presses Show

- Status: in-progress
- Created: 2026-11-24
- Updated: 2026-11-24
- Related ADRs: ADR-0003 (security model)

## Goal
Codes are not drawn on screen by default. Each account has a Show button; a revealed code hides itself again after a short time. Copying works without showing. A setting lets the user turn the behaviour off. Hidden is the default.

## Acceptance criteria
- [x] Default: every code is masked (`••• •••`), the countdown bar stays
- [x] Show / Hide button per account; revealed code auto-hides after 30 s
- [x] Copy still works while hidden and never reveals the code
- [x] Setting "Hide codes until I press Show" (default on), validated like the other settings
- [ ] Locking the vault leaves nothing revealed (by design the list unmounts on lock, so reveal state is lost; not covered by a test)
- [x] Tests for each of the above
- [ ] CI green

## Decisions & notes (append-only, dated)
- 2026-11-24: Requested by the user as the safer variant. Decisions by Claude: reveal is per account, not global; 30 s fixed auto-hide (no setting yet); copy-while-hidden allowed (the clipboard is cleared by the existing timer); a setting exists so the choice stays the user's. **Honest limit:** this is a UI measure against shoulder-surfing and over-the-shoulder screenshots. The Rust side still sends the current codes to the webview on every refresh, so it does not protect against malware inside the app's own process. A stronger variant (core returns a code only for accounts the user revealed, with a re-auth option) is possible later; not done here.

## Progress log (append-only, dated)
- 2026-11-24: Task created.

- 2026-11-24: Implemented in EntryRow (per-row reveal with 30 s timer), EntryList, settings (`hideCodes`, default true, validated) and the settings screen. UI tests 149 pass locally (`--pool=vmThreads`), tsc clean, oxlint shows only the existing button.tsx warning. Existing list tests now set `hideCodes: false` explicitly; 4 new list tests and 3 settings tests cover masking, per-account reveal, copy-while-hidden, auto-hide, default and round trip.

## Next step
Push, watch CI, then close: the user to look at it in the real app (Show button layout, 30 s auto-hide).
