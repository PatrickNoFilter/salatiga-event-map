#!/usr/bin/env bash
# Start the stack on Render: API on 127.0.0.1:8080, serve.py on $PORT.
# Render injects PORT; serve.py reads it via SEM_PORT.
set -euo pipefail
/usr/local/bin/api &
SEM_ROOT=/srv/web SEM_PORT="${PORT:-10000}" exec python3 /app/serve.py
