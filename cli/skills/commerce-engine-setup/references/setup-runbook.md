# Setup Runbook

## Prerequisites

- Node.js 18+ for the CLI
- Optional: Docker + Docker Compose for sequencer
- Repo paths:
  - CLI: /home/dom/stateset-icommerce/cli
  - Examples: /home/dom/stateset-icommerce/examples

## Local Setup (No Sync)

1. Install the CLI locally:
   - `cd /home/dom/stateset-icommerce/cli`
   - `npm install`
   - `npm link`
2. Choose a database path:
   - CLI flag: `--db ./store.db`
   - Env var: `STATESET_DB=./store.db`
3. Run a health check:
   - `stateset-doctor`
4. Seed demo data:
   - `examples/seed-demo-data.sh --db ./store.db`
5. Verify setup:
   - `examples/verify-setup.sh --db ./store.db`

## Optional Sync Setup (Sequencer)

1. Start the sequencer:
   - `docker-compose -f examples/docker-compose.full.yml up -d`
2. Initialize sync:
   - `stateset-sync init --sequencer-url http://localhost:8080 --tenant-id <uuid> --store-id <uuid> --db ./store.db`
3. Register keys:
   - `stateset-sync keys:generate`
   - `stateset-sync keys:register`
4. Push and pull:
   - `stateset-sync push --db ./store.db`
   - `stateset-sync pull --db ./store.db`

## Autonomous Engine (Optional)

- Start: `stateset-autonomous start --db ./.stateset/commerce.db`
- Status: `stateset-autonomous status`

## Quick Health Checks

- `stateset --db ./store.db "list products"`
- `stateset --db ./store.db "list customers"`
- `stateset --db ./store.db --apply "create customer customer@example.com Example Customer"`

## Cleanup

- Remove the database file if you want a clean state.
- Remove `.stateset/` if you want to re-init sync.
