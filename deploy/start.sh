#!/usr/bin/env bash
# Start the Salatiga Event Map stack on the Altare sandbox (no Docker, no systemd).
# Secrets come from $HOME/.sem-env (NOT in git — create it on the sandbox):
#   DATABASE_URL=postgres://eventmap:<password>@127.0.0.1:5432/eventmap
#   ADMIN_TOKEN=<long random token>
# Usage: bash deploy/start.sh   (run inside tmux so an SSH drop does not kill it)
set -euo pipefail
cd "$(dirname "$0")/.."

if [ -f "$HOME/.sem-env" ]; then
  set -a; . "$HOME/.sem-env"; set +a
fi
: "${DATABASE_URL:?DATABASE_URL must be set (see \$HOME/.sem-env)}"
: "${ADMIN_TOKEN:?ADMIN_TOKEN must be set (see \$HOME/.sem-env)}"

# PostgreSQL (local PostGIS cluster; version depends on what apt installed)
pg_ctlcluster 18 main start 2>/dev/null || pg_ctlcluster 16 main start 2>/dev/null || true

# Find the Dioxus web build output (dx writes under the app crate's target/)
ROOT=""
for d in crates/app/target/dx/app/release/web/public target/dx/app/release/web/public; do
  if [ -f "$d/index.html" ]; then ROOT="$PWD/$d"; break; fi
done
[ -n "$ROOT" ] || { echo "ERROR: no dx build output found (run: cd crates/app && dx build --release --platform web)"; exit 1; }

# API on 8080 (internal only), static server on the Altare public port 25336
DATABASE_URL="$DATABASE_URL" ADMIN_TOKEN="$ADMIN_TOKEN" \
  nohup ./target/release/api > /tmp/api.log 2>&1 &
SEM_ROOT="$ROOT" SEM_PORT=25336 \
  nohup python3 deploy/serve.py > /tmp/serve.log 2>&1 &

sleep 2
echo "--- api health ---"
curl -s http://127.0.0.1:8080/api/health; echo
echo "--- web root ---"
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:25336/
echo "--- events ---"
curl -s http://127.0.0.1:25336/api/events | head -c 300; echo
echo "Logs: /tmp/api.log /tmp/serve.log"
