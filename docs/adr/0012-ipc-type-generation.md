# ADR-0012: Typed IPC between Rust and the UI — `ts-rs` with a hand-written client

- Status: accepted (2026-11-24, by the user, as written)
- Date: 2026-11-24
- Related tasks: TASK-0007 (also constrains TASK-0008, TASK-0009, TASK-0010, TASK-0011)
- Refines: ADR-0002 ("typed IPC client generated from Rust types"; "seeds never enter JavaScript")

## Context
The React UI talks to the Rust core through Tauri commands. ADR-0005 asked for a typed IPC client generated from Rust types, so the two sides cannot drift apart; ADR-0002 forbids seeds in JavaScript. A generator has to be chosen before the first command exists.

State of the candidates, checked on crates.io on 2026-11-24:

| | `tauri-specta` + `specta` | `ts-rs` |
|---|---|---|
| Release line | **pre-release**: `tauri-specta` 2.0.0-rc.29, `specta` 2.0.0-rc.28, `specta-typescript` 0.0.15 — three crates versioned separately, `specta` pinned with an exact `=` requirement | **stable**: 12.0.1, declared minimum Rust 1.78 |
| Last updates | 2026-11-05 and 2026-11-13 (still changing fast) | 2026-01-31 |
| What it types | data types **and** commands/events (generates a `commands.*` client) | data types only |
| Tauri coupling | tight (depends on `tauri ^2`) | none |

## Options considered
1. **`tauri-specta`** — one source of truth for command names, arguments, results and errors; no string command names in TypeScript. Cost: a pre-release dependency chain on the security boundary of the app; breaking changes between release candidates are expected, and its exact-version pinning ties our `Cargo.lock` to a moving target.
2. **`ts-rs` for data types + a small hand-written typed client** — stable generator; commands are called through one module of typed wrappers. Cost: command names exist as strings in TypeScript and must be kept in line by a test.
3. **Hand-written TypeScript types** — no tooling, but silent drift. Rejected.
4. **JSON Schema → TypeScript** (e.g. `schemars` + a Node generator) — adds a second toolchain for no benefit. Rejected.

## Decision
Option 2, to be revisited when `tauri-specta` reaches a stable 2.0 (a superseding ADR then).

1. **Wire types are separate DTOs** in the Tauri shell (`app/src-tauri/src/ipc/`). Core types (`Entry`, `Secret`, `VaultData`) never derive `TS` and are never serialised across the boundary. Only DTOs derive `TS` and `Serialize`.
2. **No secret field exists on any DTO.** The entry DTO carries display metadata, the current code and seconds remaining — not the seed. The only type that carries a seed is a separately named `RevealedSecret`, returned by exactly one command that requires re-authentication; it is wiped after use (ADR-0002).
3. **Generated types are committed** under `app/src/ipc/generated/`. CI regenerates them (`cargo test`) and fails on any diff, so Rust and TypeScript cannot drift.
4. **One module calls `invoke`**: `app/src/ipc/client.ts` holds one typed function per command. A lint rule forbids importing Tauri's `invoke` anywhere else. A test checks that the command names in `client.ts` equal the registered command list.
5. **Errors are a closed set**: an `IpcError` DTO with a stable `code` enum and an optional safe message. Messages never contain secrets, passwords or file contents.
6. **Numbers**: millisecond timestamps are sent as JSON numbers (safe below 2^53). `ts-rs` may map 64-bit integers to `bigint` by default; DTOs override this explicitly and a test proves the generated type matches what `serde_json` really sends.
7. **Proof of the no-secrets rule**: a test serialises a sample of every DTO built from entries with known seed bytes and asserts the seed bytes appear nowhere in the JSON; a second test fails if a `TS`-deriving type contains a `Secret` or byte-array field outside `RevealedSecret`.

## Consequences
+ Stable generator on the security boundary; no dependence on release candidates.
+ The no-secrets rule is enforced by the type layout and by tests, not by convention.
- Roughly one hand-written wrapper per command (about a dozen for the MVP); drift in command *names* is caught by a test rather than by the compiler.
- Two things to verify at implementation time rather than assume: `ts-rs`'s transitive dependency footprint (`cargo tree`, `cargo deny`) and its handling of 64-bit integers.
