# Versioning

This book represents the **latest** documentation from the main branch.

## Semantic Versioning

iCommerce follows [Semantic Versioning](https://semver.org/):

- **Major** (`X.0.0`): Breaking changes to stable public surfaces
- **Minor** (`X.Y.0`): Backward-compatible additions and new capabilities
- **Patch** (`X.Y.Z`): Bug fixes, security fixes, performance work, and documentation updates

## Current Line

The current workspace release line is `v1.3.0`. The `v1` compatibility
contract is active now, so breaking changes to the documented stable surfaces
require a future `v2.0.0` release.

## Active v1 Contract

The `v1.0.0` contract is active for the published `v1.x` line:

- Patch releases in `v1.x` are non-breaking bug, security, performance, and
  documentation updates.
- Minor releases in `v1.x` are additive for the documented stable surfaces.
- Stable surfaces for `v1.x` are the curated Rust SDK and embedded preludes,
  language binding version line, MCP tool names and schemas, CLI flags, policy
  YAML, and additive SQLite migrations.
- Deprecations require runtime warnings and documentation updates, and remain
  supported for at least two minor releases and 90 days before removal in the
  next major.
- `v1.0.x` is the initial stabilization line: critical regressions and security
  fixes are eligible for backport there until `v1.3.0` ships. After that, the
  latest `v1.y` and previous `v1.(y-1)` lines receive security and
  release-blocking bug backports.

## Current Release

**v1.3.0** — patch release for CLI outbound network security, remote
marketplace package validation, and BlueBubbles auth handling. See
`CHANGELOG.md` and `RELEASE_NOTES_v1.3.0.md` for the release narrative.

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

For each tagged release, a documentation snapshot is created under `docs/versions/vX.Y.Z/`. Each snapshot is a standalone mdBook so it can be built and hosted under a stable path (for example, `/docs/v1.0.0/`).

## Process

1. Run `./docs/scripts/snapshot-version.sh vX.Y.Z`
2. Review the snapshot and adjust any version-specific notes
3. Publish both the latest book and the snapshot

See `RELEASING.md` for the full release checklist.

For exact guarantee boundaries and current open trust gaps, see [Trust Foundation](trust-foundation.md).
