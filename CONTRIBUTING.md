# Contributing

This project is licensed under AGPL-3.0-or-later (ADR-0009). Read `CLAUDE.md` and `docs/adr/README.md` before starting; decisions live in ADRs and work is tracked in `docs/tasks/`.

## Sign-off (DCO)
Every commit must carry a `Signed-off-by` line, certifying the [Developer Certificate of Origin](https://developercertificate.org/):

```
git commit -s
```

## Rules of the road
- Conventional Commits (`feat:`, `fix:`, `docs:` ...); short-lived branches off `main`.
- Tests accompany every behaviour; CI must be green (see `.github/workflows/ci.yml`).
- Never commit vaults, keystores, `.env` files or real TOTP seeds. Never log secrets.
- Non-trivial decisions need an ADR; accepted ADRs are superseded, never edited.
