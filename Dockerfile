# Build stage
FROM rust:1.82-slim AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Create dummy main.rs to cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy source code
COPY src ./src

# Build for release
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/target/release/agentim /usr/local/bin/agentim

# Create non-root user
RUN useradd -r -s /bin/false agentim

# Create directories for state and config
RUN mkdir -p /app/state /app/config && \
    chown -R agentim:agentim /app

USER agentim

# Expose default port
EXPOSE 8080

# Set default config path
ENV AGENTIM_CONFIG=/app/config/config.json
ENV AGENTIM_STATE=/app/state/sessions.json

# Default command
CMD ["agentim"]
