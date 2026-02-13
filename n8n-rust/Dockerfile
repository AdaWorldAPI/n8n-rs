# =============================================================================
# n8n-rust Multi-Transport Server
# =============================================================================
# Arrow 57 | DataFusion 51 | Lance 2.0 | Tonic 0.14 | Rust 1.93
# With ladybug-rs vendor import for CAM operations
# =============================================================================

# ---------------------------------------------------------------------------
# Stage 1: Build
# ---------------------------------------------------------------------------
FROM rust:1.93-bookworm AS builder

# Install system dependencies for Arrow, protobuf, and native builds
RUN apt-get update && apt-get install -y --no-install-recommends \
    protobuf-compiler \
    libprotobuf-dev \
    cmake \
    pkg-config \
    libssl-dev \
    git \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# -- Vendor dependencies (ladybug-rs) --
# Clone ladybug-rs for CAM operations (4096 content-addressable methods)
RUN git clone --depth 1 https://github.com/AdaWorldAPI/ladybug-rs.git /build/vendor/ladybug-rs

# -- Copy workspace manifests first for dependency caching --
COPY Cargo.toml Cargo.lock ./
COPY crates/n8n-workflow/Cargo.toml crates/n8n-workflow/Cargo.toml
COPY crates/n8n-core/Cargo.toml crates/n8n-core/Cargo.toml
COPY crates/n8n-arrow/Cargo.toml crates/n8n-arrow/Cargo.toml
COPY crates/n8n-grpc/Cargo.toml crates/n8n-grpc/Cargo.toml
COPY crates/n8n-hamming/Cargo.toml crates/n8n-hamming/Cargo.toml
COPY crates/n8n-db/Cargo.toml crates/n8n-db/Cargo.toml
COPY crates/n8n-server/Cargo.toml crates/n8n-server/Cargo.toml

# Create dummy source files to cache dependency builds
RUN mkdir -p crates/n8n-workflow/src && echo "fn main() {}" > crates/n8n-workflow/src/lib.rs && \
    mkdir -p crates/n8n-core/src && echo "fn main() {}" > crates/n8n-core/src/lib.rs && \
    mkdir -p crates/n8n-arrow/src && echo "fn main() {}" > crates/n8n-arrow/src/lib.rs && \
    mkdir -p crates/n8n-grpc/src && echo "fn main() {}" > crates/n8n-grpc/src/lib.rs && \
    mkdir -p crates/n8n-grpc/proto && touch crates/n8n-grpc/proto/n8n.proto && \
    mkdir -p crates/n8n-grpc/src/generated && touch crates/n8n-grpc/src/generated/mod.rs && \
    mkdir -p crates/n8n-hamming/src && echo "fn main() {}" > crates/n8n-hamming/src/lib.rs && \
    mkdir -p crates/n8n-db/src && echo "fn main() {}" > crates/n8n-db/src/lib.rs && \
    mkdir -p crates/n8n-server/src && echo "fn main() {}" > crates/n8n-server/src/main.rs

# Build dependencies only (cached layer)
RUN cargo build --release 2>/dev/null || true

# -- Copy actual source code --
COPY crates/ crates/

# Build the release binary
RUN cargo build --release --bin n8n-server

# ---------------------------------------------------------------------------
# Stage 2: Runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    tini \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r n8n && useradd -r -g n8n -d /home/n8n -s /bin/bash n8n && \
    mkdir -p /home/n8n && chown -R n8n:n8n /home/n8n

WORKDIR /home/n8n

# Copy the built binary
COPY --from=builder /build/target/release/n8n-server /usr/local/bin/n8n-server

# Copy vendor libs for runtime reference
COPY --from=builder /build/vendor/ladybug-rs/README.md /usr/share/doc/ladybug-rs/

# Default environment
ENV RUST_LOG=info
ENV N8N_REST_ENABLED=true
ENV N8N_REST_ADDR=0.0.0.0:8080
ENV N8N_GRPC_ENABLED=true
ENV N8N_GRPC_ADDR=0.0.0.0:50051
ENV N8N_FLIGHT_ENABLED=true
ENV N8N_FLIGHT_ADDR=0.0.0.0:50052
ENV N8N_STDIO_ENABLED=false

# Expose ports: REST, gRPC, Arrow Flight
EXPOSE 8080/tcp
EXPOSE 50051/tcp
EXPOSE 50052/tcp

USER n8n

ENTRYPOINT ["tini", "--"]
CMD ["n8n-server"]

# Labels
LABEL org.opencontainers.image.title="n8n-rust" \
      org.opencontainers.image.description="n8n Workflow Automation - Rust Multi-Transport Server" \
      org.opencontainers.image.source="https://github.com/AdaWorldAPI/n8n-rs" \
      org.opencontainers.image.vendor="AdaWorld API" \
      n8n.rust.arrow="57" \
      n8n.rust.datafusion="51" \
      n8n.rust.lance="2.0" \
      n8n.rust.tonic="0.14" \
      n8n.rust.version="1.93" \
      n8n.rust.vendor.ladybug="github.com/AdaWorldAPI/ladybug-rs"
