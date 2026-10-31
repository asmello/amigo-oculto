# Amigo Oculto Simples

Secret Santa for the family. The organizer creates the event and adds the participants; the app
draws the names and emails each person a secret link to find out who they drew — no sign-up,
no app to install, and nobody sees the result, not even the organizer.

One Rust service, server-rendered HTML, SQLite on a volume. No Node, no SPA, no build step beyond
`cargo`. The user interface is in Brazilian Portuguese; code, comments and documentation are in
English.

## Features

- **Create with email verification** — a 6-digit code, rate-limited per email and per IP.
- **Participants** — add one at a time or paste a list (`Name, email` / `Name <email>`), edit, remove.
- **Exclusions** — symmetric pairs (couples who should not draw each other); impossible
  combinations are refused before anything changes.
- **Draw** — a single cycle through everyone when one exists, any derangement otherwise; every
  reveal link is a 256-bit secret stored only as a SHA-256 hash.
- **Redraw semantics** — any change to participants or exclusions after the draw triggers a full
  redraw: everyone receives a new link and the old ones stop working. Nothing is ever patched in.
- **Outbox email** — invitations, welcome, resend, recovery and admin links are queued in the
  database, in the same transaction as the change that caused them; a background worker delivers
  them with exponential backoff and distinguishes permanent from transient SMTP failures; the
  organizer sees per-participant delivery status. A redraw or a re-sent link retires invitations
  still waiting in the queue, so a dead link is never delivered.
- **Reveal on POST** — opening the email or the page never counts; only pressing the reveal button does.
- **Gift hints** — written by each participant on their reveal page, visible only to whoever drew them.
- **Self-service link recovery** ("Perdi meu link") — never reveals whether an email is known.
- **Organizer access** — magic-link login to a cookie session; no passwords anywhere.
- **Minimal admin** — magic link restricted to configured emails; lists games, never matches or tokens.
- **Budget, message from the organizer, `.ics` calendar file, pt-BR dates and money.**
- **Privacy by construction** — tokens hashed at rest, secrets redacted from logs, security
  headers (CSP, HSTS, frame-deny), cross-site POSTs rejected, sliding-window rate limits, all data
  deleted 90 days after the event.

## Stack

| Piece | Choice |
| --- | --- |
| Language | Rust, edition 2024, MSRV 1.90, one crate |
| Web | axum 0.8; askama 0.16 templates (`templates/`) via askama_web |
| Database | SQLite via sqlx 0.8, `sqlx::migrate!` (WAL, replicated by Litestream) |
| Mail | lettre 0.11 over SMTP (rustls), multipart text + HTML, `Reply-To` set to the organizer |
| Frontend | hand-written CSS (`static/app.css`), one small vanilla JS file, self-hosted Fraunces subset |

Static assets and email templates are embedded in the binary (`include_bytes!`, askama code
generation), so the image ships a single executable plus Litestream.

## Development

```sh
cp .env.example .env   # MAIL_TRANSPORT=log by default: emails are printed to stdout
cargo run              # http://localhost:3000
```

The SQLite file defaults to `./dev.db`; migrations run on boot.

Cookies always carry the `Secure` attribute, so develop on `http://localhost`: Chromium and
Firefox accept Secure cookies there, Safari does not (use another browser or put the app behind
local TLS).

### Tests

```sh
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test
```

Integration tests (`tests/`) run the real router against an in-memory database with a capturing
mailer: organizer and participant flows end to end, draw/redraw semantics, reveal semantics,
recovery, admin, rate limits and the outbox worker's retry and give-up paths.

## Configuration

Environment variables (all optional except `BASE_URL` in production):

| Variable | Default | Notes |
| --- | --- | --- |
| `PORT` | `3000` | HTTP listen port (Railway injects this) |
| `BASE_URL` | — | public origin; used in links, emails and the cross-site POST check |
| `DATABASE_URL` | `sqlite:///app/data/amigo_oculto_v2.db` | SQLite path (new name: a volume still holding the v1 file starts fresh) |
| `MAIL_TRANSPORT` | `smtp` if `SMTP_HOST` is set, else `log` | `log` prints emails instead of sending them |
| `SMTP_HOST`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM` | — | SMTP relay (port 465 = implicit TLS, others STARTTLS) |
| `LITESTREAM_BUCKET`, `LITESTREAM_PATH`, `LITESTREAM_ENDPOINT`, `LITESTREAM_ACCESS_KEY_ID`, `LITESTREAM_SECRET_ACCESS_KEY` | — | off-provider replication; `LITESTREAM_PATH` is the prefix inside the bucket (production: `v2`) and is mandatory with the bucket |
| `ADMIN_EMAILS` | — | comma-separated emails allowed to log into the admin panel at `/painel` |
| `TRUST_PROXY` | `true` | trust `X-Forwarded-For` / `CF-Connecting-IP` for rate limiting |
| `GAME_RETENTION_DAYS` | `90` | days after the event date before a game is deleted |
| `RUST_LOG` | `info` | request tracing redacts the secret path segments |

## Deployment (Railway)

- The repository deploys as-is from `main`: `railway.toml` selects the Dockerfile, whose `CMD`
  is `run.sh` — the single source of truth. Do not override the start command in the dashboard.
- `run.sh` restores the database from Wasabi through Litestream when the volume is empty, then runs
  the app under `litestream replicate -exec`, so every write is replicated off-provider within
  seconds.
- Railway's native daily volume backups are enabled as a second line of defence.
- Health check: `/healthz`. Logs run at `info`; the secret path segments (`/revelar/…`,
  `/entrar/…`) are rewritten to `[token]` before logging.
- Restore, backups, DNS and incident procedures: see [RUNBOOK.md](RUNBOOK.md).

## Repository layout

```
src/
  main.rs        binary: config, pool, background workers, graceful shutdown
  web/           router, middleware (headers, cross-site check, rate limit), handlers, sessions
  repo/          SQL queries (one module per table)
  mail/          SMTP transports, email composition, outbox worker, .ics
  matching.rs    draw algorithm (Hamiltonian cycle with a derangement fallback)
templates/       askama pages and transactional emails (HTML and plain text)
static/          app.css, app.js, favicon, Fraunces subset (embedded in the binary)
migrations/      SQL migrations (applied on boot)
tests/           integration tests (in-memory SQLite + capturing mailer)
```

MIT licensed. Fonts: [Fraunces](https://github.com/undercasetype/Fraunces) (OFL 1.1);
icons derived from [Lucide](https://lucide.dev) (ISC).
