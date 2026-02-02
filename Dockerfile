# ═══════════════════════════════════════════════════════════════════════════════
# Ada N8N Orchestrator - Rust Build
# ═══════════════════════════════════════════════════════════════════════════════
# Multi-stage build for minimal production image

# ─────────────────────────────────────────────────────────────────────────────
# Stage 1: Build
# ─────────────────────────────────────────────────────────────────────────────
FROM rust:1.75-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first for layer caching
COPY Cargo.toml Cargo.lock* ./

# Create dummy src to build dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source
COPY src ./src

# Build the real application
RUN touch src/main.rs && cargo build --release

# ─────────────────────────────────────────────────────────────────────────────
# Stage 2: Runtime
# ─────────────────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/ada-n8n /app/ada-n8n

# Copy workflow definitions (for reference/config)
COPY workflows /app/workflows

# ─────────────────────────────────────────────────────────────────────────────
# Environment Configuration
# ─────────────────────────────────────────────────────────────────────────────

# Server config (Railway port)
ENV N8N_PORT=8080
ENV N8N_HOST=0.0.0.0
ENV N8N_PROTOCOL=https
ENV GENERIC_TIMEZONE=Europe/Berlin

# Service endpoints (set in Railway)
# ENV ADA_MCP_URL=https://mcp.exo.red
# ENV ADA_POINT_URL=https://point.exo.red
# ENV UPSTASH_REDIS_REST_URL=...
# ENV UPSTASH_REDIS_REST_TOKEN=...
# ENV ADA_XAI_KEY=...

# Logging
ENV RUST_LOG=ada_n8n=info,tower_http=info

EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/healthz || exit 1

# Run
CMD ["/app/ada-n8n"]
