# ---------- Builder stage ----------
FROM 1.92-slim-bookworm AS builder

# Set the working directory inside the container
WORKDIR /app

# Install build dependencies (needed for linking, e.g., standard C libraries)
# If you use crates requiring OpenSSL, you might need 'pkg-config libssl-dev' here.
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \ 
    && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests first (for cache efficiency)
COPY Cargo.toml Cargo.lock ./
COPY shared/Cargo.toml shared/Cargo.toml
COPY services/users-api/Cargo.toml services/users-api/Cargo.toml

# Dummy build to cache deps
RUN mkdir -p services/users-api/src \
    && echo "fn main() {}" > services/users-api/src/main.rs \
    && cargo build --release --bin users-api

# Copy actual source
COPY shared shared
COPY services/users-api services/users-api

# Build real binary
RUN cargo build --release --bin users-api

# ---------- Runtime stage ----------
FROM debian:bookworm-slim
FROM gcr.io/distroless/cc-debian13

# Create non-root user
RUN useradd -m -u 10001 appuser

WORKDIR /app

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/users-api /app/users-api

# Permissions
RUN chown appuser:appuser /app/users-api

USER appuser

EXPOSE 8000

ENTRYPOINT ["/app/users-api"]
