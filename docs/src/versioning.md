# Versioning

This book represents the **latest** documentation from the main branch.

## Release snapshots

For each tagged release, create a snapshot under `docs/versions/vX.Y.Z/`. Each snapshot is a standalone mdBook so it can be built and hosted under a stable path (for example, `/docs/v0.7.26/`).

## Process

1. Run `./docs/scripts/snapshot-version.sh vX.Y.Z`.
2. Review the snapshot and adjust any version-specific notes.
3. Publish both the latest book and the snapshot.

See `RELEASING.md` for the full release checklist.
