# TASK-0005: Vault data model, file format and crypto

- Status: in-progress
- Created: 2026-11-17
- Updated: 2026-11-24
- Related ADRs: ADR-0003, ADR-0004

## Goal
Implement the entry model and the encrypted, versioned vault file in `mfa-core`: random 256-bit vault key wrapped by an Argon2id-derived key, XChaCha20-Poly1305 with an authenticated header. Pure functions over bytes; storage and RNG are injected.

## Acceptance criteria
- [x] Entry model per ADR-0004: UUID, `otp_type`, issuer, account, secret, `params`, icon, tags, notes, timestamps
- [x] Tombstone records `{id, deleted_at}` in the model, with a compaction function
- [x] Unknown `otp_type` entries round-trip unchanged (and unknown fields, on payload and entries)
- [x] Header stores format version and Argon2id parameters; parameters never below OWASP minimums; tuning function targets ~1 s on the device (`tune_kdf_params`, with the timing closure supplied by the caller)
- [x] Encrypt/decrypt round-trip, tamper detection (header and body), wrong-password failure, versioned-format test fixtures
- [x] Vault key and derived keys zeroized on drop (`Zeroizing`); see the zeroization limits in the notes
- [x] Binary format documented in `docs/` (spec file), with a golden test vector (`core/mfa-core/tests/golden/vault-v1.bin`)
- [ ] CI green (local: 77 tests, fmt, clippy `-D warnings`, `cargo deny` all pass; CI run pending the push)

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: none (parallel to 0003). Blocks 0006. Crate choices (argon2, chacha20poly1305, zeroize) must pass `cargo deny`; record any new ADR-worthy choice (e.g. wrap format) as an ADR before coding it.

- 2026-11-24: Format decisions are recorded in ADR-0011 (proposed) with the normative byte layout in `docs/specs/vault-format-v1.md`. Planned crates (each must pass `cargo deny`): `argon2`, `chacha20poly1305`, `ciborium`, `unicode-normalization`, plus existing `zeroize`/`thiserror`; an `Rng` trait is injected (production: OS randomness via `getrandom`, in the Tauri shell, not the core).

## Progress log (append-only, dated)
- 2026-11-17: Task created.
- 2026-11-24: Wrote ADR-0011 and `docs/specs/vault-format-v1.md` for user review. No code yet.
- 2026-11-24: User accepted ADR-0011 as written. Implemented in `core/mfa-core/src/vault/`: `model` (EntryId, Entry, Tombstone, VaultData, tombstone compaction), `payload` (deterministic CBOR codec with unknown-field preservation, 1 KiB padding), `kdf` (Argon2id `KdfParams` with floors/ceilings, NFKC, `tune_kdf_params`), `rng` (injected `Rng` trait), `container` (`create`, `open`, `OpenedVault::{save, rewrap}`). 77 tests pass locally (46 new: 17 model/payload/secret, 10 kdf, 19 container incl. 2 golden); fmt, clippy `-D warnings`, `cargo deny` clean; `getrandom`/`rand` are not in the core's dependency tree.
- 2026-11-24: Found and fixed an inconsistency in my own spec while coding: it called bytes `12..109` the slot "(`kdf_id` … `wrapped_key`)" but `kdf_id` is at offset 10; corrected to `kdf_memory_kib` … `wrapped_vk` with the fixed prefix `0..12` (magic, version, kdf_id, aead_id). No format change.

## Notes and limits (honest)
- **Zeroization is best effort.** The decrypted payload buffer, vault key, KEK and password copies are `Zeroizing`; secret bytes in the intermediate CBOR tree are wiped after encoding. On *decode*, `ciborium` builds its own value tree and the CBOR reader may leave transient copies of seed bytes that are not wiped (moved into `Secret` where possible). A stronger guarantee would need a streaming decoder; not done.
- **The golden vector is self-generated** (our code wrote it); it pins the format against regressions, it is not an independent cross-check. No second implementation of XChaCha20-Poly1305/Argon2 was run against it.
- `OpenedVault::rewrap` trusts that `file` is the file the vault was opened from/saved to; it validates the layout but does not check the body decrypts under the key.
- Known open points carried to later tasks: entry reordering representation (TASK-0007); a production `Rng` backed by OS randomness and a real timing closure for `tune_kdf_params` belong in the Tauri shell (TASK-0007/0006).

## Next step
Push, confirm CI is green on all jobs, tick the CI criterion and move this task to `done/`. Then TASK-0006 (vault lifecycle: storage trait, auto-lock, atomic save) builds on `create`/`open`/`save`/`rewrap`.
