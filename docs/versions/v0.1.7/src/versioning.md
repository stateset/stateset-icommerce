# Versioning

This book represents the **v0.1.7** documentation snapshot.

## Release snapshots

For each tagged release, create a snapshot under `docs/versions/vX.Y.Z/`. Each snapshot is a standalone mdBook so it can be built and hosted under a stable path (for example, `/docs/v0.1.7/`).

## Process

1. Run `./docs/scripts/snapshot-version.sh vX.Y.Z`.
2. Review the snapshot and adjust any version-specific notes.
3. Publish both the latest book and the snapshot.

See `RELEASING.md` for the full release checklist.
