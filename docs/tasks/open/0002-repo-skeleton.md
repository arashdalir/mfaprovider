# TASK-0002: Repo skeleton (layout, CI, lint, test harness)

- Status: in-progress
- Created: 2026-11-12
- Updated: 2026-11-12
- Related ADRs: ADR-0002, ADR-0005, ADR-0009

## Goal
A buildable, empty-but-wired monorepo per ADR-0005: Rust workspace (`core/`), Tauri 2 + React/TS/Tailwind/shadcn app (`app/`), lint/format/test tooling, licence files, and GitHub Actions CI that gates every change. No product features yet.

## Acceptance criteria
- [x] Toolchains verified/documented (Rust, Node, Tauri prerequisites; Android SDK/NDK deferred to a later task)
- [x] `core/` Rust workspace with `mfa-core` (and placeholder crates only where needed now), `cargo fmt`, `clippy -D warnings`, `cargo test` green
- [x] `app/` Tauri 2 shell + Vite React TS + Tailwind + shadcn/ui builds; strict CSP and minimal capabilities per ADR-0002
- [x] lint (oxlint), `tsc --noEmit`, `vitest` green
- [ ] AGPL-3.0-or-later `LICENSE`, DCO note in CONTRIBUTING, `cargo deny` config
- [ ] GitHub Actions CI: Rust + JS gates on Windows and Linux (Android build in a later task)
- [ ] README with build/test instructions

## Decisions & notes (append-only, dated)
- 2026-11-12: Scope is skeleton only; Android toolchain/build is a separate later task to keep this one verifiable on the Windows dev machine.

- 2026-11-16: User floated a web-app mode (Rust HTTP server exposing codes over the internet behind a credential). Claude's view: feasible later because the core is I/O-free, but conflicts with ADR-0006 non-goal "no server component" and ADR-0002 "seeds never in JS", and weakens the model (one login guards all second factors). Not in MVP. If pursued: new ADR "self-hosted server mode", post-sync, LAN/VPN-first, server sends only codes. No work now.

- 2026-11-16: Linter is **oxlint** (Vite template default) instead of eslint named in ADR-0005; accepted as an implementation detail, revisit if a needed rule is missing.
- 2026-11-16: Tauri bundle identifier set to `org.mfaprovider.app` as a placeholder. It becomes the Android application id and is painful to change after release — **user to confirm/replace before any release**.
- 2026-11-16: Tauri shell is its own crate (`app/src-tauri`, edition 2024) outside the `core/` workspace; it will depend on `mfa-core` by path. `tauri-plugin-log` removed (no logging plumbing until needed; never log secrets). Capabilities list is empty; CSP is strict (dev CSP relaxed only for Vite HMR).

## Progress log (append-only, dated)
- 2026-11-12: Task created.
- 2026-11-12: Toolchain check: node 24.15, npm 11.12, pnpm 10.33, git 2.55 present. **Missing: Rust (rustc/cargo/rustup) and MSVC Build Tools (no Visual Studio installer found).** winget is available. Awaiting user go-ahead to install.

- 2026-11-12: User approved installs. Rust 1.99.1 (stable, msvc) installed via winget/rustup (`%USERPROFILE%\.cargo\bin` — new shells pick it up; current shell needs PATH added). VS Build Tools C++ workload install running in background (slow).

- 2026-11-12: VS Build Tools installed (vswhere OK). Created `core/` workspace (`mfa-core` empty lib, workspace lints: forbid unsafe, clippy all=deny/pedantic=warn, `rust-toolchain.toml`, `.gitattributes` LF). `cargo fmt --check`, `clippy -D warnings`, `cargo test` all green (0 tests; linker works). Cargo is not on the Bash tool's PATH — use PowerShell with `%USERPROFILE%\.cargo\bin` added.
- Criteria done: toolchains verified; `core/` workspace green.

- 2026-11-16: Scaffolded `app/` (Vite 8, React 19, TS 7, Tailwind 4, shadcn/ui, Vitest 5) and Tauri 2 shell. Build-only packages are devDependencies. `pnpm lint/typecheck/test/build` and `pnpm tauri build --no-bundle --debug` green; `cargo clippy -D warnings` green on src-tauri; built app launches and shows the window. Committed 4d4d895.

## Next step
Add `LICENSE` (AGPL-3.0-or-later full text), `CONTRIBUTING.md` (DCO sign-off), `deny.toml` for `cargo deny`, GitHub Actions CI (Rust + JS gates, Windows + Linux), README with build/test instructions. Then close TASK-0002 and write MVP tasks.
