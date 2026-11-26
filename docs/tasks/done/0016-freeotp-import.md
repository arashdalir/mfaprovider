# TASK-0016: Import from a FreeOTP backup (JSON)

- Status: done
- Created: 2026-11-26
- Updated: 2026-11-26
- Related ADRs: ADR-0004

## Goal
The import wizard accepts a FreeOTP / FreeOTP+ JSON backup (`freeotp-backup_*.json`) as another format on the existing `Importer` interface: same preview, merge and apply as the other formats.

## Acceptance criteria
- [x] `FreeOtpImporter` (id `freeotp`) in `mfa-formats`, with tests on invented fixtures
- [x] Secrets (signed byte arrays in the file) are zeroised after use
- [x] Unusable tokens are skipped with fixed sentences; one bad token never hides the good ones
- [x] Listed in the wizard (formats list tests updated)
- [x] The user's real backup opens in an opt-in ignored test (counts only)
- [ ] CI green

## Decisions & notes (append-only, dated)
- 2026-11-26: Sample inspected by structure only (field names and non-secret settings; no secret or account name read): top level `tokens` (array) and `tokenOrder` (array of strings); each token has `algo`, `counter`, `digits`, `issuerExt`, `label`, `period`, `secret` (array of signed bytes, 20 long), `type`, sometimes `category`. 17 tokens, all SHA1/TOTP/6 digits/30 s, 10 with empty `issuerExt`, no label contains a colon.
- 2026-11-26: Position of a token = its 1-based index in `tokens`; `tokenOrder` is ignored.
- 2026-11-26: Needs `serde` + `serde_json` in `mfa-formats` (already used by the shell crate); no ADR, dependency is ubiquitous and only reads untrusted JSON under the existing 5 MiB cap.

## Progress log (append-only, dated)
- 2026-11-26: Task created.

- 2026-11-26: Done. `FreeOtpImporter` (`mfa-formats::import::freeotp`): reads `tokens`; secret as signed bytes (`Vec<i8>`, zeroised on drop, converted through a zeroising buffer); TOTP/HOTP (HOTP keeps its counter); SHA1/256/512 (MD5 and unknown types skipped with fixed sentences); only non-default digits/period are stored; issuer from `issuerExt`, else split from `label`. New `ImportError::NotThisFormat` for files that are not this JSON shape. 10 tests (formats 107, shell 103 with the format list updated). Real backup check (counts only): 17 read, 0 skipped, 17 new. Added `serde` + `serde_json` to `mfa-formats`; `cargo deny` ok in both workspaces. **Not verified:** CI; the wizard in the real app; other FreeOTP variants (`issuerInt`/`issuerAlt`, images, original FreeOTP exports) were not seen and are ignored. Known limit: the decoded JSON text holds the secrets until the parse ends and is not zeroised (same as the link-list importer).

- 2026-11-26: Closed on the user's instruction.

## Next step
None.
