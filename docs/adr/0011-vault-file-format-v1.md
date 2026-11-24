# ADR-0011: Vault file format v1

- Status: proposed (2026-11-24) — awaiting user review
- Date: 2026-11-24
- Related tasks: TASK-0005 (also constrains TASK-0006, TASK-0007, TASK-0011)
- Refines: ADR-0003 (crypto choices) and ADR-0004 (what is stored). Does not change them.

## Context
ADR-0003 fixed the primitives (random vault key wrapped by an Argon2id-derived key, XChaCha20-Poly1305) and ADR-0004 fixed what an entry holds. Neither fixes the bytes on disk. A file format is expensive to change once real vaults exist, so it is specified and reviewed before code. Normative layout: [docs/specs/vault-format-v1.md](../specs/vault-format-v1.md).

## Decision
1. **Layout**: fixed 137-byte header, then one encrypted body. The header carries a magic, a format version, algorithm ids, the Argon2id parameters, a salt and the wrapped vault key.
2. **One password slot**: the password unlocks the vault key; the vault key encrypts the body. Changing the password rewrites only the slot, never the entries (needed by TASK-0006). Biometric wraps stay device-local and outside the file (ADR-0003). No recovery key (ADR-0003, option declined).
3. **Tamper protection**: the slot is authenticated over the version, algorithm ids and KDF parameters, so a tampered or downgraded header cannot unlock the vault. The body is bound to the magic, version and algorithm ids.
4. **Password normalisation**: Unicode NFKC before hashing, so the same password typed on Windows, Linux and Android keyboards yields the same key.
5. **Payload**: CBOR (RFC 8949) with text keys, padded to 1 KiB multiples to hide the exact entry count; no compression. Secrets are raw bytes, not text.
6. **Forward compatibility**: unknown fields are preserved on read and write; unknown `otp_type` entries are kept intact (ADR-0004); a newer payload or file version is refused and never rewritten.
7. **Untrusted headers**: KDF parameters have floors (OWASP minimums: 19 MiB, 2 iterations, 1 lane) and ceilings (1 GiB, 20 iterations, 16 lanes), so a hostile file cannot make the app allocate unbounded memory.
8. **Injection**: randomness (salt, nonces, vault key) comes from an injected RNG trait, so the core stays I/O-free and tests can use fixed values for a golden vector.
9. **Reuse**: the encrypted export file (TASK-0011) uses the same container with a different magic (`MFAEXPRT`) and its own independent password.

## Options considered
- **Payload as JSON** instead of CBOR: easier to eyeball, but secrets need text encoding and are harder to keep out of long-lived strings. Rejected.
- **Rewrite the whole file on password change** (header bound to body): simpler AAD, but re-encrypts every entry and makes TASK-0006's cheap re-wrap impossible. Rejected.
- **Compress the payload**: pointless at this size, and compression before encryption can leak information. Rejected.

## Open point (decide in TASK-0007, not blocking)
Reordering: entry order is the list order in v1. Sync (ADR-0008) merges per entry by id and would not preserve order; adding an explicit sort key later is a payload-schema change (`"v": 2`) that the forward-compatibility rules already allow.

## Consequences
+ Cheap password change, tamper-evident parameters, room for new algorithms (`aead_id`/`kdf_id`) and new payload fields without breaking old files.
- A fixed 137-byte header means any change to its layout is a new `version`.
- NFKC normalisation is permanent for v1: it can never change without breaking existing passwords.
- Forgotten password = unrecoverable vault (ADR-0003).
