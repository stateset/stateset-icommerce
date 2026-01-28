# StateSet iCommerce Channel Gateway
# Multi-stage Node.js 20 build

# --- Stage 1: Build ---
FROM node:20-alpine AS builder
WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci --omit=dev

# --- Stage 2: Production ---
FROM node:20-slim
WORKDIR /app

ENV NODE_ENV=production

# Create non-root user
RUN groupadd --gid 1001 stateset && \
    useradd --uid 1001 --gid stateset --shell /bin/sh --create-home stateset

# Copy built node_modules and app files
COPY --from=builder /app/node_modules ./node_modules
COPY package.json ./
COPY src/ ./src/
COPY bin/ ./bin/

# Create data directory
RUN mkdir -p /app/data && chown stateset:stateset /app/data

USER stateset

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD node -e "const http = require('http'); http.get('http://localhost:8080/health', (r) => { process.exit(r.statusCode === 200 ? 0 : 1); }).on('error', () => process.exit(1));"

CMD ["node", "bin/stateset-channels.js"]
