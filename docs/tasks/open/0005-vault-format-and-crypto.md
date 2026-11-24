# TASK-0005: Vault data model, file format and crypto

- Status: in-progress
- Created: 2026-11-17
- Updated: 2026-11-24
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

- 2026-11-24: Format decisions are recorded in ADR-0011 (proposed) with the normative byte layout in `docs/specs/vault-format-v1.md`. Planned crates (each must pass `cargo deny`): `argon2`, `chacha20poly1305`, `ciborium`, `unicode-normalization`, plus existing `zeroize`/`thiserror`; an `Rng` trait is injected (production: OS randomness via `getrandom`, in the Tauri shell, not the core).

## Progress log (append-only, dated)
- 2026-11-17: Task created.
- 2026-11-24: Wrote ADR-0011 and `docs/specs/vault-format-v1.md` for user review. No code yet.

## Next step
User reviews ADR-0011 / the spec. After acceptance: implement test-first — entry/tombstone model and CBOR payload (with unknown-field preservation), then the container (seal/open, bounds, error types), then Argon2 parameter tuning, then the golden vector; commit after each green step.
