# mfa-provider

Open-source, free-to-use authenticator (TOTP / HOTP / Steam Guard) for Windows, Linux and Android, with a Rust core and a Tauri 2 + React UI. Licensed AGPL-3.0-or-later.

Status: in development, not yet released or security-reviewed. Architecture and decisions: [docs/adr/](docs/adr/README.md). Current work: [docs/tasks/open/](docs/tasks/open/).

## Layout
| Path | Purpose |
|------|---------|
| `core/` | Rust workspace: domain, crypto, vault (no I/O) |
| `app/` | React + TypeScript + Tailwind + shadcn/ui, and the Tauri shell in `app/src-tauri` |
| `docs/` | ADRs, tasks, session notes |

## Prerequisites
- Rust (stable, via rustup) — on Windows also Visual Studio Build Tools with the "Desktop development with C++" workload
- Node 24+ and pnpm 10+
- Linux only: the Tauri system libraries ([Tauri prerequisites](https://v2.tauri.app/start/prerequisites/))

## Build and test
```
# Rust core
cd core
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# UI
cd app
pnpm install
pnpm lint
pnpm typecheck
pnpm test

# Desktop app
pnpm tauri dev                          # run
pnpm tauri build --no-bundle --debug    # build without installers
```

Android: see [docs/android.md](docs/android.md). Sync (designed, not yet available): [docs/sync.md](docs/sync.md).

Dependency policy: `cargo deny --manifest-path core/Cargo.toml check` and `cargo deny --manifest-path app/src-tauri/Cargo.toml check` (config: `deny.toml`); `pnpm audit` for the UI.

## Contributing
See [CONTRIBUTING.md](CONTRIBUTING.md). Commits are signed off (DCO).
