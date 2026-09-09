# ICP protocol-layer image.
#
# Single image that serves icp-handler, settler-stateset, and icp-mcp.
# The component to run is selected by CMD/entrypoint at runtime — see
# docker-compose.yml.
#
# Zero npm install: every protocol-layer component uses only Node stdlib.

FROM node:20-slim

# Non-root user (matches Node default UID/GID).
USER node

WORKDIR /app

# Copy the protocol-layer surfaces. The .dockerignore at the repo root keeps
# this lean — only the directories these services actually need.
COPY --chown=node:node icp-handler /app/icp-handler
COPY --chown=node:node icp-mcp /app/icp-mcp
COPY --chown=node:node services/settler-stateset /app/services/settler-stateset

# The chain-watcher's ABI decoder — mock-rpc.mjs imports EVENT_TOPICS from it
# so the synthetic logs and the real decoder can never drift apart. (The
# watcher itself runs from its own image; see
# services/icp-chain-watcher/Dockerfile.)
COPY --chown=node:node services/icp-chain-watcher /app/services/icp-chain-watcher

# Add a tiny healthcheck script used by the handler + settler, and the mock
# EVM RPC that gives the chain-watcher something to observe in the stack.
# mock-rpc.mjs keeps its repo-relative path so its ../services/... import
# resolves identically inside and outside the image.
COPY --chown=node:node icp-docker/healthcheck.mjs /app/healthcheck.mjs
COPY --chown=node:node icp-docker/mock-rpc.mjs /app/icp-docker/mock-rpc.mjs

# Default command does nothing; docker-compose overrides with the actual service.
CMD ["node", "-e", "console.error('icp-docker: select a service via docker-compose')"]
