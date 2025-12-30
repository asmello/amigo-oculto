# syntax=docker/dockerfile:1.6

ARG RUST_VERSION=1.92.0
ARG DEBIAN_SUITE=bookworm
ARG BIN_NAME=amigo-oculto-backend

############################
# Build
############################
FROM rust:${RUST_VERSION}-slim-${DEBIAN_SUITE} AS builder
WORKDIR /app

# Install only what is typically needed to build + run TLS clients.
# Add more (e.g., libssl-dev, sqlite3, clang) only if your build fails.
RUN apt-get update && apt-get install -y --no-install-recommends \
  ca-certificates pkg-config libssl-dev \
  && rm -rf /var/lib/apt/lists/*

# Faster, more reliable registry fetching in containers
ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

# 1) Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./

# 2) Dummy main to compile deps only (best cache reuse)
RUN mkdir -p src && printf "fn main() {}\n" > src/main.rs

# Build deps with caches (requires BuildKit, which Fly uses)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release --locked \
 && rm -rf src

# 3) Copy actual source and build final binary
COPY src ./src

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release --locked

############################
# Runtime
############################
FROM debian:${DEBIAN_SUITE}-slim AS runtime
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
  && rm -rf /var/lib/apt/lists/*

# Non-root user (Fly is happy with this; good default hardening)
RUN useradd -r -u 10001 -g nogroup appuser

COPY --from=builder /app/target/release/${BIN_NAME} /app/${BIN_NAME}

# This is where your Fly volume should mount (or adjust to your fly.toml)
RUN mkdir -p /app/data && chown -R appuser:nogroup /app

USER appuser
ENV PORT=3000
EXPOSE 3000

CMD ["/app/amigo-oculto-backend"]

