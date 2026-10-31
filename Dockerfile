# syntax=docker/dockerfile:1.6

# Single-service image: Rust app + Litestream in one container.

ARG RUST_VERSION=1.90
ARG DEBIAN_SUITE=bookworm

############################
# Build (dependency layer first for cache reuse)
############################
FROM rust:${RUST_VERSION}-slim-${DEBIAN_SUITE} AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential pkg-config \
  && rm -rf /var/lib/apt/lists/*

ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

WORKDIR /app

# Parallel rustc processes can exceed ~1 GB RAM each on small builders; tune per host.
ARG BUILD_JOBS=8
ENV CARGO_BUILD_JOBS=${BUILD_JOBS}

# Dependencies only: manifests, the build script and a dummy binary. `static/` stays empty
# here so asset edits never invalidate this (slow) layer; build.rs re-runs when it is filled.
COPY Cargo.toml Cargo.lock build.rs ./
RUN mkdir -p src static && printf "fn main() {}\n" > src/main.rs
RUN cargo build --release --locked && rm -rf src

# Real sources (templates/, migrations/ and static/ are embedded at compile time).
COPY static/ ./static/
COPY templates/ ./templates
COPY migrations/ ./migrations
COPY src/ ./src
RUN touch src/main.rs src/lib.rs build.rs
RUN cargo build --release --locked

############################
# Runtime
############################
FROM debian:${DEBIAN_SUITE}-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl \
  && rm -rf /var/lib/apt/lists/*

# Litestream from the official image (0.5.x config format).
COPY --from=litestream/litestream:0.5 /usr/local/bin/litestream /usr/local/bin/litestream

WORKDIR /app

RUN useradd --system --uid 10001 --home /app appuser \
  && mkdir -p /app/data && chown -R appuser:appuser /app

COPY --from=builder /app/target/release/amigo-oculto /app/amigo-oculto
COPY litestream.yml /etc/litestream.yml
COPY run.sh /app/run.sh
RUN chmod +x /app/run.sh

# The container starts as root only to take ownership of the (root-mounted) volume;
# run.sh immediately re-executes itself as `appuser`.
ENV PORT=8080 \
    DATABASE_URL=sqlite:///app/data/amigo_oculto_v2.db \
    RUST_LOG=info
EXPOSE 8080
VOLUME ["/app/data"]

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -fsS "http://127.0.0.1:${PORT}/healthz" || exit 1

# run.sh restores from the latest Litestream replica when the volume is empty, then
# runs the app under `litestream replicate -exec`, so every write is shipped off-provider.
CMD ["/app/run.sh"]
