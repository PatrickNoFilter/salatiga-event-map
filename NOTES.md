# Event Map Notes
Stack: Rust workspace (shared, api, app). Dioxus (web + Android), Axum, sqlx, Postgres+PostGIS, MapLibre GL JS.
City: Salatiga, Jawa Tengah, Indonesia | Map center: [110.5076, -7.3307] | Tiles: OpenStreetMap raster (dev) / swap to Protomaps or MapTiler for production
Versions: Rust 1.93, Dioxus 0.7.x, Axum 0.8, sqlx 0.8, MapLibre GL JS 4.x (pinned in index.html)

## Done
- [x] M1 database (PostGIS + 8 Salatiga seed events)
- [x] M2 API (GET /api/events with filters, GET /api/events/{id})
- [x] M3 web map + dots (MapLibre, clustering, category colors)
- [x] M4 detail card + filters (Today/weekend/category/Free + map-area, responsive)
- [x] M5 submit + admin (validation, rate limit, bearer token, constant-time compare)
- [x] M6 Android (config, manifest, cleartext, Geolocation Near me, EVENT_API_BASE_URL)
- [x] M7 polish (loading/empty/error states, WIB times, escaping via Dioxus, a11y focus)
- [x] M8 deploy (prod compose, Caddyfile, backup script, Dockerfile, smoke tests)

## Build status
- Rust 1.93, Dioxus 0.7.10, Axum 0.8, sqlx 0.8, MapLibre GL JS 4.7.1
- `cargo build -p api` OK; `dx build --platform web --release` OK
- API smoke tests: 7/7 passed
- Browser verified with Playwright: map renders Salatiga, 8 dots/clusters, zoom splits to colored dots, detail card opens, no console errors

## Decisions
- Categories: music, food, arts, community
- Coordinates: Salatiga city center ~ lat -7.3307, lng 110.5076 (decimal degrees, WGS84)
- Status flow: pending -> approved (visible) or rejected / cancelled
- Dev API: http://localhost:8080 ; Android emulator: http://10.0.2.2:8080
- Admin token read from ADMIN_TOKEN env var; kept in browser memory only
- Tiles during development use tile.openstreetmap.org (fine for dev/demo only). For launch, self-host Protomaps .pmtiles or use MapTiler with a key. Always show attribution.

## Current problem
(none - all milestones built and verified locally)

## Next step
Deploy: copy .env.example to .env, set strong secrets, edit deploy/Caddyfile domain, then docker compose -f docker-compose.prod.yml up -d --build. For Android, run `EVENT_API_BASE_URL=http://10.0.2.2:8080 dx serve --platform android` with the SDK/NDK/JDK installed.
