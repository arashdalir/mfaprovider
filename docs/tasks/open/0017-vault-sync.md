# TASK-0017: Synchronise the vault between devices

- Status: in-progress (phase 1)
- Created: 2026-11-26
- Updated: 2026-11-27
- Related ADRs: ADR-0008 (accepted: sync via a user-selected cloud file, phased), ADR-0018 (accepted: master key, sync secret, merge rules); spec `docs/specs/sync-format-v1.md` (accepted); guide `docs/sync.md`, ADR-0011 (vault file format), ADR-0003, ADR-0010 (proposed)

## Goal
(User request, 2026-11-26.) An account added on one device shows up on the others. The user also asked for a link-based setup (Google Drive or OneDrive link to a password-protected folder) where the link and password can be changed later.

## What ADR-0008 already decides
- Sync unit: the encrypted vault file; same master password on every device; biometric wraps stay device-local.
- Merge per entry by `id`, last-writer-wins on `updated_at`, tombstones for deletes; merge on decrypted data inside the core, then re-encrypt.
- Phases behind a `StorageProvider` trait in `mfa-sync`: (1) a file or folder the user picks (Google Drive/OneDrive/Syncthing clients do the transport; on Android the Storage Access Framework exposes cloud providers), (2) WebDAV, (3) direct Google Drive/OneDrive APIs (OAuth).
- Conflicted copies made by cloud clients are detected and merged; atomic writes with a header version/etag; opt-in.

## Where the user's idea does not fit as worded
A share *link* to a password-protected folder is a web page, not something an app can read or write: Google Drive has no password-protected links, and OneDrive's are checked by the web page, not an API. The password-protected-folder idea maps to phase 2 (WebDAV: URL + user + password, changeable in settings) or phase 3 (OAuth sign-in per provider). Phase 1 (pick a synced folder) needs neither.

## Open questions for the user
1. Start with phase 1 (pick a folder kept in sync by the Drive/OneDrive app; works on desktop and, through Android's file picker, on the phone)? It is the smallest way to get "add on one device, see it on the others".
2. Are WebDAV (phase 2) and direct provider APIs (phase 3) wanted later, and in which order?

## Acceptance criteria
- [x] Decision on the phase to build first (user: phase 1, a synced file)
- [ ] Task broken down for that phase (merge in core with tombstones first; the vault model already has tombstones, see TASK-0007/ADR-0004)
- [ ] Implementation with tests using a fake provider; no secrets in logs

## Decisions & notes (append-only, dated)
- 2026-11-26: Task created from the user's idea. First version of this file wrongly treated the design as open and promised a new ADR; ADR-0008 (accepted 2026-11-12) already covers it, so this file now points there. No new ADR needed unless the user wants to change ADR-0008.
- 2026-11-26: **Purpose stated by the user:** an account added on one device must show up on the others.

## Progress log (append-only, dated)
- 2026-11-26: Task created and corrected.

- 2026-11-27: The user said to start phase 1 once CI was green (it was, on 9ad9c48). ADR-0018 written (proposed).

## Breakdown of phase 1
1. [x] Core: pure merge of two `VaultData` with tests (ADR-0018 rules). Does not depend on the key decision.
2. [x] Core: sync file container (spec, with golden vector), sync secret text form, typed `sync` field in the local vault (never copied into the sync file or an export, with tests), turn on / join / sync cycle / change sync password / new master key. (ADR-0018 and the spec accepted 2026-11-27.)
3. [ ] `mfa-sync`: `StorageProvider` trait (read with version, write only over that version), file implementation, fake for tests.
4. [ ] Shell and UI: turn on / join (scan or type the secret) / turn off, recovery sheet, choose the file, status and warnings, change sync password, new master key, replace a damaged file, conflicted copies. Keep `docs/sync.md` true to what is built.
5. [ ] Android: Storage Access Framework plugin (pick or create a document, keep the permission); the picker excuse of ADR-0017 applies. **Biggest unknown:** whether the Drive and OneDrive apps give a lasting, writable document, and their writes are not atomic. Try this on the user's phone early.
6. [ ] Tried across the PC and the phone; results recorded here.

- 2026-11-27: Step 1 done: `mfa_core::vault::merge` (`merge(local, remote) -> Merged {data, local_changed, remote_changed}`), changes detected by a SHA-256 digest of each entry (also the tie-break), 13 tests (mfa-core 181), clippy clean. Nothing calls it yet.
- 2026-11-27: TASK-0012's Android CI job passed on its first run (9ad9c48).

- 2026-11-27: Review discussion. The user chose: two passwords (device and sync), a separate master key instead of one shared vault key, and a second secret; asked for thorough GitHub documentation. ADR-0018 rewritten (still proposed), format spec drafted, guide `docs/sync.md` written. The merge code is unaffected.

- 2026-11-27: The user accepted ADR-0018 and the format spec, and would prefer OneDrive.

- 2026-11-27: Checked over adb on the Pixel: OneDrive 7.64.2 (`com.microsoft.skydrive`) is installed and registers a documents provider (`content.StorageAccessProvider`). This says nothing yet about whether a picked file stays writable.

- 2026-11-27: OneDrive trial built at the user's request: a temporary debug-only activity "Sync trial" in `app/src-tauri/gen/android/app/src/debug/` (**deliberately uncommitted; delete the folder afterwards**), writing only to files whose name contains `sync-trial`. Dummy file `mfa-sync-trial.txt` created in the PC's OneDrive folder. Four questions: pick the file; keep access after a restart; write and have it upload; see a change made on the PC.

- 2026-11-27: **OneDrive trial results** (Pixel 10a, OneDrive 7.64.2; one run):
  1. Picking `mfa-sync-trial.txt` in OneDrive: works (user).
  2. Access after a phone restart: works; the screen showed `kept permission: read=true write=true` (user).
  3. Writing with mode `wt`: `read-back MATCHES`; the line reached the PC, before and after the restart (checked on the PC; first one about 1 s after the phone's timestamp, by the file's modification time; second 2 s). No conflict copy seen.
  4. Seeing a PC change: **the first read after the restart returned the old content**, the second the new (user). Probably a cached copy refreshed in the background; not proven. The phone's next write kept the PC's line.
  Not tried: creating a new document from the app, `ContentResolver.refresh`, offline behaviour, writing while stale, interrupted writes, large delays. The phone log was lost in the restart.
  **For the design:** one read must not be taken as current. A stale read is harmless to merge, but a write after it can replace a newer cloud version, and a content-hash version check would not notice; devices re-merge their own changes later (a password-slot change is the exception).

- 2026-11-27: Create trial: the user created `mfa-sync-trial-created.txt` from the phone. **Unconfirmed:** the file was not found anywhere in the PC's OneDrive folder; where it was saved is not known (phone log lost, adb unreachable). Test code (`src/debug`) deleted; the installed app on the phone still has the trial screen until the next install.
- 2026-11-27: The user went to sleep and asked for the next steps to be planned and carried out, to be checked tomorrow. Plan, in order:
  1. Write ADR-0019 (storage providers, location settings, stale reads), **proposed only**; no accepted document is changed.
  2. Step 2 in `mfa-core`, test first, each part committed on its own: (a) sync secret and its text/QR form; (b) sync file container per the accepted spec, with golden vector; (c) typed `sync` field in the local vault, never copied by a merge or into a sync file; (d) session operations: turn on, join, sync cycle, change sync password, new master key, repair. The location stays the accepted spec's `location: text` until ADR-0019 is accepted.
  3. Update `docs/sync.md` and this file, push, check CI.
  Not planned for tonight: shell, UI, Android file access, anything on the phone.

- 2026-11-27: **Step 2 done** (core only, nothing in the app calls it). `vault/sync_secret.rs`, `vault/sync_file.rs`, `vault/sync_state.rs`, `vault/session/sync_ops.rs`. Operations: `prepare_new_sync`/`commit_new_sync` (turn on, new master key), `prepare_join`/`preview_join`/`commit_join`, `sync`, `change_sync_password`, `replacement_sync_file`, `record_upload`, `disable_sync`, `reveal_sync_secret`, `sync_status`. They take and return bytes; file access is step 3. Checked locally: core 225 + 114 + 8 + 4 tests, shell 147, UI 191, fmt, clippy, typecheck. **CI not run (not pushed).**
  Points the user should know:
  - The golden vector was produced by this code: it pins the format and does not prove the cryptography; of it only HKDF is checked against an independent vector (RFC 5869). To be covered by the security review (TASK-0013).
  - Only showing the sync secret asks for the device password in the core (as ADR-0018 says). Whether a password change, new master key, repair or turning sync off should ask too is for the shell step.
  - A replacement file cannot verify the typed password, so the UI must ask for it twice. It must be offered only for a damaged or missing file.
  - An unknown field beside `revision` and `vault` in a sync payload is refused (it could not be written back).
  - Duplicates dropped on joining leave no deletion marker. If such an id had been shared through an earlier sync group and another device brings it back, the account would appear twice.
  - ADR-0018's "no export holds the sync settings" has no test yet because no export exists (TASK-0011 must add it). The sync payload is tested.
  - The warning for a returning old password lock belongs to ADR-0019 and is not built. The location is the accepted spec's single text.

- 2026-11-27: The user **accepted ADR-0019** and agreed with the morning report’s suggestions: the app asks for the device password before changing the sync password, making a new master key, repairing the file or turning sync off; repair asks for the sync password twice and is offered only for a damaged or missing file. New request: password fields must be defined so password managers offer to save them, with the sync secret and the two passwords named apart so they never overwrite each other.
  Plan: (1) record this as ADRs; (2) core: require the device password for those operations, test first; (3) UI: name the existing password fields for password managers, keep other secret fields out of them; (4) docs. The sync screens do not exist yet; they get the same rules in step 4.

## Next step
The user reviews ADR-0019 and pushes (`git push` needs a GitHub sign-in), then CI must be checked. After that: step 3 (`mfa-sync` storage provider; its shape depends on ADR-0019).
