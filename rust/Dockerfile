# ------------------------------------------------------
# Stage 1: Chef (Base Image + Tooling)
# ------------------------------------------------------
FROM rust:1.92.0-slim-bookworm AS chef
WORKDIR /app
# We only pay the installation cost once,
# it will be cached from the second build onwards
RUN cargo install cargo-chef

# System dependencies for compilation
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# ------------------------------------------------------
# Stage 2: Planner (Workspace Skeleton)
# ------------------------------------------------------
FROM chef AS planner
WORKDIR /app
# Copy only the files needed to calculate dependencies
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY services/ services/
# This prepares the dependency recipe for the whole workspace
RUN cargo chef prepare --recipe-path recipe.json

# ------------------------------------------------------
# Stage 3: Cacher (Dependency Pre-build)
# ------------------------------------------------------
FROM chef AS cacher
WORKDIR /app

COPY --from=planner /app/recipe.json recipe.json
# Build *only dependencies*; cached until manifests change
RUN cargo chef cook --release --recipe-path recipe.json

# ------------------------------------------------------
# Stage 4: Builder (Application Build)
# ------------------------------------------------------
FROM chef AS builder
WORKDIR /app

ARG SERVICE_NAME
# Fail the build if no SERVICE_NAME is provided
RUN test -n "$SERVICE_NAME" || (echo "⚠️ SERVICE_NAME argument not set" && false)

# Copy cached dependencies
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo

# Copy the actual source code, all services beacuse of cargo workspace...
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY services/ services/

# sqlx offline mode stuffs
COPY .sqlx .sqlx

ENV SQLX_OFFLINE=true

# Build the specific binary
# At this point, CARGO_MANIFEST_DIR is baked as "/app/services/${SERVICE_NAME}"
RUN cargo build --release --bin ${SERVICE_NAME}

# ------------------------------------------------------
# Stage 5: Runtime (Minimal Distroless)
# ------------------------------------------------------
FROM gcr.io/distroless/cc-debian13 AS runtime

# Re-declare ARGs for runtime context
ARG SERVICE_NAME

# This matches the "CARGO_MANIFEST_DIR" path baked into the binary
WORKDIR /app/services/${SERVICE_NAME}

# Copy the binary
COPY --from=builder /app/target/release/${SERVICE_NAME} ./server

ENV PORT=8000
EXPOSE ${PORT}

# Distroless runs as non-root (uid 65532) by default
USER nonroot:nonroot

ENTRYPOINT ["./server"]

# In a Distroless images there is no shell (/bin/sh or /bin/bash).
# Variable expansion like $SERVICE_NAME will not work inside the ENTRYPOINT brackets
# ENV SERVICE_NAME=$SERVICE_NAME
# ENTRYPOINT ["/services", "$SERVICE_NAME"]
