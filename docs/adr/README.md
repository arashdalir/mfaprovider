# ADR index

| ADR | Title | Status |
|-----|-------|--------|
| [0001](0001-record-decisions.md) | Record decisions as ADRs and tasks as files | accepted |
| [0002](0002-tech-stack.md) | Tauri 2: Rust core + React/Tailwind/shadcn UI | accepted |
| [0003](0003-security-model.md) | Vault encryption and authentication model | accepted |
| [0004](0004-vault-and-export-formats.md) | Vault, export formats, import sources | accepted |
| [0005](0005-repo-layout-and-practices.md) | Repo layout, architecture, practices | accepted |
| [0006](0006-scope-and-open-questions.md) | MVP scope, non-goals, open questions | accepted |
| [0007](0007-otp-type-extensions.md) | OTP types as sandboxed WASM extensions | accepted |
| [0008](0008-sync.md) | Sync via user-selected cloud file | accepted |
| [0009](0009-license.md) | Open-source licence (AGPL-3.0-or-later) | accepted |
| [0011](0011-vault-file-format-v1.md) | Vault file format v1 (layout, key slot, payload, bounds) | accepted |
| [0012](0012-ipc-type-generation.md) | Typed IPC: `ts-rs` DTOs + hand-written client (not `tauri-specta`, still rc) | accepted |
| [0013](0013-qr-image-decoding.md) | QR image decoding: `rqrr` + `png` + `zune-jpeg` with hard input limits | accepted |
| [0010](0010-self-hosted-server-mode.md) | Self-hosted server mode (web access to codes); supersedes ADR-0006's "no server" non-goal | proposed |
| [0014](0014-password-protected-zip-import.md) | Password-protected ZIP import: `zip` crate, minimal features, hard limits | accepted |
| [0015](0015-android-application-id-differs-from-code-namespace.md) | Android application id keeps the underscore; code namespace does not (`applicationId` vs `namespace`) | accepted |
| [0016](0016-android-biometric-unlock-plugin.md) | Android biometric unlock: own Kotlin plugin, Keystore key bound to the biometric, core unlocks with a key | accepted |
| [0017](0017-file-picker-does-not-lock.md) | The trip to a phone's file picker does not lock the vault (one departure, 2 minutes) | accepted |
| [0018](0018-sync-keys-secrets-and-merge-rules.md) | Sync: separate master key for the sync file, opened by sync password + sync secret; separate device passwords (replaces ADR-0008's same-password rule); merge rules | proposed |
