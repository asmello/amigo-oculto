# Amigo Oculto Simples

Sorteio de amigo oculto para a família: você cria o evento, adiciona os participantes, e cada um
recebe por email um link secreto para descobrir quem tirou — sem cadastro, sem aplicativo,
e sem que o organizador veja o resultado.

A single Rust service, server-rendered HTML, SQLite on a volume. No Node, no SPA, no build step
beyond `cargo`. UI in Brazilian Portuguese.

## Features

- **Create with email verification** — 6-digit code, rate limited per email and IP.
- **Participants** — add one by one, paste a list (`Nome, email` / `Nome <email>`), edit, remove.
- **Exclusions** — symmetric pairs ("casais can't draw each other"); infeasible combinations are refused.
- **Draw** — single-cycle matching when possible, derangement fallback; every reveal link is a
  256-bit secret stored only as a SHA-256 hash.
- **Redraw semantics** — any post-draw change to participants or exclusions triggers a full redraw:
  everyone gets a new link and the old ones die. Nothing is ever patched in.
- **Outbox email** — invitations, welcome, resend, recovery and admin links queue in the database;
  a background worker delivers with exponential backoff; permanent vs transient failures from SMTP
  are classified; per-participant delivery status on the organizer page.
- **Reveal on POST** — opening the email or the page never counts; only pressing *Revelar* does.
- **Wishlist hints** — written on the reveal page, visible only to whoever drew that person.
- **Self-service link recovery** ("Perdi meu link") — never reveals whether an email is known.
- **Organizer access** — magic-link login to a cookie session; no passwords anywhere.
- **Minimal admin** — magic link restricted to configured emails; lists games, never matches or tokens.
- **Budget, organizer message, .ics calendar, pt-BR dates and money.**
- **Privacy by construction** — tokens hashed at rest, secrets redacted from logs and traces,
  security headers (CSP, HSTS, frame-deny), cross-site POST rejection, sliding-window rate limits,
  90-day retention after the event date.

## Stack

| Piece | Choice |
| --- | --- |
| Language | Rust, edition 2024, MSRV 1.90, one crate |
| Web | axum 0.8, askama 0.16 templates (`templates/`), askama_web for typed responses |
| Database | SQLite via sqlx 0.8, `sqlx::migrate!` (WAL; Litestream-replicated) |
| Mail | lettre 0.11 over SMTP (rustls), multipart alternative, Reply-To organizer |
| Frontend | hand-written CSS (`static/app.css`), one small vanilla JS file, self-hosted Fraunces subset |

Static assets and email templates are embedded in the binary (`include_bytes!`, askama codegen),
so the image ships a single executable plus Litestream.

## Development

```sh
cp .env.example .env   # MAIL_TRANSPORT=log by default; mails print to stdout
cargo run              # http://localhost:3000
```

The SQLite file defaults to `./dev.db`; migrations run on boot.

### Tests

```sh
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test
```

Integration tests (`tests/`) run the real router against an in-memory database with a capturing
mailer: full organizer and participant flows, draw/redraw semantics, reveal semantics, recovery,
admin, rate limits and the outbox worker retry/backoff paths.

## Configuration

Environment variables (all optional except `BASE_URL` in production):

| Variable | Default | Notes |
| --- | --- | --- |
| `PORT` | `3000` | HTTP listen port (Railway injects this) |
| `BASE_URL` | — | Public origin; used in links, emails and the CSRF origin check |
| `DATABASE_URL` | `sqlite:///app/data/amigo_oculto_v2.db` | SQLite path (new name: a volume still holding the v1 file starts fresh) |
| `LITESTREAM_BUCKET/PATH/ENDPOINT/ACCESS_KEY_ID/SECRET_ACCESS_KEY` | — | off-provider replication; `LITESTREAM_PATH` is the bucket prefix (prod `v2`), mandatory with the bucket |
| `MAIL_TRANSPORT` | `smtp` if `SMTP_HOST` is set, else `log` | `log` prints emails instead of sending |
| `SMTP_HOST/PORT/USERNAME/PASSWORD/FROM` | — | SMTP relay (port 465 = implicit TLS, others STARTTLS) |
| `ADMIN_EMAILS` | — | comma-separated emails allowed to log into `/admin` |
| `TRUST_PROXY` | `true` | trust `X-Forwarded-For`/`CF-Connecting-IP` for rate limits |
| `GAME_RETENTION_DAYS` | `90` | days after the event before a game is deleted |
| `RUST_LOG` | `info` | request tracing redacts secret path segments |

## Deployment (Railway)

- The repo deploys as-is from `main`: `railway.toml` selects the Dockerfile, whose `CMD` is
  `run.sh` — the single source of truth. Do not override the start command in the dashboard.
- `run.sh` restores the database from Wasabi via Litestream when the volume is empty, then runs the
  app under `litestream replicate -exec`, so every write is replicated off-provider (~10 s).
- Railway native volume backups (daily) are enabled on the volume as a second line of defence.
- Health: `/healthz`. Logs run at `info`; secret path segments (`/revelar/…`, `/entrar/…`) are
  rewritten to `[token]` before logging.
- Restore, backups, DNS, incident procedures: see [RUNBOOK.md](RUNBOOK.md).

## Repository layout

```
src/
  main.rs        binary: config, pool, workers, graceful shutdown
  web/           router, middleware (headers, CSRF, rate limit), handlers, sessions
  repo/          SQL queries (one module per table)
  mail/          SMTP transports, email composition, outbox worker, .ics
  matching.rs    draw algorithm (Hamiltonian cycle w/ fallback)
templates/       askama pages + transactional emails (HTML and plain text)
static/          app.css, app.js, favicon, Fraunces subset (embedded)
migrations/      SQL migrations (applied on boot)
tests/           integration tests (in-memory SQLite + capturing mailer)
```

MIT licensed. Fonts: [Fraunces](https://github.com/undercasetype/Fraunces) (OFL 1.1),
icons derived from [Lucide](https://lucide.dev) (ISC).
