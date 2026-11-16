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
- [x] AGPL-3.0-or-later `LICENSE`, DCO note in CONTRIBUTING, `cargo deny` config
- [ ] GitHub Actions CI: Rust + JS gates on Windows and Linux (Android build in a later task) — **written, not yet run**: the repo has no GitHub remote
- [x] README with build/test instructions

## Decisions & notes (append-only, dated)
- 2026-11-12: Scope is skeleton only; Android toolchain/build is a separate later task to keep this one verifiable on the Windows dev machine.

- 2026-11-16: User floated a web-app mode (Rust HTTP server exposing codes over the internet behind a credential). Claude's view: feasible later because the core is I/O-free, but conflicts with ADR-0006 non-goal "no server component" and ADR-0002 "seeds never in JS", and weakens the model (one login guards all second factors). Not in MVP. If pursued: new ADR "self-hosted server mode", post-sync, LAN/VPN-first, server sends only codes. No work now.

- 2026-11-16: Linter is **oxlint** (Vite template default) instead of eslint named in ADR-0005; accepted as an implementation detail, revisit if a needed rule is missing.
- 2026-11-16: Tauri bundle identifier set to `dev.area_51.android.mfaprovider` (user's own domain/ID; `cargo check` accepts it). It becomes the Android application id, so it must not change after release.
- 2026-11-16: Tauri shell is its own crate (`app/src-tauri`, edition 2024) outside the `core/` workspace; it will depend on `mfa-core` by path. `tauri-plugin-log` removed (no logging plumbing until needed; never log secrets). Capabilities list is empty; CSP is strict (dev CSP relaxed only for Vite HMR).

- 2026-11-16: User chose for the web-app idea: server decrypts; master password + a second factor (push/email suggested — Claude to advise); rate limiting + lockout mandatory; localhost/LAN default with TLS/VPN for internet. Recorded as ADR-0010 (proposed; second factor still open). Post-sync, no work in this task.

## Progress log (append-only, dated)
- 2026-11-12: Task created.
- 2026-11-12: Toolchain check: node 24.15, npm 11.12, pnpm 10.33, git 2.55 present. **Missing: Rust (rustc/cargo/rustup) and MSVC Build Tools (no Visual Studio installer found).** winget is available. Awaiting user go-ahead to install.

- 2026-11-12: User approved installs. Rust 1.99.1 (stable, msvc) installed via winget/rustup (`%USERPROFILE%\.cargo\bin` — new shells pick it up; current shell needs PATH added). VS Build Tools C++ workload install running in background (slow).

- 2026-11-12: VS Build Tools installed (vswhere OK). Created `core/` workspace (`mfa-core` empty lib, workspace lints: forbid unsafe, clippy all=deny/pedantic=warn, `rust-toolchain.toml`, `.gitattributes` LF). `cargo fmt --check`, `clippy -D warnings`, `cargo test` all green (0 tests; linker works). Cargo is not on the Bash tool's PATH — use PowerShell with `%USERPROFILE%\.cargo\bin` added.
- Criteria done: toolchains verified; `core/` workspace green.

- 2026-11-16: Scaffolded `app/` (Vite 8, React 19, TS 7, Tailwind 4, shadcn/ui, Vitest 5) and Tauri 2 shell. Build-only packages are devDependencies. `pnpm lint/typecheck/test/build` and `pnpm tauri build --no-bundle --debug` green; `cargo clippy -D warnings` green on src-tauri; built app launches and shows the window. Committed 4d4d895.

- 2026-11-16: Added `LICENSE` (official AGPL-3.0 text), `CONTRIBUTING.md` (DCO), `README.md`, `deny.toml` (installed cargo-deny 0.20.2; `cargo deny check` passes for `core/` and `app/src-tauri`), `.github/workflows/ci.yml` (core + app on ubuntu/windows, cargo-deny, pnpm audit high+). Third-party licences all pass; AGPL added to the allow-list for our own crates.
- 2026-11-16: Known, accepted findings: RUSTSEC-2024-0370 (proc-macro-error via Linux-only gtk-rs 0.18 in Tauri) is ignored in `deny.toml` with reason; `pnpm audit` reports 1 moderate (minimatch) + 1 low (deepmerge), both only via the shadcn CLI dev dependency, not shipped. Actions in CI use major-version tags, not commit SHAs — pin SHAs when the repo goes public.
- 2026-11-16: ADR-0010 (self-hosted server mode) created as proposed; second login factor still to be decided (see ADR).

## Next step
Create/choose a GitHub remote and push so the CI workflow actually runs; fix whatever it flags, then tick the CI criterion and move this task to `done/`. Then write the MVP scope (ADR-0006) as tasks and close TASK-0001.
