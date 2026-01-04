# syntax=docker/dockerfile:1.6

ARG RUST_VERSION=1.92.0
ARG DEBIAN_SUITE=bookworm
ARG BIN_NAME=amigo-oculto-backend
ARG NODE_VERSION=20-bookworm-slim

############################
# Frontend build
############################
FROM node:${NODE_VERSION} AS fe_builder
WORKDIR /app/frontend

RUN corepack enable

# Copy only package files first for caching
COPY frontend/package.json frontend/pnpm-lock.yaml ./

RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile

# Now copy the rest and build
COPY frontend/ .
RUN pnpm run build

############################
# Build
############################
FROM rust:${RUST_VERSION}-slim-${DEBIAN_SUITE} AS be_builder
WORKDIR /app/backend

# Install only what is typically needed to build + run TLS clients.
# Add more (e.g., libssl-dev, sqlite3, clang) only if your build fails.
RUN apt-get update && apt-get install -y --no-install-recommends \
  ca-certificates pkg-config libssl-dev \
  && rm -rf /var/lib/apt/lists/*

# Faster, more reliable registry fetching in containers
ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

# 1) Copy manifests first for dependency caching
COPY backend/Cargo.toml backend/Cargo.lock ./

# 2) Dummy main to compile deps only (best cache reuse)
RUN mkdir -p src && printf "fn main() {}\n" > src/main.rs

# Build deps with caches (requires BuildKit, which Fly uses)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release --locked \
 && rm -rf src

# 3) Copy actual source and build final binary
COPY backend/src ./src

# Touch source files to ensure they're newer than the dummy-built binary
# (Docker COPY preserves mtimes, which can confuse cargo's incremental builds)
RUN touch src/main.rs

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release --locked

############################
# Runtime
############################
FROM debian:${DEBIAN_SUITE}-slim AS runtime

# Re-declare ARG to make it available in this stage (Docker ARGs have stage scope)
ARG BIN_NAME

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
  && rm -rf /var/lib/apt/lists/*

# Non-root user (Fly is happy with this; good default hardening)
# RUN useradd -r -u 10001 -g nogroup appuser

COPY --from=be_builder /app/backend/target/release/${BIN_NAME} /app/${BIN_NAME}
COPY --from=fe_builder /app/frontend/build /app/public

# Copy Litestream binary from official image (0.5.x config format)
COPY --from=litestream/litestream:0.5 /usr/local/bin/litestream /usr/local/bin/litestream

# Copy Litestream config and startup script
COPY litestream.yml /etc/litestream.yml
COPY run.sh /app/run.sh
RUN chmod +x /app/run.sh

# This is where your Fly volume should mount (or adjust to your fly.toml)
RUN mkdir -p /app/data #&& chown -R appuser:nogroup /app

# USER appuser
ENV PORT=3000
EXPOSE 3000

CMD ["/app/amigo-oculto-backend"]

