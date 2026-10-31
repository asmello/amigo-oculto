# Runbook — amigo oculto

Operational notes for https://amigoocultosimples.app. Keep in sync with reality.

## Where things live

| Thing | Where |
| --- | --- |
| Repo | `asmello/amigo-oculto`, branch `main` (single branch, squash merges) |
| Hosting | Railway, project `amigo-oculto`, environment `production`, service `amigo-oculto` |
| Domain | `amigoocultosimples.app` — CNAME to the Railway domain, proxied by Cloudflare |
| Database | SQLite volume, `/app/data/amigo_oculto_v2.db` (500 MB). `amigo_oculto.db` on the same volume is the dead v1 file; delete it with `railway volume files` when convenient |
| Off-provider backups | Litestream → Wasabi S3 (`LITESTREAM_BUCKET`, prefix `LITESTREAM_PATH=v2`; v1 lives under `db`), daily snapshot kept 7 days, 10 s sync |
| Provider backups | Railway volume snapshots, daily schedule on the volume (Backups tab) |
| Mail | smtp2go (free tier 1 000/month); from `naoresponda@amigoocultosimples.app` |
| Admin | `/admin`, magic link to the emails in `ADMIN_EMAILS` |

Secrets live only in Railway variables (service → Variables). After any suspected leak, rotate at
the provider (smtp2go / Wasabi keys), then `railway variable set --stdin` with the new values.

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
  backoff (30 s → 2 min → 10 min → 1 h → 6 h, then failed).

### Volume wiped / fresh deploy restores empty DB
`run.sh` restores the latest Litestream replica automatically at boot (`-if-db-not-exists
-if-replica-exists`). Deliberate restore of an older point:

```sh
litestream restore -o /tmp/restore.db s3://<bucket>/v2    # pick -timestamp as needed
# stop the service, replace /app/data/amigo_oculto.db, start the service
```

Railway snapshots restore from the volume's Backups tab (same project/environment only).

### Someone deleted a game by accident
Restore from snapshot (above) or accept the loss; there is no soft-delete. Ask the organizer to
recreate: costs one email verification.

### Emails being used for spam (create form)
Rate limits: 3 verification codes/email/hour, 10/IPC/hour, 5 games/day/email, recovery 2/email/hour.
If more is needed: Railway variable `RUST_LOG=debug` temporarily **is not** safe with tokens in
URLs — paths are redacted, but prefer adding limits over lowering log level.

### App down (crash loop)
Railway restarts on failure (`restartPolicyType = "on-failure"`). If the DB is wedged
(`database is locked` in logs), restart the service once; if it persists, restore a backup and
report — worker and cleanup both hold short transactions, so locks indicate a bug, not load.

## Cutover / deploy checklist (one branch, one service)

1. PR → CI: fmt, clippy, tests, Docker image smoke test.
2. Squash-merge (GitHub signs it; ruleset requires it).
3. Railway deploys `main`; watch healthcheck `/healthz` and the deploy logs.
4. Verify an email flow end-to-end on prod after any mail change.

## Cost

~$1/month all-in at family scale: ≈40 MB RAM always on + negligible CPU + 500 MB volume; volume
backups and Litestream traffic are pennies. Budget line: Railway Hobby $5.
