# syntax=docker/dockerfile:1

# ---- 1. Front-end (Svelte) ----
FROM node:24-slim AS frontend
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY web/ ./
RUN npm run build

# ---- 2. Rust binary (static, musl) ----
FROM rust:1-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /src
# Cache dependencies separately from sources.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && echo '' > src/lib.rs \
    && cargo build --release --locked \
    && rm -rf src
COPY src ./src
COPY migrations ./migrations
COPY --from=frontend /web/dist ./web/dist
RUN touch src/main.rs src/lib.rs && cargo build --release --locked \
    && mkdir -p /out/data && cp target/release/rustspeed /out/rustspeed

# ---- 3. Runtime ----
FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=builder /out/rustspeed /usr/local/bin/rustspeed
COPY --from=builder --chown=nonroot:nonroot /out/data /data
ENV RUSTSPEED_LISTEN_ADDR=0.0.0.0:8080 \
    RUSTSPEED_DATABASE_PATH=/data/rustspeed.sqlite3
VOLUME ["/data"]
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/rustspeed", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/rustspeed"]
