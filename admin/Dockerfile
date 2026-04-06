# StateSet Admin Dashboard Dockerfile
# Multi-stage build for optimal image size

# Stage 1: Dependencies
FROM node:20-alpine@sha256:b5b9467fe7b33aad47f1ec3f6e0646a658f85f05c18b0e5b49ea5a5d7f3b7e4a AS deps
RUN apk add --no-cache libc6-compat
WORKDIR /app

# Copy lock file first for better layer caching
COPY package.json package-lock.json* ./

# Use npm ci for reproducible builds
RUN npm ci --ignore-scripts

# Stage 2: Builder
FROM node:20-alpine@sha256:b5b9467fe7b33aad47f1ec3f6e0646a658f85f05c18b0e5b49ea5a5d7f3b7e4a AS builder
WORKDIR /app

# Copy dependencies
COPY --from=deps /app/node_modules ./node_modules
COPY . .

# Set build-time environment variables
ENV NEXT_TELEMETRY_DISABLED=1
ENV NODE_ENV=production

# Build the application
RUN npm run build

# Stage 3: Runner
FROM node:20-alpine@sha256:b5b9467fe7b33aad47f1ec3f6e0646a658f85f05c18b0e5b49ea5a5d7f3b7e4a AS runner
WORKDIR /app

ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1

# Create non-root user
RUN addgroup --system --gid 1001 nodejs && \
    adduser --system --uid 1001 nextjs

# Copy built assets
COPY --from=builder /app/public ./public
COPY --from=builder /app/.next/standalone ./
COPY --from=builder /app/.next/static ./.next/static

# Create writable directories for read-only root filesystem
RUN mkdir -p /app/.next/cache /tmp && \
    chown -R nextjs:nodejs /app /tmp

USER nextjs

EXPOSE 3000

ENV PORT=3000
ENV HOSTNAME="0.0.0.0"

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:3000/api/health || exit 1

CMD ["node", "server.js"]
