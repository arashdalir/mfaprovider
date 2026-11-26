# TASK-0017: Synchronise the vault between devices

- Status: open (not started; design is ADR-0008)
- Created: 2026-11-26
- Updated: 2026-11-26
- Related ADRs: ADR-0008 (accepted: sync via a user-selected cloud file, phased), ADR-0011 (vault file format), ADR-0003, ADR-0010 (proposed)

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
- [ ] Decision on the phase to build first (user)
- [ ] Task broken down for that phase (merge in core with tombstones first; the vault model already has tombstones, see TASK-0007/ADR-0004)
- [ ] Implementation with tests using a fake provider; no secrets in logs

## Decisions & notes (append-only, dated)
- 2026-11-26: Task created from the user's idea. First version of this file wrongly treated the design as open and promised a new ADR; ADR-0008 (accepted 2026-11-12) already covers it, so this file now points there. No new ADR needed unless the user wants to change ADR-0008.
- 2026-11-26: **Purpose stated by the user:** an account added on one device must show up on the others.

## Progress log (append-only, dated)
- 2026-11-26: Task created and corrected.

## Next step
Ask the user which phase to build first (question 1 above); then break the task down.
