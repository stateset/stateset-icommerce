# Versioning

This book represents the **v0.9.8** documentation snapshot.

## Snapshot scope

This snapshot is frozen to the **v0.9.8** release line. For the latest docs
from the main branch, use the root `docs/` book.

## Release snapshots

For each tagged release, a documentation snapshot is created under
`docs/versions/vX.Y.Z/`. Each snapshot is a standalone mdBook that can be
built and hosted under a stable path (for example, `/docs/v0.9.8/`).

## Process

1. Run `./docs/scripts/snapshot-version.sh vX.Y.Z`.
2. Review the snapshot and adjust any version-specific notes.
3. Publish both the latest book and the snapshot.

See `RELEASING.md` for the full release checklist.
