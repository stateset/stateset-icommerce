#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-}"
if [ -z "$VERSION" ]; then
  echo "Usage: $0 vX.Y.Z" >&2
  exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SRC="$ROOT/docs"
DEST="$ROOT/docs/versions/$VERSION"

if [ -e "$DEST" ]; then
  echo "Snapshot already exists: $DEST" >&2
  exit 1
fi

mkdir -p "$DEST"
cp "$SRC/book.toml" "$DEST/book.toml"
cp -R "$SRC/src" "$DEST/src"

echo "Created snapshot at $DEST"
