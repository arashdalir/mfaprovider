# ADR-0017: The trip to a phone's file picker does not lock the vault

- Status: proposed
- Date: 2026-11-27
- Related tasks: TASK-0012
- Refines: ADR-0003 (lock when the app goes to the background)

## Context
ADR-0003 locks the vault when the app goes to the background. On Android the system file picker is a separate screen, so choosing an import file or a QR image counts as going to the background: the vault locks, and the import cannot be finished (seen on the user's phone). The camera scan, which stays inside the app, is not affected.

## Options considered
1. **Keep locking.** Safe, but importing from a file is unusable on the phone.
2. **Keep the vault open after the lock** by remembering the chosen file and resuming after unlocking. The file picker's result is lost when the screen is replaced by the unlock screen, and holding a secret-bearing file across a lock is worse than the exemption.
3. **Excuse that one trip, narrowly.** The UI announces the picker as the user opens it; the shell ignores the next background event.
4. **Switch background locking off while an import screen is open.** Simple, but far wider than needed.

## Decision
Option 3, enforced in the shell (`ipc::picker_excuse`), not in the UI. The excuse
- is granted only while unlocked, and only on mobile (a desktop file dialog does not take the app off the screen);
- covers one departure, and only a `Backgrounded` event, never `Suspended`;
- lasts at most 2 minutes, measured on both the monotonic and the wall clock; staying away longer locks, whether or not the app ever comes back;
- ends as soon as the app is back on the screen.

The idle lock is unchanged.

## Consequences
- For up to 2 minutes after opening the picker, leaving the app from the picker (for example to the home screen) does not lock it. `FLAG_SECURE` still hides its content from the recent-apps view.
- If the user closes the picker without the app ever leaving the screen, the unused excuse could cover a departure within those 2 minutes.
- While the app is suspended the timer cannot fire; the check then happens on return.
- Any later screen that takes the app off the display (share sheet, save-file dialog for the encrypted export) has to decide explicitly whether to use the same excuse.
