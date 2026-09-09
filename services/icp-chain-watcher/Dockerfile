# icp-chain-watcher image.
#
# Mirrors icp-docker/Dockerfile: node:20-slim, non-root, zero npm install —
# the watcher's only dependencies are `fetch` and hand-rolled ABI decoding.
#
# It is its own image rather than another CMD on the shared protocol-layer
# image because the watcher is the one component with an outbound dependency
# (an EVM RPC endpoint) and a different operational shape: exactly one
# replica per chain, with a state file it must not share.
#
# Build from the repository root:
#   docker build -f services/icp-chain-watcher/Dockerfile -t stateset/icp-chain-watcher:dev .
#
# See icp-docker/docker-compose.yml for the wired-up stack (mock RPC +
# settler) and the watcher's README for the env-var contract.

FROM node:20-slim

# Non-root user (matches Node default UID/GID). Declared before WORKDIR so
# /app is created owned by `node`.
USER node

WORKDIR /app

COPY --chown=node:node services/icp-chain-watcher /app/services/icp-chain-watcher

# Same healthcheck script the handler and settler use: hits /healthz on $PORT.
COPY --chown=node:node icp-docker/healthcheck.mjs /app/healthcheck.mjs

# RPC_URL, CONTRACT_ADDRESS and SETTLER_URL are required at runtime; the
# entrypoint exits 2 with a diagnostic if any is missing.
ENV PORT=8789

CMD ["node", "services/icp-chain-watcher/src/server.mjs"]
