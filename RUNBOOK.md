# Runbook — amigo oculto

Operational notes for https://amigoocultosimples.app. Keep in sync with reality.

## Where things live

| Thing | Where |
| --- | --- |
| Repo | `asmello/amigo-oculto`, branch `main` (single branch, squash merges) |
| Hosting | Railway, project `amigo-oculto`, environment `production`, service `amigo-oculto` |
| Domain | `amigoocultosimples.app` — CNAME to the Railway domain, proxied by Cloudflare |
| Database | SQLite on a 5 GB volume, `/app/data/amigo_oculto_v2.db`. The volume started fresh at the v2 cutover; v1 data survives only under the Wasabi `db` prefix (and the `v1` tag for code) |
| Backups | Litestream → Wasabi S3 (`LITESTREAM_BUCKET`, prefix `LITESTREAM_PATH=v2`; v1 lives under `db`), 10 s sync, one week of point-in-time restore. This is the **only** backup: Railway volume backups need a Pro workspace and the project is on Hobby |
| Mail | smtp2go (free tier 1 000/month); from `naoresponda@amigoocultosimples.app` |
| Admin | `/painel`, magic link to the emails in `ADMIN_EMAILS` (`/admin` is blocked by the Cloudflare WAF on purpose) |
| Edge | Cloudflare proxy with WAF custom rules that affect routes and static files, see [Cloudflare](#cloudflare) |

Secrets live only in Railway variables (service → Variables). After any suspected leak, rotate at
the provider (smtp2go / Wasabi keys) and keep the old key until the new one is proven:

1. Create the new key. Set each value with `railway variable set KEY --stdin --skip-deploys`, then
   redeploy once.
2. Wasabi: make a write (e.g. a "recuperar" request for an unknown address), check that a new
   `.ltx` object appears under the prefix, and run the restore drill below with the new key.
   smtp2go: request an admin login link and check it arrives.
3. Only then delete the old key at the provider.

The Dockerfile `CMD` (`/app/run.sh`) is the single source of truth for the start command; the
dashboard override must stay empty. The container starts as root only to `chown` the volume, then
re-executes `run.sh` as `appuser` (no `RAILWAY_RUN_UID` needed).

## Daily reality check

1. `GET https://amigoocultosimples.app/healthz` → `ok`.
2. Railway logs: an `email sent` (or retry) line when mail should be flowing; no repeated panics.
3. Litestream: `snapshot complete` log once a day; sync errors would appear continuously.
4. During December: check the organizer pages people email about (delivery chips per participant).

## Common incidents

### Email "não chegou"
- Organizer page → per-participant chip: `Convite enviado` / `Email rejeitado` / `Enviando`.
- Rejected (permanent SMTP error) → fix the address (edit participant; new link is issued and
  emailed automatically) or wait out the 10-minute resend throttle.
- `Enviando` for long → check smtp2go dashboard (quota/outage) and Railway logs for the retry
  backoff (30 s → 2 min → 10 min → 1 h → 6 h, then failed). Rows whose link was rotated
  before they went out (redraw, resend, email change) are marked `cancelled`, never sent.

### Volume wiped / fresh deploy restores empty DB
`run.sh` restores the latest Litestream replica automatically at boot (`-if-db-not-exists
-if-replica-exists`). Deliberate restore of an older point:

Verified drill (any Docker host, the app image carries Litestream and `/etc/litestream.yml`;
the five `LITESTREAM_*` values are the service's variables, pass them from memory, never from a
file):

```sh
docker run --rm -v "$PWD/restore:/out" \
  -e LITESTREAM_BUCKET -e LITESTREAM_PATH=v2 -e LITESTREAM_ENDPOINT \
  -e LITESTREAM_ACCESS_KEY_ID -e LITESTREAM_SECRET_ACCESS_KEY \
  --entrypoint litestream <image> restore -o /out/restored.db /app/data/amigo_oculto_v2.db
# add -timestamp 2026-12-20T00:00:00Z for point-in-time; inspect with sqlite3 restore/restored.db
# to roll production back: stop the service, replace /app/data/amigo_oculto_v2.db on the volume, start it
```

### Someone deleted a game by accident
Point-in-time restore (above, `-timestamp` just before the deletion) or accept the loss; there is no soft-delete. Ask the organizer to
recreate: costs one email verification.

### Emails being used for spam (create form)
App rate limits: 3 verification codes/email/hour, 10/IP/hour, 5 games/day/email, recovery
2/email/hour. Cloudflare also rate limits the email-sending routes per IP (see [Cloudflare](#cloudflare)).
If more is needed: Railway variable `RUST_LOG=debug` temporarily **is not** safe with tokens in
URLs — paths are redacted, but prefer adding limits over lowering log level.

### App down (crash loop)
Railway restarts on failure (`restartPolicyType = "ON_FAILURE"`, 3 retries). If the DB is wedged
(`database is locked` in logs), restart the service once; if it persists, restore a backup and
report — worker and cleanup both hold short transactions, so locks indicate a bug, not load.

## Cloudflare

The zone's WAF custom rules run before the app on every request to `amigoocultosimples.app`. The
`*.up.railway.app` domain bypasses Cloudflare, so smoke tests there cannot catch an edge block.

- File extensions are allow-listed (html, js, css, images, json, woff2, ics; `/.well-known/` is
  exempt). A new static file type is blocked until it is added to the rule.
- Scanner paths and keywords (`php`, `wp-`, `wordpress`, `swagger`, …) are blocked, except under
  `/revelar/` and `/entrar/`: their base64url secrets can contain those strings by chance. IDs
  elsewhere are ULIDs (uppercase, no `-`), which cannot. Random tokens in a path belong only under
  those two prefixes.
- `/admin` is blocked on purpose as a scanner trap; the panel lives at `/painel`.
- The email-sending routes (`/`, `/criar`, `/recuperar`, `/painel/entrar`, `/verificar/…`) are rate
  limited per IP at the edge, on top of the app's own limits.

Changing rules needs a Cloudflare API token with Zone → WAF: Edit. After any route or rule change,
request each affected path through `https://amigoocultosimples.app` and check that the response
comes from the app, not a Cloudflare block page.

## Cutover / deploy checklist (one branch, one service)

1. PR → CI: fmt, clippy, tests, Docker image smoke test.
2. Squash-merge (GitHub signs it; ruleset requires it).
3. Railway deploys `main`; watch healthcheck `/healthz` and the deploy logs.
4. Verify an email flow end-to-end on prod after any mail change.

## Cost

~$1/month all-in at family scale: ≈40 MB RAM always on + negligible CPU + a volume using
~85 MB; Litestream's Wasabi storage and traffic are pennies. Budget line: Railway Hobby $5.
