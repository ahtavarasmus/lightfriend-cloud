# syntax=docker/dockerfile:1.4
#
# Lightfriend Enclave Dockerfile
# Multi-stage build for AWS Nitro Enclave deployment
# Contains: Lightfriend Core + 5 mautrix bridges + PostgreSQL + s6-overlay
#
# Build modes:
#   Source (default): Builds Core from Rust source
#     docker build -f docker/enclave/Dockerfile .
#
#   Prebuilt (fast): Uses pre-built image from Docker Hub
#     docker build -f docker/enclave/Dockerfile \
#       --build-arg CORE_BINARY_SOURCE=prebuilt \
#       --build-arg CORE_IMAGE=ahtavarasmus/lightfriend-core:latest .
#

# Build configuration
ARG CORE_BINARY_SOURCE=build
ARG CORE_IMAGE=ahtavarasmus/lightfriend-core:latest

# =============================================================================
# STAGE 1: Build Lightfriend Core from source (default)
# =============================================================================
FROM rust:1.86-bookworm AS backend-builder

# Install cargo-chef and sccache for faster builds
RUN cargo install cargo-chef sccache --locked

# Install diesel_cli with sqlite support
RUN cargo install diesel_cli --no-default-features --features sqlite --locked

# Configure sccache
ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache
ENV SCCACHE_DIR=/sccache
ENV SCCACHE_CACHE_SIZE="2G"

WORKDIR /app

# Stage 1a: Generate recipe file for dependencies
FROM backend-builder AS planner
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src
RUN cargo chef prepare --recipe-path recipe.json

# Stage 1b: Build dependencies (cached unless dependencies change)
FROM backend-builder AS cacher
COPY --from=planner /app/recipe.json recipe.json
RUN apt-get update && apt-get install -y \
    libsqlite3-dev \
    pkg-config \
    libssl-dev \
    mold \
    clang \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache
ENV SCCACHE_DIR=/sccache

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/sccache \
    cargo chef cook --release --recipe-path recipe.json && \
    sccache --show-stats

# Stage 1c: Build application
FROM backend-builder AS builder
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
RUN apt-get update && apt-get install -y \
    libsqlite3-dev \
    pkg-config \
    libssl-dev \
    mold \
    clang \
    && rm -rf /var/lib/apt/lists/*
COPY backend /app
WORKDIR /app

ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache
ENV SCCACHE_DIR=/sccache
ENV RUSTFLAGS="-C link-arg=-fuse-ld=mold -C linker=clang"

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/sccache \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    sccache --show-stats && \
    cp target/release/backend /backend && \
    strip /backend

# =============================================================================
# STAGE 1b: Pre-built Core alternative (optional)
# =============================================================================
FROM ${CORE_IMAGE} AS prebuilt-core

# =============================================================================
# STAGE 1c: Normalize binary location (unified interface)
# =============================================================================
# Create intermediate stages that expose the binary at /binary
# This allows the final stage to copy from either source consistently

FROM alpine:latest AS extractor-build
COPY --from=builder /backend /binary
COPY --from=builder /usr/local/cargo/bin/diesel /diesel

FROM alpine:latest AS extractor-prebuilt
COPY --from=prebuilt-core /app/backend /binary
COPY --from=prebuilt-core /app/diesel /diesel

# Select which extractor to use based on CORE_BINARY_SOURCE arg
# Resolves to either "extractor-build" or "extractor-prebuilt"
FROM extractor-${CORE_BINARY_SOURCE} AS core-binary

# =============================================================================
# STAGE 2: Extract bridge binaries from official mautrix images
# =============================================================================
FROM dock.mau.dev/mautrix/whatsapp:latest AS whatsapp-bridge
FROM dock.mau.dev/mautrix/signal:latest AS signal-bridge
FROM dock.mau.dev/mautrix/meta:latest AS meta-bridge
FROM dock.mau.dev/mautrix/telegram:latest AS telegram-bridge

# =============================================================================
# STAGE 3: Runtime - debian:bookworm-slim + s6-overlay + PostgreSQL
# =============================================================================
FROM debian:bookworm-slim AS runtime

# Install xz-utils first (needed to extract s6-overlay)
RUN apt-get update && apt-get install -y xz-utils && rm -rf /var/lib/apt/lists/*

# Install s6-overlay
ARG S6_OVERLAY_VERSION=3.1.6.2
ADD https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-noarch.tar.xz /tmp
ADD https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-x86_64.tar.xz /tmp
RUN tar -C / -Jxpf /tmp/s6-overlay-noarch.tar.xz && \
    tar -C / -Jxpf /tmp/s6-overlay-x86_64.tar.xz && \
    rm /tmp/s6-overlay-*.tar.xz

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    # Core runtime
    ca-certificates \
    libsqlite3-0 \
    libssl3 \
    curl \
    # PostgreSQL
    postgresql-15 \
    postgresql-client-15 \
    # Bridge dependencies (mautrix-telegram needs Python)
    python3 \
    python3-pip \
    python3-venv \
    # For VSOCK proxy
    socat \
    # Utilities
    jq \
    gettext-base \
    && rm -rf /var/lib/apt/lists/*

# Install Python dependencies for Telegram bridge
# mautrix-telegram is Python-based (unlike other bridges which are Go binaries)
RUN pip3 install --break-system-packages \
    PySocks \
    mautrix-telegram

# Create directories
RUN mkdir -p \
    /app/data \
    /app/uploads \
    /app/matrix_store \
    /data/bridges/whatsapp \
    /data/bridges/signal \
    /data/bridges/messenger \
    /data/bridges/instagram \
    /data/bridges/telegram \
    /data/postgresql \
    /run/postgresql \
    /var/lib/postgresql/15/main

# Copy Lightfriend Core binary and diesel (from either build or prebuilt source)
COPY --from=core-binary /binary /app/backend
COPY --from=core-binary /diesel /app/diesel

# Copy migrations
COPY backend/migrations /app/migrations
COPY backend/diesel.toml /app/

# Copy bridge binaries
COPY --from=whatsapp-bridge /usr/bin/mautrix-whatsapp /usr/bin/mautrix-whatsapp
COPY --from=signal-bridge /usr/bin/mautrix-signal /usr/bin/mautrix-signal
COPY --from=meta-bridge /usr/bin/mautrix-meta /usr/bin/mautrix-meta

# Copy Telegram bridge (Python-based, needs packages + wrapper)
COPY --from=telegram-bridge /usr/lib/python3.12/site-packages/mautrix /usr/lib/python3/dist-packages/mautrix
COPY --from=telegram-bridge /usr/lib/python3.12/site-packages/mautrix_telegram /usr/lib/python3/dist-packages/mautrix_telegram
COPY --from=telegram-bridge /usr/lib/python3.12/site-packages/telethon /usr/lib/python3/dist-packages/telethon

# Create wrapper script to make telegram bridge look like a binary
RUN echo '#!/bin/sh\nexec python3 -m mautrix_telegram "$@"' > /usr/bin/mautrix-telegram && \
    chmod +x /usr/bin/mautrix-telegram

# Copy s6 service definitions
COPY docker/enclave/s6-rc.d /etc/s6-overlay/s6-rc.d

# Copy configuration templates
COPY docker/enclave/config /etc/lightfriend/config

# Copy helper scripts
COPY docker/enclave/scripts /usr/local/bin/
RUN chmod +x /usr/local/bin/*.sh

# Set PostgreSQL permissions
RUN chown -R postgres:postgres /var/lib/postgresql /run/postgresql /data/postgresql

# Environment variables for VSOCK ports
ENV VSOCK_SYNAPSE_PORT=5000
ENV VSOCK_INTERNET_PORT=5001
ENV VSOCK_STATE_PORT=5002
ENV VSOCK_ATTESTATION_PORT=5003

# s6-overlay configuration
ENV S6_KEEP_ENV=1
ENV S6_BEHAVIOUR_IF_STAGE2_FAILS=2
ENV S6_CMD_WAIT_FOR_SERVICES_MAXTIME=60000

WORKDIR /app

# Expose internal ports (these are proxied via VSOCK in enclave mode)
EXPOSE 3000 5432

# s6-overlay entrypoint
ENTRYPOINT ["/init"]
