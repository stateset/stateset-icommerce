# StateSet iCommerce Docker Image
# Multi-stage build for optimized production image

# =============================================================================
# Stage 1: Build Stage
# =============================================================================
FROM rust:1.75-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace configuration
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build release binaries
RUN cargo build --release --workspace

# =============================================================================
# Stage 2: Runtime Stage
# =============================================================================
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN useradd -m -u 1000 -s /bin/bash stateset
USER stateset

WORKDIR /app

# Copy built artifacts from builder
COPY --from=builder /app/target/release/libstateset_embedded.so /app/lib/ 2>/dev/null || true
COPY --from=builder /app/target/release/libstateset_embedded.rlib /app/lib/ 2>/dev/null || true

# Create data directory
RUN mkdir -p /app/data

# Environment variables
ENV RUST_LOG=info
ENV DATABASE_PATH=/app/data/store.db

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD test -f /app/data/store.db || exit 1

# Default command (placeholder - real apps will override)
CMD ["echo", "StateSet iCommerce library ready"]

# =============================================================================
# Stage 3: CLI Image
# =============================================================================
FROM node:20-bookworm-slim AS cli

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash stateset
USER stateset

WORKDIR /app

# Copy CLI files
COPY --chown=stateset:stateset cli/package*.json ./
RUN npm ci --only=production

COPY --chown=stateset:stateset cli/ ./

# Create data directory
RUN mkdir -p /app/data

# Environment variables
ENV NODE_ENV=production
ENV DATABASE_PATH=/app/data/store.db

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD node -e "console.log('healthy')" || exit 1

# Default command
CMD ["node", "bin/stateset.js"]

# =============================================================================
# Stage 4: Python Bindings Image
# =============================================================================
FROM python:3.11-slim-bookworm AS python

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash stateset
USER stateset

WORKDIR /app

# Install Python package
RUN pip install --user stateset-embedded

# Create data directory
RUN mkdir -p /app/data

# Environment variables
ENV PYTHONUNBUFFERED=1
ENV DATABASE_PATH=/app/data/store.db

# Default command
CMD ["python", "-c", "import stateset_embedded; print('StateSet Python ready')"]
