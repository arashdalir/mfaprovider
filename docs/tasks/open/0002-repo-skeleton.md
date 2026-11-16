# TASK-0002: Repo skeleton (layout, CI, lint, test harness)

- Status: in-progress
- Created: 2026-11-12
- Updated: 2026-11-12
- Related ADRs: ADR-0002, ADR-0005, ADR-0009

## Goal
A buildable, empty-but-wired monorepo per ADR-0005: Rust workspace (`core/`), Tauri 2 + React/TS/Tailwind/shadcn app (`app/`), lint/format/test tooling, licence files, and GitHub Actions CI that gates every change. No product features yet.

## Acceptance criteria
- [ ] Toolchains verified/documented (Rust, Node, Tauri prerequisites; Android SDK/NDK deferred to a later task)
- [ ] `core/` Rust workspace with `mfa-core` (and placeholder crates only where needed now), `cargo fmt`, `clippy -D warnings`, `cargo test` green
- [ ] `app/` Tauri 2 shell + Vite React TS + Tailwind + shadcn/ui builds; strict CSP and minimal capabilities per ADR-0002
- [ ] `eslint`, `tsc --noEmit`, `vitest` green
- [ ] AGPL-3.0-or-later `LICENSE`, DCO note in CONTRIBUTING, `cargo deny` config
- [ ] GitHub Actions CI: Rust + JS gates on Windows and Linux (Android build in a later task)
- [ ] README with build/test instructions

## Decisions & notes (append-only, dated)
- 2026-11-12: Scope is skeleton only; Android toolchain/build is a separate later task to keep this one verifiable on the Windows dev machine.

## Progress log (append-only, dated)
- 2026-11-12: Task created.
- 2026-11-12: Toolchain check: node 24.15, npm 11.12, pnpm 10.33, git 2.55 present. **Missing: Rust (rustc/cargo/rustup) and MSVC Build Tools (no Visual Studio installer found).** winget is available. Awaiting user go-ahead to install.

- 2026-11-12: User approved installs. Rust 1.99.1 (stable, msvc) installed via winget/rustup (`%USERPROFILE%\.cargo\bin` — new shells pick it up; current shell needs PATH added). VS Build Tools C++ workload install running in background (slow).

- 2026-11-12: VS Build Tools installed (vswhere OK). Created `core/` workspace (`mfa-core` empty lib, workspace lints: forbid unsafe, clippy all=deny/pedantic=warn, `rust-toolchain.toml`, `.gitattributes` LF). `cargo fmt --check`, `clippy -D warnings`, `cargo test` all green (0 tests; linker works). Cargo is not on the Bash tool's PATH — use PowerShell with `%USERPROFILE%\.cargo\bin` added.
- Criteria done: toolchains verified; `core/` workspace green.

## Next step
Scaffold `app/`: Vite + React + TS + Tailwind + shadcn/ui, then add Tauri 2 shell (`app/src-tauri`) with strict CSP and minimal capabilities (ADR-0002); get `eslint`, `tsc --noEmit`, `vitest` green. Then LICENSE/CONTRIBUTING/deny.toml, CI workflow, README.
