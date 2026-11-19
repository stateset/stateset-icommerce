FROM node:20-slim AS base

# Install dependencies only when needed
FROM base AS deps
RUN apt-get update && apt-get install -y python3 make g++ && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# Install dependencies based on the preferred package manager
COPY package.json package-lock.json* ./
RUN npm ci --include=optional

# Rebuild the source code only when needed
FROM base AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY . .

RUN npm run build

# Both long-running services use the same SQLite directory and process user.
FROM base AS runtime
WORKDIR /app
ENV NODE_ENV=production
ENV STATESET_DB_PATH=/var/lib/stateset/store.db

RUN addgroup --system --gid 1001 nodejs \
 && adduser --system --uid 1001 --ingroup nodejs nextjs \
 && install -d -m 0750 -o nextjs -g nodejs /var/lib/stateset

# Operator commands use SDKs that the web server can bundle into compiled routes.
FROM deps AS operator-deps
RUN npm prune --omit=dev --offline --no-audit --no-fund

FROM runtime AS operator
COPY --from=operator-deps /app/node_modules ./node_modules
COPY package.json ./
COPY scripts ./scripts
COPY lib ./lib
USER nextjs
CMD ["node", "scripts/merchant-worker.js"]

FROM runtime AS runner
COPY --from=builder /app/public ./public

# Set the correct permission for prerender cache
RUN mkdir .next
RUN chown nextjs:nodejs .next

# Automatically leverage output traces to reduce image size
COPY --from=builder --chown=nextjs:nodejs /app/.next/standalone ./
COPY --from=builder --chown=nextjs:nodejs /app/.next/static ./.next/static

# Mount a named volume at /var/lib/stateset for the database and its WAL files.

USER nextjs

EXPOSE 3000

ENV PORT=3000
ENV HOSTNAME="0.0.0.0"

CMD ["node", "server.js"]
