#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-}"
if [ -z "$VERSION" ]; then
  echo "Usage: $0 vX.Y.Z" >&2
  exit 1
fi

ROOT="${STATESET_DOCS_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
SRC="$ROOT/docs"
DEST="$ROOT/docs/versions/$VERSION"

if [ -e "$DEST" ]; then
  echo "Snapshot already exists: $DEST" >&2
  exit 1
fi

mkdir -p "$DEST"
cp "$SRC/book.toml" "$DEST/book.toml"
cp -R "$SRC/src" "$DEST/src"

cat > "$DEST/src/versioning.md" <<EOF
# Versioning

This book represents the **$VERSION** documentation snapshot.

## Snapshot scope

This snapshot is frozen to the **$VERSION** release line. For the latest docs
from the main branch, use the root \`docs/\` book.

## Release snapshots

For each tagged release, a documentation snapshot is created under
\`docs/versions/vX.Y.Z/\`. Each snapshot is a standalone mdBook that can be
built and hosted under a stable path (for example, \`/docs/$VERSION/\`).

## Process

1. Run \`./docs/scripts/snapshot-version.sh vX.Y.Z\`.
2. Review the snapshot and adjust any version-specific notes.
3. Publish both the latest book and the snapshot.

See \`RELEASING.md\` for the full release checklist.
EOF

mdbook build "$DEST"

echo "Created snapshot at $DEST"
echo "Built snapshot book at $DEST/book"
