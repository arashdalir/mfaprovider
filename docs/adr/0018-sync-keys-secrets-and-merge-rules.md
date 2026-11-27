# ADR-0018: Sync keys, the sync secret, and merge rules

- Status: accepted
- Date: 2026-11-27 (rewritten the same day after review; the first draft gave all devices one vault key)
- Related tasks: TASK-0017
- Refines: ADR-0008 (sync through a user-selected file) and **replaces its rule "same password on all devices"**; ADR-0003 (crypto), ADR-0011 (file format)
- Details: [format spec](../specs/sync-format-v1.md), [guide](../sync.md)

## Context
ADR-0008 fixes that sync goes through an encrypted file the user places in a cloud folder, merged per entry inside the core. It does not say how a second device reads that file. Each local vault has its own random vault key (VK), and an unlocked session holds that key, not the password. The file also sits where others may get a copy of it, and a copy can be attacked offline for as long as the attacker likes.

## Options considered
**How a device reads the sync file**
1. Keep the password in memory while unlocked. Widens what a memory dump reveals; biometric unlock never has the password. Rejected.
2. Type a password at every sync. Safe, but sync stops being automatic.
3. All synced devices share one VK (first draft). Joining would change a device's VK and password and switch biometrics off, and a device would have to take over password slots from the cloud file.
4. **A separate master key (MK) for the sync file**, kept inside each joined device's own vault.

**What protects a stolen copy of the file**
- (a) The sync password alone. Its strength is then the only defence against offline guessing.
- (b) **The sync password and a random sync secret** that never appears in the file in readable form.
- (c) A key file in place of a password. One factor only.

## Decision
Options 4 and (b).

**Keys**
- Local vaults do not change: own VK, own device password, own biometric unlock.
- A random 256-bit **MK** encrypts the body of the sync file.
- A random 128-bit **sync secret** is created when sync is turned on.
- The file header holds the MK encrypted under a key derived from **both** the sync password (Argon2id) and the sync secret (HKDF). Neither alone opens it. The secret is not stored in the file in any form.
- A joined device keeps MK and sync secret **inside its encrypted local vault**. They are never part of the synced data or of any export (this is an invariant with tests).
- A random **sync id** names one MK. A new MK means a new id.
- The whole header is authenticated by the body's encryption, so only a holder of the MK can produce a file whose header is accepted.

**Passwords**
- The **device password** unlocks the vault on one device. The **sync password** is needed to join a device. They are changed separately and may differ per device. This replaces ADR-0008's same-password rule.
- The sync password needs at least 12 characters (a device password needs 8). The UI recommends a long passphrase used nowhere else; it does not forbid reusing the device password.

**Joining and recovery**
- A new device needs the file, the sync secret (QR code from a joined device, or typed from the recovery sheet) and the sync password.
- Showing the secret again needs the device password, like revealing an account's secret.
- When sync is turned on, the app asks the user to save a **recovery sheet** with the secret. It never contains a password. If every joined device is lost, the file can be opened only with the sheet and the sync password.

**Two ways to react**
- **Change sync password:** the same MK and secret are locked with the new password. Other joined devices carry on.
- **Create a new master key:** new MK, new secret, new sync id, written to a new location. Every other device joins again.

**Safety rules**
- The app creates a sync file only when the user turns sync on, starts a new master key or asks to replace a damaged file. A missing file is reported, never recreated silently.
- A file the device cannot authenticate, or with another sync id, is never overwritten without that explicit request.
- A write replaces only the version that was read; if the file changed meanwhile, read and merge again.
- The body carries a revision number. Each device remembers the highest it has seen and warns when the file's is lower. It still merges, which is safe.

**Merge rules** (`mfa-core::vault::merge`), for each entry id:
- The newest of all versions and deletion markers wins. A deletion wins a tie with an edit.
- Two different edits with the same time are decided by their content, so every device picks the same one.
- A counter-based account keeps the highest counter of the two versions while its secret is unchanged.
- An edit made after a deletion brings the account back.
- Deletion markers are kept for good for now, so a long-absent device cannot bring deleted accounts back.
- Order is not synced (ADR-0011's open point): each device keeps its own, new accounts go to the end, oldest first. Order alone never causes a write.
- Unknown fields are kept; where both sides have one, the side saved last wins.
- On joining only, a local account identical in type, issuer, name, secret and settings to one in the file is dropped rather than doubled.

## Consequences
+ A thief with only the cloud file cannot get in by guessing passwords: the 128-bit secret is also required.
+ Joining leaves the device's password and biometric unlock alone. Sync works after a biometric unlock with no stored password.
+ Tampering with the header cannot weaken or replace the lock unnoticed.
- Two things to look after (password and sync secret). Losing every device and the recovery sheet makes the sync file useless.
- Whoever opens a joined device's vault holds the MK and the secret. A new master key protects only what is written afterwards; copies already taken stay readable. If accounts may have been read, two-factor authentication must be set up again on those services.
- Changing the sync password does not shut out someone who has the secret, the old password and an old copy of the file (cloud services keep old versions): they can get the MK. That case needs a new master key.
- Someone who can write the file can delete it, damage it or restore an old copy. Damage stops sync until the user repairs it. A device that is only now joining cannot tell that a copy is old.
- Last-writer-wins depends on the devices' clocks (ADR-0008).
- A second file format to maintain.
