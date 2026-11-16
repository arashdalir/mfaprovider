# ADR-0010: Self-hosted server mode (web access to codes)

- Status: proposed (2026-11-16) — decisions 1, 3 and rate limiting agreed with the user; the second login factor (decision 2) is still open
- Date: 2026-11-16
- Related tasks: TASK-0002 (idea recorded there); implementation tasks to be created after sync (ADR-0008)
- Supersedes: only the non-goal "any server component" in ADR-0006. The rest of ADR-0006 stays accepted.

## Context
The user wants the same UI reachable as a web app, served by a Rust HTTP server, so codes can be viewed from any browser. This conflicts with ADR-0002 ("seeds never enter JavaScript") unless the server does the decrypting, and it concentrates risk: one login would guard every second factor.

The desktop/Android app is unaffected: it stays free of server code and, per ADR-0003, makes no network connections until sync is enabled. Server mode is a **separate, optional binary** (`mfa-server`) that reuses `mfa-core`.

## Decisions

### 1. Where decryption happens — server decrypts (agreed)
After login the server unlocks the vault in memory with the master password and sends the browser **only the current codes and display metadata**. Seeds never reach the browser (ADR-0002 preserved). The server holds the vault key only while a session is unlocked, zeroizes it on lock/timeout, and never persists it.
- Rejected: browser-side decryption (WASM) — better against a hostile server, but seeds would enter browser JS.

### 2. Login — master password plus a second factor (open)
Master password is required, plus a second factor verified **before** the vault is unlocked, using credentials stored in the server's own config (not in the vault).
Candidates, to be chosen:
- **Passkey / WebAuthn** — phishing-resistant, no external service. Claude's recommendation.
- **TOTP** for the server login itself (the product is a TOTP app) — simple, no external service.
- **Email one-time code** — needs SMTP; the factor is only as strong as the mailbox, and a mailbox protected by the very codes this app generates is circular. Better as a recovery path than a primary factor.
- **Push approval** — needs a push channel (e.g. self-hosted ntfy or approval in the Android app) and an always-online approver; adds infrastructure.

### 3. Network exposure (agreed)
Default bind is localhost/LAN only. Internet exposure is supported only behind TLS (user-provided reverse proxy) and recommended behind a VPN/Tailscale. The server refuses to listen on a public interface without TLS configured, unless an explicit, loudly named flag is set.

### 4. Abuse protection (agreed: mandatory)
Rate limiting per IP and per account, exponential backoff, and lockout after repeated failures — applied to every authentication step, including the second factor. Failed attempts are logged without logging any secret or submitted credential.

### 5. Baseline hardening (Claude's proposal)
Short-lived sessions with idle and absolute timeouts; `HttpOnly`/`Secure`/`SameSite=Strict` cookies; CSRF protection; strict CSP; no seed or secret ever in an HTTP response, URL or log; vault file opened by the server with the same atomic-write rules as ADR-0008.

## Planned shape
- New crate `core/mfa-server` (e.g. axum) exposing the same typed commands as the Tauri shell.
- UI gains a transport layer: Tauri IPC on desktop/Android, HTTP in the browser; the React app is otherwise shared.
- Dockerfile, config file and TLS guidance.
- A threat model is written before implementation starts.

## Consequences
+ Web access without moving seeds into the browser; reuses the vetted core.
+ AGPL-3.0 (ADR-0009) suits hosted use.
- A compromised server exposes every unlocked vault; the server is a high-value target.
- New attack surface (sessions, HTTP, auth) and operational burden (TLS, updates).
- Sequenced after sync (ADR-0008) because both depend on multi-device vault handling.
