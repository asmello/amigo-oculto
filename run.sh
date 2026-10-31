#!/bin/sh
set -eu

DB_PATH="${DB_PATH:-/app/data/amigo_oculto_v2.db}"
DATA_DIR="$(dirname "$DB_PATH")"

# Volumes are mounted root-owned (Railway, docker -v). Take ownership once, then drop
# privileges for everything else.
if [ "$(id -u)" = "0" ]; then
  mkdir -p "$DATA_DIR"
  chown -R appuser:appuser "$DATA_DIR"
  exec setpriv --reuid=appuser --regid=appuser --init-groups "$0" "$@"
fi

if [ -n "${LITESTREAM_BUCKET:-}" ]; then
  # Each deployment replicates into its own prefix so a test service can never restore or
  # overwrite production data: LITESTREAM_PATH is mandatory alongside the bucket.
  : "${LITESTREAM_PATH:?LITESTREAM_PATH must be set when LITESTREAM_BUCKET is set}"
  # Fresh volume (or volume loss): pull the latest replica before serving anything.
  litestream restore -if-db-not-exists -if-replica-exists "$DB_PATH"
  exec litestream replicate -exec /app/amigo-oculto
fi

# Local development / CI smoke tests: no replication configured.
exec /app/amigo-oculto
