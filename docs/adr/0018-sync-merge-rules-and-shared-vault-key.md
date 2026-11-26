# ADR-0018: Sync merge rules, and how devices come to share one vault key

- Status: proposed
- Date: 2026-11-27
- Related tasks: TASK-0017
- Refines: ADR-0008 (sync through a user-selected file), ADR-0011 (file format)

## Context
ADR-0008 fixes: the sync unit is the encrypted vault file, one password on all devices, per-entry last-writer-wins on `updated_at` with tombstones, merged inside the core. It leaves open the tie and edge cases, and how one device can read another's file: every vault gets its own random vault key (VK), and an unlocked session holds the VK, not the password.

## Options considered (reading the sync file)
1. **Keep the password in memory** while unlocked and use it on the sync file. Widens what a memory dump reveals; biometric unlock never has the password. Rejected.
2. **Ask for the password at each sync.** Safe, but sync stops being automatic.
3. **All synced devices share one VK.** A device joins once, with the password; afterwards it reads the sync file with the VK it already holds.

## Decision
**Shared key (option 3).**
- The first device turns sync on by writing its vault file to the chosen place.
- A device that *joins* opens that file with its password, merges its own accounts into it, and from then on uses the file's VK and password slot. Joining therefore changes that device's VK and, if its password differed, its password; the UI says so before it happens. Biometric unlock is switched off on that device and offered again.
- A password change rewrites only the slot (ADR-0011). Another device **never takes over a slot by itself**: opening the file with the VK does not authenticate the slot, so whoever can write the cloud file could replace it and lock the user out. A device that sees a different slot asks for the new password, and adopts the slot only if that password opens it.
- A sync file that the VK cannot open is never overwritten.

**Merge rules** (`mfa-core::vault::merge`), for each entry id:
- The newest of all versions and deletion markers wins. A deletion wins a tie with an edit.
- Two different edits with the same time are decided by their content, so every device picks the same one.
- A counter-based account keeps the **highest** counter of the two versions while its secret is unchanged, so a counter never goes back.
- An edit made after a deletion brings the account back.
- Deletion markers are kept for good for now (nothing calls compaction), so a device that was away for a long time cannot bring deleted accounts back.
- **Order is not synced** (ADR-0011's open point): each device keeps its own order, and accounts new to it go to the end, oldest first. A difference in order alone never causes a write.
- Unknown fields are kept (ADR-0011); where both sides have one, the side saved last wins.
- On joining only, an account identical in type, issuer, name, secret and settings to one in the sync file is dropped rather than doubled. Later, two accounts added separately stay two accounts.

## Consequences
+ Sync works after a biometric unlock and needs no stored password.
- One VK for all of a user's devices: whoever obtains it from one device can open every copy. A password change does not change the VK (ADR-0011), so removing a lost device's access needs a key rotation, which does not exist yet.
- Someone who can write the cloud file can delete it or feed back an old copy. An old copy cannot undo newer entries or deletions on a device that already has them; it can mislead a device that is only now joining.
- Last-writer-wins depends on the devices' clocks (ADR-0008).
- Manual reordering stays per device until a sort key is added to the payload.
