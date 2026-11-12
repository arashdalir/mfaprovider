# ADR-0004: Vault, export formats and import sources

- Status: proposed (revised 2026-11-12: OTP type extensibility, tombstones for sync)
- Date: 2026-11-12
- Related tasks: TASK-0001

## Decision (proposed)
- **Entry**: UUID id, `otp_type` (string id, e.g. `totp`, `hotp`, `steam`, or an extension id), issuer, account, secret, type-specific `params` (key/value map validated by the type's schema — algorithm, digits, period, counter, ...), icon, tags/groups, notes, `created_at`, `updated_at`.
- **Tombstones**: deleted entries leave `{id, deleted_at}` records so deletions survive sync merges (compacted after a retention period).
- **Unknown types are preserved**: an entry whose `otp_type` has no installed extension is kept intact (shown as "unsupported type"), never dropped or rewritten.
- **Native export**: own versioned encrypted file (same KDF/AEAD as the vault, *independent* password chosen at export). **Partial export** by selection, tag/group, or all. Plaintext export (otpauth:// list) only behind explicit warning and re-authentication.
- **Import sources, priority order**: `otpauth://` URIs and QR images; Google Authenticator `otpauth-migration://`; Aegis (plain + encrypted); 2FAS; andOTP; Bitwarden JSON; WinAuth; others on demand. Each importer is an isolated codec behind one `Importer` interface.
- **Import is preview-then-merge**: new / duplicate / conflicting entries shown; per-entry skip/replace/keep-both. Duplicate = same issuer + account + secret.

## Consequences
+ Round-trips with the ecosystem; partial export covers sharing/splitting; format is sync-ready.
- Each third-party format needs fixtures/tests; formats drift.
