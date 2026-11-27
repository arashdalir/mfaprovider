# Sync file format, version 1

Normative layout of the sync file (ADR-0018). Status: **accepted** with ADR-0018 (2026-11-27). All integers are big-endian. A file is one header followed by one encrypted body. It is a separate format from the [vault file](vault-format-v1.md).

## Container

| Offset | Size | Field | Value / meaning |
|-------:|-----:|-------|-----------------|
| 0 | 8 | `magic` | ASCII `MFASYNC` followed by one zero byte |
| 8 | 2 | `version` | `1` |
| 10 | 1 | `kdf_id` | `1` = Argon2id (version 0x13) then HKDF-SHA-256 |
| 11 | 1 | `aead_id` | `1` = XChaCha20-Poly1305 |
| 12 | 16 | `sync_id` | random; names one master key |
| 28 | 4 | `kdf_memory_kib` | Argon2 memory cost in KiB |
| 32 | 4 | `kdf_iterations` | Argon2 time cost |
| 36 | 1 | `kdf_lanes` | Argon2 parallelism |
| 37 | 16 | `salt` | random, new whenever the password slot is written |
| 53 | 24 | `wrap_nonce` | random, new whenever the password slot is written |
| 77 | 48 | `wrapped_mk` | AEAD(master key, 32 B) = 32 B ciphertext + 16 B tag |
| 125 | 24 | `body_nonce` | random, new on every write |
| 149 | 4 | `body_len` | length of `body` in bytes |
| 153 | `body_len` | `body` | AEAD(padded payload) = ciphertext + 16 B tag |

`body_len` must equal the remaining file length exactly. Files over 64 MiB are rejected before parsing.

## Keys
- **Master key (MK):** 32 random bytes. Not derived from anything.
- **Sync secret (S):** 16 random bytes. Never written to the file.
- `stretched` = Argon2id(`password`, `salt`, memory, iterations, lanes) → 32 B. `password` is the UTF-8 of the sync password after Unicode NFKC normalisation.
- `KEK` = HKDF-SHA-256(salt = S, input key = `stretched`, info = ASCII `mfa-provider sync v1 kek` followed by `sync_id`) → 32 B.

## Encryption
- `wrapped_mk` = XChaCha20-Poly1305(KEK, `wrap_nonce`, MK), **AAD = bytes `0..77`**.
- `body` = XChaCha20-Poly1305(MK, `body_nonce`, padded payload), **AAD = bytes `0..125`** (the whole header up to the body nonce, including the password slot).
- Padding as in the vault format: the payload, byte `0x80`, then zero bytes up to a multiple of 1024.
- No compression.

Because the body authenticates the password slot, changing the slot means writing a new body. A device that holds the MK keeps bytes `0..125` of the file it read and authenticated when it writes again.

A device that only has the MK checks, in order: magic and version, `sync_id` equal to the stored one, then the body's authentication. A different `sync_id` means another master key; a failed authentication means damage or tampering. In neither case is the file overwritten.

## KDF parameter bounds
The same bounds as the vault format: writers use at least 19456 KiB, 2 iterations and 1 lane; readers also reject more than 1048576 KiB, 20 iterations or 16 lanes, before deriving anything.

## Payload (CBOR; text keys, deterministic order)
```
{ "v": 1,
  "revision": <unsigned, 1 or more>,
  "vault": { "created_at": ..., "updated_at": ..., "entries": [...], "tombstones": [...] } }
```
- `vault` has the fields and entry layout of the vault payload, without its `v`. Unknown fields are preserved.
- The local vault's sync settings (below) are **never** part of it.
- `revision`: a writer stores one more than the largest of the revision it read and the highest it has ever seen.
- A `"v"` higher than the reader supports: refuse to open, never rewrite.

## What a joined device stores
Inside its **encrypted local vault payload**, under the top-level key `sync`, as a typed field (not as an unknown field, which a merge would copy):

```
{ "sync_id": <16 B>, "master_key": <32 B>, "sync_secret": <16 B>,
  "location": text, "highest_revision": <unsigned> }
```
`location` is a file path, or on Android a document URI. It applies to this device only.

## The sync secret as text and QR code
- Text: the 16 bytes in base32 (RFC 4648 alphabet, no padding; 26 characters), followed by 2 check characters (the first 10 bits of SHA-256 of the 16 bytes), shown as 7 groups of 4. Input ignores case, spaces and hyphens.
- QR code: `mfaprovider-sync:v1:` followed by those 28 characters. It holds no password.

## What the file reveals
That it is an mfa-provider sync file, its `sync_id`, the Argon2 settings, and its size rounded up to 1 KiB. Without the master key, nothing about the accounts.

## Golden vector
To be added with the implementation: fixed password, secret, keys, nonces and payload giving fixed file bytes.
