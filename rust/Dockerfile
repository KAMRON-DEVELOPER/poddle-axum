# ------------------------------------------------------
# Stage 1: Chef (Base Image + Tooling)
# ------------------------------------------------------
FROM 1.92-slim-bookworm AS chef
WORKDIR /app
# We only pay the installation cost once,
# it will be cached from the second build onwards
RUN cargo install cargo-chef

# ------------------------------------------------------
# Stage 2: Planner (Workspace Skeleton)
# ------------------------------------------------------
FROM chef AS planner
WORKDIR /app
# Copy only the files needed to calculate dependencies
COPY Cargo.toml Cargo.lock ./
COPY shared/Cargo.toml shared/Cargo.toml
COPY crates/*/Cargo.toml crates/*/Cargo.toml
COPY services/*/Cargo.toml services/*/Cargo.toml
# This prepares the dependency recipe for the whole workspace
RUN cargo chef prepare --recipe-path recipe.json

# ------------------------------------------------------
# Stage 3: Cacher (Dependency Pre-build)
# ------------------------------------------------------
FROM chef AS cacher
WORKDIR /app
# System dependencies for compilation
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY --from=planner /app/recipe.json recipe.json
# Build *only dependencies*; cached until manifests change
RUN cargo chef cook --release --recipe-path recipe.json

# ------------------------------------------------------
# Stage 4: Builder (Application Build)
# ------------------------------------------------------
FROM chef AS builder
WORKDIR /app
ARG SERVICE_NAME
# Fail the build if no service name is provided
RUN test -n "$SERVICE_NAME"

# Copy cached dependencies from cacher
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo

# Copy full source *after* deps are cached
COPY . .

# Build the specific service binary
RUN cargo build --release --bin ${SERVICE_NAME}

# ------------------------------------------------------
# Stage 5: Runtime (Minimal Distroless)
# ------------------------------------------------------
FROM gcr.io/distroless/cc-debian13 AS runtime
WORKDIR /app

# Re-declare ARGs for runtime context
ARG SERVICE_NAME
ARG PORT=8000

# Copy the compiled binary
COPY --from=builder /app/target/release/${SERVICE_NAME} /app/server

EXPOSE ${PORT}

# Distroless runs as non-root (uid 65532) by default
USER nonroot:nonroot

ENTRYPOINT ["/app/server"]
