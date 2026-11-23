# Salatiga Event Map

**Live: https://salatiga-event-map.go.altare.global**

A city event map for **Salatiga, Jawa Tengah, Indonesia**. One Rust codebase renders as a web app (WASM) and an Android app (WebView), backed by an Axum REST API and a PostgreSQL + PostGIS database. The map canvas is MapLibre GL JS, called from Rust.

## Live deployment (Altare sandbox)

- Host: Altare sandbox `salatiga-event-map` (Tokyo jp-tyo01, Ubuntu 26.04, no Docker, no systemd).
- The web frontend is served by `deploy/serve.py` on the public port **25336** (Altare-assigned; the Altare HTTPS domain points at it), proxying `/api/*` to the Axum API on `127.0.0.1:8080`. Postgres + PostGIS runs locally in the sandbox.
- The frontend uses the page's own origin as its API base, so no rebuild is needed for the domain.
- Start/restart on the sandbox (secrets live in `~/.sem-env` on the box, never in git):
  ```bash
  cd ~/salatiga-event-map
  git pull
  cargo build --release -p api
  cd crates/app && dx build --release --platform web && cd ../..
  bash deploy/start.sh   # run inside tmux
  ```
- CI (`.github/workflows/ci.yml`) builds the API and the Dioxus WASM frontend on every push to `main`. Built WASM is never committed.

Built following [EVENT_MAP_BUILD_GUIDE.md](EVENT_MAP_BUILD_GUIDE.md).

## Stack

| Layer | Technology |
|---|---|
| Frontend | Dioxus 0.7 (web + Android WebView) |
| Map | MapLibre GL JS 4.7.1 (OSM raster tiles in dev) |
| API | Axum 0.8, sqlx 0.8, tower-http, tower_governor |
| Database | PostgreSQL 16 + PostGIS 3.4 |
| Deploy | Docker Compose + Caddy (auto HTTPS) |

City center: `lng 110.5076, lat -7.3307`. Categories: `music`, `food`, `arts`, `community`.

## Quick start (development)

```bash
# 1. Start the database
docker compose up -d

# 2. Apply migrations + seed 8 Salatiga sample events
export DATABASE_URL=postgres://eventmap:eventmap@localhost:5432/eventmap
docker compose exec -T db psql -U eventmap -d eventmap -f /tmp/0001_init.sql  # see note below
# Easier: copy migrations into the container and apply:
docker cp migrations/0001_init.sql event-map-db-1:/tmp/0001_init.sql
docker cp migrations/0002_seed.sql event-map-db-1:/tmp/0002_seed.sql
docker compose exec -T db psql -U eventmap -d eventmap -f /tmp/0001_init.sql
docker compose exec -T db psql -U eventmap -d eventmap -f /tmp/0002_seed.sql

# 3. Run the API (from crates/api so .env is picked up)
cd crates/api && DATABASE_URL=$DATABASE_URL ADMIN_TOKEN=dev-token cargo run -p api

# 4. Run the web app (from crates/app)
cd crates/app && dx serve --platform web
```

Open the URL printed by `dx serve`. The map centers on Salatiga with colored dots; zoom in to split clusters, click a dot for details.

> Migration note: for a real project use `sqlx migrate run --source migrations` (install `sqlx-cli` once). The manual `psql` commands above are a quick path for the demo; they are idempotent for `0001` but `0002` inserts duplicates if re-run.

## Features

- **Map** with clustering, category colors, OSM attribution, pan/zoom, and a "moveend" query that refetches only visible events (debounced 300 ms).
- **Detail card** with every event field, Directions (Google Maps), Tickets, and Share (copy link).
- **Filters**: All / Today / This weekend, category chips, Free only, plus map-area bounds. Times shown in WIB (UTC+7).
- **Submission form**: tap the map to place a pin; validates on the server; new events become `pending`.
- **Admin panel** (`⚙`): token kept in memory only; approve/reject pending events. Token compared in constant time.
- **Near me** (`◎`): browser/Android Geolocation API centers the map on the user.
- **Responsive**: sidebar list becomes a bottom sheet and the card slides up on phones; 44 px touch targets.

## API

| Method | Path | Purpose |
|---|---|---|
| GET | `/api/health` | Health check |
| GET | `/api/events` | Approved events; filters `west,south,east,north,category,free_only,from,to` |
| GET | `/api/events/{id}` | One approved event |
| POST | `/api/events` | Submit (becomes `pending`); rate-limited per IP |
| GET | `/api/admin/events?status=pending` | List by status (Bearer token) |
| POST | `/api/admin/events/{id}/approve` | Approve (Bearer token) |
| POST | `/api/admin/events/{id}/reject` | Reject (Bearer token) |
| POST | `/api/admin/events/{id}/cancel` | Cancel (Bearer token) |

## Testing

```bash
# With API running and ADMIN_TOKEN=test-token:
bash crates/api/tests/smoke.sh
```

## Android (Milestone 6)

Prerequisites: Android Studio SDK + emulator image + NDK, JDK 17, and Rust targets:

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
export ANDROID_HOME=/path/to/sdk ANDROID_NDK_HOME=/path/to/ndk
```

Build with an emulator-safe API base URL (the emulator cannot reach `localhost`):

```bash
cd crates/app
EVENT_API_BASE_URL=http://10.0.2.2:8080 dx serve --platform android
```

The manifest in `android/AndroidManifest.xml` declares location permission and cleartext traffic for dev. Use HTTPS on real devices. See `NOTES.md` and the guide's Milestone 6 for release signing and Play upload steps.

## Production (Milestone 8)

1. Copy `.env.example` to `.env`, set strong `POSTGRES_PASSWORD` and `ADMIN_TOKEN`.
2. Edit `deploy/Caddyfile` with your real domain and restrict CORS in `crates/api/src/main.rs`.
3. `dx build --release --platform web` and place output in `target/dx/app/release/web/public/`.
4. `docker compose -f docker-compose.prod.yml up -d --build`.
5. Apply migrations with `sqlx migrate run` and schedule `deploy/backup.sh` in cron. Test a restore once.

## Project layout

```
event-map/
├─ Cargo.toml                 # workspace: shared, api, app
├─ docker-compose.yml         # dev database
├─ docker-compose.prod.yml    # production stack
├─ .env.example
├─ NOTES.md
├─ migrations/                # 0001_init.sql, 0002_seed.sql
├─ android/AndroidManifest.xml
├─ deploy/                    # Caddyfile, api.Dockerfile, backup.sh
├─ crates/
│  ├─ shared/                 # Event / NewEvent / EventQuery types
│  ├─ api/                    # Axum server, .env, smoke tests
│  └─ app/                    # Dioxus web + Android
│     ├─ index.html           # loads pinned MapLibre GL JS
│     ├─ public/assets/       # map.js (bridge), style.css
│     └─ src/main.rs
```

## Attribution

Map data © [OpenStreetMap contributors](https://www.openstreetmap.org/copyright). OSM tiles are for development only — for launch, self-host Protomaps `.pmtiles` or use a keyed provider such as MapTiler.

## Tailscale persistence & phone transfer

This project's `.tailscale/` folder stores a **Tailscale login state** and helper
scripts so a fresh sandbox session can rejoin the tailnet without re-login:

```bash
bash .tailscale/restore.sh          # start tailscaled + SOCKS5 on 127.0.0.1:1055
PHONE_PASS='<termux-password>' .tailscale/phone-ssh.sh 'hostname'   # SSH to a33-samsung
```

The compiled app and full source were copied to the Samsung phone at
`/storage/emulated/0/Salatiga Event Map/` (web-app/ + source/ + BACA-PERTAMA.txt).
