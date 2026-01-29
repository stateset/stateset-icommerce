# Sync Commands

## Initialize

- `stateset-sync init --sequencer-url http://localhost:8080 --tenant-id <uuid> --store-id <uuid> --db ./store.db`

## Keys

- `stateset-sync keys:generate`
- `stateset-sync keys:register`

## Push / Pull

- `stateset-sync status --db ./store.db`
- `stateset-sync push --db ./store.db --batch-size 100`
- `stateset-sync pull --db ./store.db --limit 1000`

## Conflict Resolution

- `stateset-sync rebase`
- Use `resolve_conflict` MCP tool in autonomous mode if needed.
