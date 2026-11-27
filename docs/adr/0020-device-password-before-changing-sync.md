# ADR-0020: The device password again before changing sync, and what may be repaired

- Status: accepted (user, 2026-11-27)
- Date: 2026-11-27
- Related tasks: TASK-0017
- Refines: ADR-0018 (which asks for the device password only to show the sync secret). Changes none of its decisions.

## Context
An unlocked but unattended app could otherwise change the sync password, move sync to new keys, overwrite the sync file or turn sync off. A repair also writes a file under whatever sync password is typed, because a broken file offers nothing to check it against.

## Options considered
1. Ask only for showing the sync secret.
2. **Also ask before every operation that changes the keys or the file, or ends sync.**
3. Ask before every sync operation, including joining and syncing. Rejected: syncing runs in the background, and joining already needs the sync password and the secret.

## Decision
Option 2.

- The core asks for the device password for: changing the sync password, creating a new master key, repairing the sync file, turning sync off and showing the sync secret. Not for turning sync on, joining or syncing. A fingerprint does not stand in for it.
- The core replaces a sync file only if it is missing or broken. It refuses a file that still opens, that belongs to another master key, or that a newer version wrote.
- The UI asks for the sync password **twice** for a repair, offers repair only when sync reports a missing or damaged file, and applies it only to the configured location.

## Consequences
+ The rules hold even if a screen forgets them, except asking twice.
- One more password prompt for rare actions.
- A file that is not a sync file at all counts as broken, so the UI must never repair a place the user did not set up for sync.
- A damaged identifier in the header looks like another master key: repair refuses until the user removes the file.
