# TASK-0017: Synchronise the vault between devices through a cloud folder

- Status: open (design only; nothing built)
- Created: 2026-11-26
- Updated: 2026-11-26
- Related ADRs: ADR-0011 (vault file format), ADR-0010 (self-hosted server, proposed); new ADR to be written

## Goal
(User request, 2026-11-26.) Devices keep the same vault in sync through a Google Drive or OneDrive link that points to a password-protected folder. The user can change the link (move to another platform), change the folder password, and so on.

## Open questions (an ADR must answer them before code)
1. **A link alone cannot be read or written by an app.** A Drive/OneDrive share link is a web page; reading and writing needs the provider's API with a sign-in (OAuth) and an app registration (client id, scopes such as Drive `drive.file`/`appdata` or Graph `Files.ReadWrite.AppFolder`). Google Drive has no password-protected links at all; OneDrive has them only in some plans and the password is checked by the web page, not by an API. So "link + password" needs a different mechanism, e.g.:
   - a. OAuth sign-in per provider (app registered by the project; tokens kept inside the vault),
   - b. WebDAV or similar with URL + user + password (Nextcloud, many NAS; provider-neutral, matches "change link / change password" best),
   - c. a plain folder that a sync client (Drive/OneDrive desktop app, Syncthing) keeps in sync; the app only reads/writes a file path (works on desktop; on Android needs the folder picker).
2. **What is synced.** The already encrypted vault file only (ADR-0011): the provider sees ciphertext; the folder password is then a second, transport-level secret, not the vault's protection. Keys and the master password never leave the device.
3. **Conflicts.** Two devices change the vault offline: whole-file last-writer-wins loses entries; entry-level merge needs per-entry ids, timestamps and deletion markers, probably a format change (new ADR superseding parts of ADR-0011).
4. **Credentials.** Where sync credentials live (inside the encrypted vault), how they are changed or removed, and what a lost device means (revoking the token or password).
5. **Scope.** Desktop and Android only; background sync vs. on unlock/on change; offline behaviour; what the UI shows when the remote is unreachable or was changed by someone else.

## Acceptance criteria
- [ ] ADR proposed with options a–c compared and a recommendation; user decision recorded
- [ ] (after the decision) criteria for implementation, tests with a fake remote, no secrets in logs

## Decisions & notes (append-only, dated)
- 2026-11-26: Task created from the user's idea; no design decision taken.

## Progress log (append-only, dated)
- 2026-11-26: Task created.

## Next step
Write the ADR (options a–c, recommendation) and ask the user to choose; do not start code before.
