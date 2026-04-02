ARG RUST_VERSION=1.85.0

# Build stage
FROM rust:${RUST_VERSION}-slim AS builder

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
COPY start.sh /usr/local/bin/agentim-start
RUN chmod 755 /usr/local/bin/agentim-start

# Create non-root user
RUN useradd -r -s /bin/false agentim

# Create directories for state and config
RUN mkdir -p /app/state /app/config && \
    chown -R agentim:agentim /app

USER agentim

# Expose default port
EXPOSE 8080

# Runtime wrapper uses the installed binary instead of repo-local build artifacts.
ENV AGENTIM_BINARY=/usr/local/bin/agentim
ENV AGENTIM_STATE_FILE=/app/state/sessions.json

ENTRYPOINT ["/usr/local/bin/agentim-start"]
