# TASK-0011: Export — encrypted full/partial, plaintext with re-auth

- Status: open
- Created: 2026-11-17
- Updated: 2026-11-17
- Related ADRs: ADR-0003, ADR-0004

## Goal
Native versioned encrypted export (same KDF/AEAD as the vault, independent password chosen at export time) for all entries, a tag/group, or a hand-picked selection; plus plaintext `otpauth://` export only behind an explicit warning and re-authentication. Export can be re-imported by the app.

## Acceptance criteria
- [ ] Export file format documented; round-trip test export → import preserves entries (incl. unknown types)
- [ ] Partial export by selection, tag/group, or all
- [ ] Plaintext export requires re-auth + explicit user confirmation; never written without both
- [ ] Export password independent from master password; wrong password fails cleanly
- [ ] UI flows and component tests
- [ ] CI green

## Decisions & notes (append-only, dated)
- 2026-11-17: Depends on: 0005, 0007. Plaintext export is the only sanctioned plaintext write of secrets (CLAUDE.md).

## Progress log (append-only, dated)
- 2026-11-17: Task created.

## Next step
After TASK-0005 and 0007: specify the export file format (reusing the vault container where possible) and write round-trip tests first.
