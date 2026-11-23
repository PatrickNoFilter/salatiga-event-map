#!/usr/bin/env bash
# Daily database backup. Add to crontab:
#   0 3 * * * /path/to/event-map/deploy/backup.sh >> /var/log/eventmap-backup.log 2>&1
set -euo pipefail

# Load credentials from .env (POSTGRES_USER, POSTGRES_PASSWORD, POSTGRES_DB)
ENV_FILE="$(dirname "$0")/../.env"
if [ -f "$ENV_FILE" ]; then
  set -a; source "$ENV_FILE"; set +a
fi

: "${POSTGRES_USER:=eventmap}"
: "${POSTGRES_DB:=eventmap}"
BACKUP_DIR="${BACKUP_DIR:-/var/backups/eventmap}"
RETENTION_DAYS="${RETENTION_DAYS:-14}"
STAMP="$(date +%Y%m%d-%H%M%S)"
mkdir -p "$BACKUP_DIR"

echo "[$(date)] Starting backup"
docker compose -f "$(dirname "$0")/../docker-compose.prod.yml" exec -T db \
  pg_dump -U "$POSTGRES_USER" "$POSTGRES_DB" | gzip > "$BACKUP_DIR/eventmap-$STAMP.sql.gz"
echo "[$(date)] Wrote $BACKUP_DIR/eventmap-$STAMP.sql.gz"

# Keep only the last N days of backups
find "$BACKUP_DIR" -name 'eventmap-*.sql.gz' -mtime "+$RETENTION_DAYS" -delete

# IMPORTANT: test a restore periodically with:
#   gunzip -c eventmap-<stamp>.sql.gz | docker compose exec -T db psql -U "$POSTGRES_USER" -d restore_test
echo "[$(date)] Backup complete"
