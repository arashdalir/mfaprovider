# ADR-0019: Sync storage providers, their settings, and reads that may be old

- Status: proposed
- Date: 2026-11-27
- Related tasks: TASK-0017
- Refines: ADR-0008 (phases behind a `StorageProvider` trait), ADR-0018. Would change one local field of the [sync file format](../specs/sync-format-v1.md); the shared file itself is unchanged.

## Context
The sync file, its keys and the merge do not depend on where the file is stored. Drive and OneDrive must not be the only places: WebDAV and other services should fit without touching them. The storage differs in what it can promise. A first trial with OneDrive on Android (TASK-0017) found that picking a file, keeping access across a restart and writing all worked, but **the first read after a restart returned the old content**. The accepted format stores a device's sync location as a single text, which cannot hold an address with sign-in details.

## Options considered
1. One built-in integration per service. Every service then repeats the sync logic.
2. **One small interface over an opaque file, with each provider declaring what it can guarantee.**
3. Support only "a file the user chooses". Nothing to sign in to, but on Android it depends on how each provider's app behaves.

## Decision
Option 2.

**Interface** (crate `mfa-sync`; the provider sees only bytes):
- `read` → bytes and an opaque **version**, or "missing";
- `replace(bytes, expected_version)` → writes only over that version, else "conflict";
- `create(bytes)` → writes only if nothing is there;
- `guarantees` → for conditional writes and for atomic writes: **enforced** or **best effort**.

| Provider | Phase | Version | Conditional write | Atomic write |
|---|---|---|---|---|
| Local file | 1 | hash of the content | best effort (check, then rename) | yes (temporary file and rename) |
| Android document (any app offering writable files) | 1 | hash of the content | best effort | not guaranteed |
| WebDAV | 2 | ETag (`If-Match`, `If-None-Match`) | enforced only if the server honours it | server dependent |
| A service's own API | 3 | the service's revision | per service | per service |

**Correctness must not depend on the guarantees.**
- A read may be old. Merging old data is harmless; writing afterwards can replace a newer version. A content hash does not detect this.
- Each device keeps its own changes and deletion markers, so a replaced change returns at that device's next sync. This heals as long as the device syncs again.
- To shorten the window, a device reads again before writing where the provider can refresh, and checks again at the next opportunity after writing.
- **The password lock is the exception:** a device writing from an old copy puts the old lock back. After changing the sync password, the device remembers the new lock's hash and, until the file has been seen with it on a later sync, warns if the old lock returns.
- A half-written file (possible without atomic writes) fails authentication. It is retried later and only replaced on the user's request (ADR-0018). The UI must make that repair easy to find.

**Settings per device.** The local field `sync.location: text` becomes
`sync.provider: { "kind": text, "settings": { text: text } }`, for example `file` {`path`}, `android-document` {`uri`}, `webdav` {`url`, `user`, `password`}. No released version writes this field. The settings, including any password or token, live only in the encrypted vault: never in the sync file, an export or a log.

**Network.** Providers that use the network need HTTPS with normal certificate checks, ask for app passwords or scoped tokens where the service has them, and their code runs only after the user turns such a provider on (ADR-0008).

**Moving** to another provider writes the same sync file to the new place and points each device there; keys and passwords stay. The old file must then be removed, or a device left behind keeps syncing to it unnoticed.

**Tests.** One conformance suite (conflict, missing file, create over an existing file, interrupted write, old read) runs against every provider and a fake that can misbehave.

## Consequences
+ A new service is one provider and the same tests.
+ Anything with a synced folder works in phase 1 on desktop.
- The weakest case (old reads, no atomic write) is the phone's, so the logic has to be built for it.
- Between an overwrite and the losing device's next sync, other devices do not see that change.
- WebDAV and service APIs bring stored sign-in details and network code, which the security review must cover.
- Android behaviour is known from one short trial with one app. Creating a new document from the phone is unconfirmed.
