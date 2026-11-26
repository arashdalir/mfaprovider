# TASK-0017: Synchronise the vault between devices

- Status: in-progress (phase 1)
- Created: 2026-11-26
- Updated: 2026-11-27
- Related ADRs: ADR-0008 (accepted: sync via a user-selected cloud file, phased), ADR-0018 (proposed: merge rules, shared vault key), ADR-0011 (vault file format), ADR-0003, ADR-0010 (proposed)

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
2. [ ] Core: join (adopt the file's key and slot), sync cycle (read, merge, write only if changed, detect a concurrent change), password-slot handling. **Needs ADR-0018 accepted.**
3. [ ] `mfa-sync`: `StorageProvider` trait (read with version, write only over that version), file implementation, fake for tests.
4. [ ] Shell and UI: turn on / join / turn off, choose the file, status, conflicted copies.
5. [ ] Android: Storage Access Framework plugin (pick or create a document, keep the permission); the picker excuse of ADR-0017 applies. **Biggest unknown:** whether the Drive and OneDrive apps give a lasting, writable document, and their writes are not atomic. Try this on the user's phone early.
6. [ ] Tried across the PC and the phone; results recorded here.

- 2026-11-27: Step 1 done: `mfa_core::vault::merge` (`merge(local, remote) -> Merged {data, local_changed, remote_changed}`), changes detected by a SHA-256 digest of each entry (also the tie-break), 13 tests (mfa-core 181), clippy clean. Nothing calls it yet.
- 2026-11-27: TASK-0012's Android CI job passed on its first run (9ad9c48).

## Next step
The user reviews ADR-0018 (step 2 depends on it). Independent of it: step 3 (`mfa-sync` storage provider) and the Android file-access trial of step 5.
