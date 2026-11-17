# TASK-0005: Vault data model, file format and crypto

- Status: open
- Created: 2026-11-17
- Updated: 2026-11-17
- Related ADRs: ADR-0003, ADR-0004

## Goal
Implement the entry model and the encrypted, versioned vault file in `mfa-core`: random 256-bit vault key wrapped by an Argon2id-derived key, XChaCha20-Poly1305 with an authenticated header. Pure functions over bytes; storage and RNG are injected.

## Acceptance criteria
- [ ] Entry model per ADR-0004: UUID, `otp_type`, issuer, account, secret, `params`, icon, tags, notes, timestamps
- [ ] Tombstone records `{id, deleted_at}` in the model, with a compaction function
- [ ] Unknown `otp_type` entries round-trip unchanged
- [ ] Header stores format version and Argon2id parameters; parameters never below OWASP minimums; tuning function targets ~1 s on the device
- [ ] Encrypt/decrypt round-trip, tamper detection (header and body), wrong-password failure, versioned-format test fixtures
- [ ] Vault key and derived keys zeroized on drop
- [ ] Binary format documented in `docs/` (spec file), with a golden test vector

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: none (parallel to 0003). Blocks 0006. Crate choices (argon2, chacha20poly1305, zeroize) must pass `cargo deny`; record any new ADR-worthy choice (e.g. wrap format) as an ADR before coding it.

## Progress log (append-only, dated)
- 2026-11-17: Task created.

## Next step
Draft the vault file format spec (header layout, versioning, wrapped-key structure) in `docs/` and review it with the user before implementing.
