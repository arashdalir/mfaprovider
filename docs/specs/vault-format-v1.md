# Vault file format, version 1

Normative layout for the encrypted vault file (ADR-0011, ADR-0003, ADR-0004). Status: **proposed** until ADR-0011 is accepted. All integers are big-endian. A file is one header followed by one encrypted body.

## Container

| Offset | Size | Field | Value / meaning |
|-------:|-----:|-------|-----------------|
| 0 | 8 | `magic` | ASCII `MFAVAULT` |
| 8 | 2 | `version` | `1` |
| 10 | 1 | `kdf_id` | `1` = Argon2id, version 0x13 |
| 11 | 1 | `aead_id` | `1` = XChaCha20-Poly1305 |
| 12 | 4 | `kdf_memory_kib` | Argon2 memory cost in KiB |
| 16 | 4 | `kdf_iterations` | Argon2 time cost |
| 20 | 1 | `kdf_lanes` | Argon2 parallelism |
| 21 | 16 | `salt` | random, per password slot |
| 37 | 24 | `wrap_nonce` | random |
| 61 | 48 | `wrapped_vk` | AEAD(vault key, 32 B) = 32 B ciphertext + 16 B tag |
| 109 | 24 | `body_nonce` | random, fresh on every save |
| 133 | 4 | `body_len` | length of `body` in bytes |
| 137 | `body_len` | `body` | AEAD(padded payload) = ciphertext + 16 B tag |

Header = bytes `0..137`. `body_len` must equal the remaining file length exactly; files over 64 MiB are rejected before parsing.

## Keys
- **Vault key (VK)**: 32 random bytes, generated once at vault creation, never derived from the password.
- **Key-encryption key (KEK)** = Argon2id(`password`, `salt`, memory, iterations, lanes) → 32 bytes. `password` is the UTF-8 bytes of the user's password after Unicode **NFKC** normalisation.
- **Password slot** = bytes `12..109` (`kdf_id` … `wrapped_vk`). Changing the password replaces only the slot (new salt, new nonce, re-wrapped VK); the body is untouched.

## Encryption
- `wrapped_vk` = XChaCha20-Poly1305(KEK, `wrap_nonce`, VK) with **AAD = bytes `0..61`** — authenticates version, algorithm ids, KDF parameters, salt and nonce, so tampering with or downgrading them makes unwrapping fail.
- `body` = XChaCha20-Poly1305(VK, `body_nonce`, padded payload) with **AAD = bytes `0..12`** (`magic`, `version`, `kdf_id`, `aead_id`).
- Padding: payload, then byte `0x80`, then zero bytes up to the next multiple of 1024. Hides the exact entry count; decryption strips from the last `0x80`.
- No compression.

## KDF parameter bounds
Writers: `memory ≥ 19456 KiB`, `iterations ≥ 2`, `lanes ≥ 1` (OWASP minimums), tuned per device to about one second. Readers treat the header as untrusted and reject `memory > 1048576 KiB (1 GiB)`, `iterations > 20`, `lanes > 16`, and anything below the writer minimums.

## Payload (CBOR, RFC 8949; text keys)
```
{ "v": 1,
  "created_at": <unix ms>, "updated_at": <unix ms>,
  "entries":    [ Entry, ... ],
  "tombstones": [ { "id": <16 B>, "deleted_at": <unix ms> }, ... ] }

Entry = { "id": <16 B UUID>, "otp_type": text, "issuer": text, "account": text,
          "secret": <bytes>, "params": { text: text }, "icon": text (optional),
          "tags": [text], "notes": text,
          "created_at": <unix ms>, "updated_at": <unix ms> }
```
- Entry order in the list is the display order (see ADR-0011 open point on reordering and sync).
- **Forward compatibility:** a reader preserves unknown map keys on the payload and on entries and writes them back unchanged. An entry whose `otp_type` has no implementation is kept intact (ADR-0004).
- A payload `"v"` higher than the reader supports → refuse to open; never rewrite it.

## Versioning and failure modes
- `version` higher than supported → `UnsupportedVersion`; the file is never modified.
- Older versions are read, then upgraded on the next explicit save (the storage layer keeps a backup).
- Wrong password and a corrupted/tampered header are indistinguishable by design (`UnwrapFailed`). A body that fails authentication after a successful unwrap is `BodyCorrupted`.

## Golden vector
Added with the implementation: fixed password, salt, nonces and payload → fixed file bytes, committed under `core/mfa-core/tests/`.
