#!/bin/sh
set -e

# Restore database from backup if it doesn't exist locally
# (e.g., after volume loss or fresh deployment)
litestream restore -if-db-not-exists -if-replica-exists /app/data/amigo_oculto.db

# Run backend wrapped by Litestream for continuous replication
exec litestream replicate -exec /app/amigo-oculto-backend
