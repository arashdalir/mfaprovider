# ADR-0014: Password-protected ZIP import — the `zip` crate, minimal features, hard limits

- Status: accepted (2026-11-24, by the user, as written)
- Date: 2026-11-24
- Related tasks: TASK-0014 (also TASK-0010 importers)
- Refines: ADR-0004 (import sources), ADR-0002 ("seeds never enter JavaScript")

## Context
WinAuth can export a ZIP file protected by a password instead of a plain text file. The user's sample (`winauth-2026-07-10.zip`, 2156 bytes, not committed, never copied into the repo) was inspected by its headers only: one entry (`winauth-2026-07-10.txt`), flag bit 0 set (encrypted), compression method 8 (deflate), no AES marker (method 99). That is the classic **ZipCrypto** scheme, which is cryptographically weak (known-plaintext attacks). We do not choose it; we only read files that already exist. The archive and the password are untrusted/sensitive input: a hostile file must not exhaust memory or disk, and the password must not leak.

`zip` 9.8.2 (crates.io, updated 2026-11-23, MIT). With `default-features = false` its normal dependencies are `constant_time_eq`, `crc32fast`, `indexmap`, `memchr`, `typed-path`, `zeroize`, `libc`/`windows-sys`; ZipCrypto is always built in. `deflate-flate2` adds `flate2` (already in our tree through `png`). `aes-crypto` (AES-encrypted entries, as 7-Zip writes them) would add `aes`, `hmac`, `pbkdf2`, `sha1`; the default feature set also pulls bzip2, lzma, zstd, ppmd, which we do not need. `deny.toml` already allows MIT.

## Options considered
1. **`zip` with `default-features = false, features = ["deflate-flate2"]`.** ZipCrypto + deflate only. Maintained parser for the central directory, data descriptors and CRC. Smallest practical dependency addition.
2. **Same crate with `aes-crypto` too.** Would also open AES-encrypted ZIPs. More code in a component parsing untrusted files for a format nobody has shown us. Can be added later by a one-line change plus a test, if a real file needs it.
3. **Write our own ZipCrypto + central-directory parser, use `flate2` for deflate.** No new crate, but hand-rolled archive parsing of untrusted input is the riskier part, not the 60-line cipher.
4. **Ask the user to unzip first.** Rejected as the only path: the plaintext file would then lie on disk; reading it in memory is safer. Still always possible.

## Decision
Option 1, inside `mfa-formats` (no I/O: caller passes bytes and the password).

1. **Detection by content**: an archive is recognised by its first bytes (`PK\x03\x04`), never by file name.
2. **Limits, checked before reading content:** archive ≤ 5 MiB (same cap as text imports); at most 16 entries looked at; exactly one file entry is used (directories ignored; with zero or several files the import is refused with a readable message); declared uncompressed size ≤ 5 MiB **and** the read itself is capped at 5 MiB, so a lying header cannot expand further (zip-bomb guard).
3. **Nothing touches the disk and entry names are never used as paths** (no extraction, so no path traversal).
4. **Wrong password vs damaged file** are not distinguishable with ZipCrypto (1-byte check plus final CRC): one fixed sentence, "Wrong password, or the file is damaged." No attempt counter or hint beyond that.
5. **The extracted text goes through the existing text pipeline** (`decode_text` → link-list importer → merge planner → the same preview/apply). The ZIP layer knows nothing about OTP.
6. **Password handling:** typed in the UI, sent once with the file in the preview request, held in a zeroising buffer in Rust only for the call, never stored in app state, never logged, never part of any response. The UI clears its copy when the preview arrives or the screen closes. It is not the master password and is never compared with it.
7. **Transport:** the file is binary, so it is sent base64-encoded like images (ADR-0013 / Android IPC limits).
8. **After a successful import** the UI says that the file (and its password) are still on disk and that the user may want to delete them; we never delete user files.

## Consequences
- New dependency `zip` (minimal features) and `flate2` stays shared; `cargo deny`/audit must stay green.
- Tests use invented archives built in the test (a small ZipCrypto writer in dev code or a fixture generated once and described in its provenance note), including: right password, wrong password, no password given, two files, directory only, oversized declared size, truncated archive, a deflate stream that expands beyond the cap.
- AES-encrypted ZIPs and other containers (e.g. Aegis encrypted JSON) are separate decisions.
- ZipCrypto's weakness is not ours to fix, but the UI should not imply the archive was secure.
