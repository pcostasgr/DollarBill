# ─────────────────────────────────────────────────────────────────────────────
# Stage 1 — Builder
# Uses the official Rust image.  We compile in release mode so the final
# binaries are small and fast.
# ─────────────────────────────────────────────────────────────────────────────
FROM rust:1.82-slim-bookworm AS builder

# Build-time dependencies: pkg-config + OpenSSL headers (needed by reqwest /
# lettre native-tls), plus sqlite3 dev headers (sqlx).
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# ── Dependency caching layer ──────────────────────────────────────────────────
# Copy manifests first so this layer is only invalidated when deps change.
COPY Cargo.toml Cargo.lock ./

# Create stub src so `cargo build` can resolve the dependency graph without
# compiling application code.
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && mkdir -p src/bin && echo "fn main() {}" > src/bin/dashboard.rs

RUN cargo build --release --bins 2>/dev/null || true
# Remove stubs so the real sources replace them below.
RUN rm -rf src

# ── Full build ────────────────────────────────────────────────────────────────
COPY src       ./src
COPY benches   ./benches
COPY examples  ./examples
COPY tests     ./tests

RUN cargo build --release --bins

# ─────────────────────────────────────────────────────────────────────────────
# Stage 2 — Runtime image
# Slim Debian base; only the compiled binaries + runtime libs are copied in.
# ─────────────────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# Runtime deps: CA certificates (HTTPS), OpenSSL, sqlite3.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

# Non-root user for security.
RUN useradd --uid 1001 --create-home --shell /bin/bash dollarbill

WORKDIR /app

# ── Binaries ──────────────────────────────────────────────────────────────────
COPY --from=builder /build/target/release/dollarbill  /usr/local/bin/dollarbill
COPY --from=builder /build/target/release/dashboard   /usr/local/bin/dashboard

# ── Runtime directories ───────────────────────────────────────────────────────
# config/ and data/ are mounted from the host via docker-compose volumes so
# that market data, heston params, and the SQLite DB persist across restarts.
# We still create them here so the container is runnable solo for quick tests.
RUN mkdir -p /app/config /app/data \
    && chown -R dollarbill:dollarbill /app

# ── Static config defaults shipped with the image ─────────────────────────────
COPY config/ /app/config/

USER dollarbill

# ── Environment defaults (override in docker-compose or at runtime) ───────────
# Credentials must be injected at runtime — never bake them into the image.
ENV RUST_LOG=info
ENV DOLLARBILL_CONFIG_DIR=/app/config
ENV DOLLARBILL_DATA_DIR=/app/data

VOLUME ["/app/data"]

# Default: run the trading bot in live mode.
# Override CMD to run `dashboard` or `dollarbill --help`.
CMD ["dollarbill", "trade", "--live"]
