# Versioning

This book represents the **latest** documentation from the main branch.

## Semantic Versioning

iCommerce follows [Semantic Versioning](https://semver.org/) with an explicit pre-`1.0` policy:

- **Major** (`X.0.0`) after `1.0`: Breaking changes to the public API surface
- **Minor** (`0.X.0` while pre-`1.0`, `X.Y.0` after `1.0`): New features and backward-compatible additions by project policy
- **Patch** (`0.0.X` / `X.Y.Z`): Bug fixes, performance improvements, documentation updates

## Pre-1.0 Status

Current releases are still pre-`1.0`.

The compatibility table below is a project policy commitment, not a claim that every public surface is permanently frozen. If a pre-`1.0` breaking change is unavoidable, it must be called out in release notes and accompanied by migration guidance.

## Current Release

**v0.9.6** — See `RELEASING.md` for the full changelog.

## Compatibility Guarantees

| Layer | Guarantee |
|-------|-----------|
| Rust API (`Commerce`, `AsyncCommerce`) | Stable within major version |
| Language bindings | Match Rust API version |
| MCP tool names | Stable once shipped — tools are never renamed, only deprecated |
| MCP tool schemas | Backward compatible — new optional fields may be added |
| SQLite schema | Migrations are additive — new tables/columns, never removed |
| Policy YAML format | Stable within major version |
| CLI flags | Stable — new flags are added, existing flags are not removed |
| VES v1.0 wire format | Immutable — versioned protocol, new versions are additive |

## Deprecation Policy

When a feature is deprecated:

1. It continues to work for at least one minor version
2. A deprecation warning is emitted at runtime
3. Documentation is updated with the replacement
4. The feature is removed in the next major version

## Upgrade Path

### Tier Upgrades

Tier upgrades are non-breaking — your database, policies, and configuration carry forward:

```
Tier 1 → Tier 2: Add .stateset/sync.json
Tier 2 → Tier 3: Add chain RPC URL to sync.json
```

### Version Upgrades

```bash
# Update the CLI
npm install -g @stateset/cli@latest

# Migrations run automatically on first connection
stateset "show me all customers"   # triggers any pending migrations
```

## Release Snapshots

For each tagged release, a documentation snapshot is created under `docs/versions/vX.Y.Z/`. Each snapshot is a standalone mdBook so it can be built and hosted under a stable path (for example, `/docs/v0.9.6/`).

## Process

1. Run `./docs/scripts/snapshot-version.sh vX.Y.Z`
2. Review the snapshot and adjust any version-specific notes
3. Publish both the latest book and the snapshot

See `RELEASING.md` for the full release checklist.

For exact guarantee boundaries and current open trust gaps, see [Trust Foundation](trust-foundation.md).
