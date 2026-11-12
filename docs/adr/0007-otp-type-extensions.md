# ADR-0007: OTP types as pluggable, sandboxed extensions

- Status: accepted (2026-11-12)
- Date: 2026-11-12
- Related tasks: TASK-0001

## Context
User wants each new OTP type (TOTP, HOTP, Steam, vendor-specific, ...) to be an installable extension. An OTP generator necessarily receives the seed, so an untrusted extension is a direct threat to the vault.

## Options considered
1. Native plugins (dlls/so) — full access to process memory; rejected.
2. JS plugins in the webview — seeds would have to enter JS; rejected (ADR-0002).
3. **WASM modules in a sandbox inside the Rust core** — no imports (no fs/network/clock), fuel/memory limits, deterministic.
4. Built-ins only — simplest, not extensible.

## Decision
- Core defines `OtpType` (id, parameter schema, `generate(secret, params, input) -> Code`, `input` = time or counter). Registry maps `otp_type` ids to implementations.
- TOTP, HOTP and Steam Guard are **built-ins implemented through this same trait** from day one.
- Extensions (post-MVP): a package = manifest (id, version, parameter schema for UI form generation, output alphabet/length, required core API version) + `.wasm`. Hosted by `mfa-ext-host` (wasmtime or wasmi) with zero host imports, memory and fuel caps. Output validated against the manifest (length/alphabet) so a module cannot smuggle bulk data out.
- Install = from local file with signature/hash shown and explicit user confirmation; extension list is visible and removable; an optional curated index can come later.
- Unknown-type entries are preserved untouched (ADR-0004).

## Consequences
+ Real extensibility without trusting extension code with I/O.
+ MVP pays only for the trait + registry.
- A WASM runtime adds binary size (notably on Android) and a versioned extension ABI to maintain.
- Extensions that need network or secure-element access (some vendor tokens) are out of reach by design.
