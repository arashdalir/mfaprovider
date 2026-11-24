# MFA Provider — app

Tauri 2 shell (`src-tauri/`, Rust) around a React + TypeScript UI (`src/`). All vault and OTP logic lives in the Rust core (`../core`); the UI talks to it only through `src/ipc/client.ts` (ADR-0012).

## Run the real app

```powershell
pnpm install
pnpm tauri dev
```

If port 5173 is already taken by another project:

```powershell
pnpm tauri dev --config src-tauri/tauri.alt-port.conf.json
```

The vault is created on first run under the app data folder (on Windows `%APPDATA%\dev.area_51.android.mfaprovider\vault.mfa`). Delete that folder to start over.

## Checks

```powershell
pnpm lint; pnpm typecheck; pnpm test
cargo test --manifest-path src-tauri/Cargo.toml   # also regenerates src/ipc/generated (commit it)
```
