# ADR-0005: Repository layout, architecture and engineering practices

- Status: accepted (2026-11-12; CI on GitHub Actions)
- Date: 2026-11-12
- Related tasks: TASK-0001

## Decision
Monorepo:

```
mfa-provider/
  core/            Rust workspace
    mfa-core/        domain + crypto + vault + OTP registry (no I/O)
    mfa-formats/     import/export codecs
    mfa-sync/        storage providers + merge
    mfa-ext-host/    WASM sandbox for OTP extensions (post-MVP)
  app/
    src-tauri/       Tauri shell: thin IPC commands, platform plugins (Android Keystore/biometric/FLAG_SECURE)
    src/             React + TS + Tailwind + shadcn/ui
  extensions/      example/SDK for OTP type extensions (post-MVP)
  docs/            ADRs, tasks
  .github/         CI
```

- Ports & adapters: the domain does no I/O; storage, clock, RNG, keystore are traits; everything is unit-testable. OTP verified against RFC 4226/6238 vectors.
- UI: feature folders, no business logic in components, typed IPC client generated from Rust types (e.g. specta/ts-rs).
- CI gates: `cargo fmt` + `clippy -D warnings`, `cargo test`, `cargo deny`/`audit`, `eslint` + `tsc --noEmit`, `vitest`, `npm audit`, Windows + Linux + Android builds.
- Conventional Commits; trunk-based, short-lived branches; git repo initialised with the skeleton.
- Licence: AGPL-3.0-or-later (ADR-0009).

## Consequences
Clear boundaries; more upfront structure than a single package.
