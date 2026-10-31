# Notes for coding agents

- Server-rendered Rust app (axum + askama). **There is no frontend build.** Do not add Node,
  bundlers or client frameworks; `static/app.js` is deliberately tiny and optional.
- Every user-visible flow has an integration test in `tests/` against the real router with
  in-memory SQLite and a capturing mailer; add one for anything new before touching handlers.
- Product rules that must not regress:
  - The organizer (and admin) can never see who drew whom. Admin pages list metadata only.
  - Reveal links, login links and sessions are stored **hashed**.
  - Any post-draw change (participants, exclusions) triggers a full redraw; old links die.
  - Reveal state only changes via POST. GET never mutates.
  - No secrets in query strings in logs: request tracing redacts `/revelar/…`, `/entrar/…`.
- UI copy is pt-BR; dates as `24 de dezembro de 2026`, money as `R$ 80` (see `src/fmt.rs`).
- `run.sh` + Dockerfile CMD are the deploy's single source of truth; keep the Railway dashboard
  start-command override empty.
- Production sits behind Cloudflare WAF rules (RUNBOOK → Cloudflare). A new static file type, or a
  path containing a dot or a scanner keyword (`php`, `wp-`, …), is blocked at the edge until the
  rules change. Random secrets in paths belong only under `/revelar/` or `/entrar/`.
- Ops: RUNBOOK.md. Full architecture: README.md.
