# Docs

This directory contains the mdBook source for StateSet iCommerce.

Related planning docs:

- [Agentic Commerce Baseline](./AGENTIC_COMMERCE_BASELINE.md)
- [Competitive Landscape](./COMPETITIVE_LANDSCAPE.md)
- [Default Infrastructure Playbook](./DEFAULT_INFRASTRUCTURE_PLAYBOOK.md)
- [Outcomes Model](./OUTCOMES_MODEL.md)

## Build locally

```bash
cargo install mdbook
mdbook build docs
```

## Serve locally

```bash
mdbook serve docs
```

## API references

Generate per-binding API docs into `docs/api/` (requires language-specific doc tools):

```bash
./docs/scripts/generate-api.sh
```

## Version snapshots

Create a versioned snapshot under `docs/versions/`:

```bash
./docs/scripts/snapshot-version.sh v0.7.1
```

## Generated inventories

Three manifest-backed appendices are generated into `docs/src/appendix/`:

- `node ./scripts/ci/generate_agent_inventory.mjs`
- `node ./scripts/ci/generate_mcp_inventory.mjs`
- `node ./scripts/ci/generate_workspace_inventory.mjs`
