FROM rust:1.93-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo build --release -p api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /build/target/release/api /usr/local/bin/api
EXPOSE 8080
CMD ["api"]
