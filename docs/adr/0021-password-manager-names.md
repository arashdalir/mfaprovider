# ADR-0021: How the app's secrets appear to password managers

- Status: proposed
- Date: 2026-11-27
- Related tasks: TASK-0017
- Refines: ADR-0018

## Context
(User request, 2026-11-27.) Password managers should offer to store the device password, the sync password and the sync secret, each under its own name, so that none overwrites another. A manager files a login under a site (or app) and a user name. All screens of this app are one web origin (`tauri.localhost` or the platform's equivalent), so **separate domains are not possible**; the user name is what can differ.

## Options considered
1. Password boxes without user names. A manager then has one nameless login for the app and may replace one secret with another.
2. **A user name for each secret, shown on the form, and one form per secret.**
3. Hidden user name fields. Less clutter, but managers may skip hidden fields and the user cannot see what to look for.

## Decision
Option 2.

| Secret | User name | Same on every device |
|---|---|---|
| Device password | `mfa-provider device password (<kind>-<code>)`, for example `(android-3f9a)` | No |
| ZIP import password | `mfa-provider ZIP file password` | — |
| Sync password (with the sync screens) | `mfa-provider sync password` | Yes |
| Sync secret (with the sync screens) | `mfa-provider sync secret` | Yes |

- The device password differs per device and a password manager may itself sync between devices, so its name carries the kind of device and four random characters, kept in the app's local storage. Without storage the code is left out.
- **One form holds one secret.** Two different secrets in one form look like "old and new password" of one login. Where a screen needs two (joining; a sync change that also asks for the device password), they are separate forms, in steps. A repeat box belongs to the same secret and shares its form.
- Autocomplete hints: `new-password` when a secret is chosen or first saved, `current-password` when it is asked for. The ZIP password has `off`.
- The sync secret is typed into a password box of its own. Where the app shows it, it also offers a form that holds it, for saving.
- The fields of an account (including its secret key and an `otpauth` link) have autocomplete off, so that autofill is not asked to remember them.
- The two sync names are fixed. After a new master key, the new password and secret replace the old entries; the old ones open only the old file.

## Consequences
+ The secrets can be stored and told apart. Names must not change once released.
- Under one origin a manager lists all of these for the app; the user picks by name.
- Storing the sync secret and the sync password in the same manager means one break-in there yields both. With the file, that opens it. The secret still protects against someone who gets only the file. The user guide says so.
- If the manager's second factor lives in this app, keeping the only copy of these secrets there can lock the user out. The recovery sheet and the manager's recovery codes stay necessary.
- These are hints. **A prompt to save is not guaranteed**, and a manager may ignore `off`.
- On Windows the webview (WebView2) does not save passwords unless an app enables it, which this one does not, and browser extensions do not run in it. There the names help only when copying by hand.
- **Not yet tried:** with a password manager on Android — whether it offers to save and fill, what it shows as names, and whether its own unlock screen sends the app to the background and so locks an unlocked vault (ADR-0017 covers only the file picker).
- **Not yet checked:** other apps built the same way can report the same origin. A manager that matches on the origin alone, not the app, might offer these logins there.
- Reinstalling the app, or clearing its data, gives a new device code, so a new entry.
