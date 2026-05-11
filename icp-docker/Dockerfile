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

# Add a tiny healthcheck script used by the handler + settler.
COPY --chown=node:node icp-docker/healthcheck.mjs /app/healthcheck.mjs

# Default command does nothing; docker-compose overrides with the actual service.
CMD ["node", "-e", "console.error('icp-docker: select a service via docker-compose')"]
