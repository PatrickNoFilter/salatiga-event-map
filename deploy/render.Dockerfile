# Render free-tier image: Axum API + Dioxus web bundle in one service.
# Mirrors the Altare layout: API on 127.0.0.1:8080, serve.py on $PORT
# serving the static site and proxying /api/* to the API.
FROM rust:1.96-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo install dioxus-cli --version 0.7.10
RUN cargo build --release -p api
RUN dx build --platform web --package app --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 python3 \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /build/target/release/api /usr/local/bin/api
COPY --from=builder /build/target/dx/app/release/web/public /srv/web
COPY deploy/serve.py /app/serve.py
COPY deploy/render-start.sh /app/render-start.sh
RUN chmod +x /app/render-start.sh
CMD ["/app/render-start.sh"]
