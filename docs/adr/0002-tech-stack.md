# ADR-0002: Tauri 2 — Rust core + web UI (React, TypeScript, Tailwind, shadcn/ui)

- Status: accepted (2026-11-12; user is not a Rust developer and delegated the choice to Claude, who is comfortable with Rust)
- Date: 2026-11-12
- Related tasks: TASK-0001

## Context
Targets: Windows, Linux, Android now; macOS/iOS later. Security-critical: crypto, secret handling and vault format must behave identically everywhere. User wants a web-app look with shadcn/ui + Tailwind.

## Options considered
1. Flutter (+ Rust core) — not a web UI; rejected on user preference.
2. Kotlin Multiplatform / .NET MAUI — no web UI; MAUI lacks Linux.
3. Electron + Capacitor — two shells, crypto in JS/WASM, weaker secret hygiene.
4. PWA only — no hardware-backed keystore; unacceptable.
5. **Tauri 2** — Rust backend + system webview; Windows, Linux, macOS, Android and iOS from one project. shadcn/ui + Tailwind run unchanged in the webview.

## Decision (proposed)
Option 5. Rust core (`mfa-core`) owns vault format, KDF/encryption, OTP generation, import/export codecs, sync merge and zeroization. UI is React + TypeScript + Tailwind + shadcn/ui (Vite build, bundled assets only).

### Webview hardening rules (mandatory)
- **Seeds never enter JavaScript.** Rust pushes/serves only computed codes and display metadata. Exceptions: explicit "reveal" and edit flows, behind re-auth, and wiped after use.
- Narrow, typed IPC commands; minimal Tauri capabilities/permissions; no shell/fs access from the webview.
- Strict CSP (no `unsafe-eval`, no remote scripts/styles/fonts/images), no remote content, devtools off in release builds.
- Dependency discipline on the JS side (lockfile, audit in CI, few runtime deps) — supply-chain risk is the main cost of a web UI.
- Android: `FLAG_SECURE` and Keystore/biometric via a small Kotlin Tauri plugin (core logic stays in Rust).

## Consequences
+ One vetted implementation of everything sensitive; UI is replaceable; shadcn/Tailwind available.
+ Android supported via the NDK (Rust targets aarch64/armv7/x86_64/i686-linux-android); Tauri automates builds.
- Webview differences (WebView2 on Windows, WebKitGTK on Linux, Android System WebView) need cross-platform UI testing.
- Tauri mobile is younger than desktop; some plugins (SAF file access, Keystore) are custom or community code.
- Needs Node + Rust + Android SDK/NDK toolchains.
