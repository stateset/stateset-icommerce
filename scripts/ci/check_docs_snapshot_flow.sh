#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
VERSION="v$(node -p "require('./bindings/node/package.json').version")"
TMP_ROOT="$(mktemp -d)"
trap 'rm -rf "$TMP_ROOT"' EXIT

mkdir -p "$TMP_ROOT/docs"
cp "$ROOT_DIR/docs/book.toml" "$TMP_ROOT/docs/book.toml"
cp -R "$ROOT_DIR/docs/src" "$TMP_ROOT/docs/src"

STATESET_DOCS_ROOT="$TMP_ROOT" bash "$ROOT_DIR/docs/scripts/snapshot-version.sh" "$VERSION"

SNAPSHOT_DIR="$TMP_ROOT/docs/versions/$VERSION"
SNAPSHOT_VERSIONING="$SNAPSHOT_DIR/src/versioning.md"

if [[ ! -f "$SNAPSHOT_DIR/book/index.html" ]]; then
  echo "Snapshot flow did not build $SNAPSHOT_DIR/book/index.html" >&2
  exit 1
fi

grep -F "This book represents the **$VERSION** documentation snapshot." "$SNAPSHOT_VERSIONING" >/dev/null || {
  echo "Snapshot versioning page did not pin the snapshot version." >&2
  exit 1
}

if grep -F "latest documentation from the main branch" "$SNAPSHOT_VERSIONING" >/dev/null; then
  echo "Snapshot versioning page still claims it is the latest main-branch documentation." >&2
  exit 1
fi

grep -F "/docs/$VERSION/" "$SNAPSHOT_VERSIONING" >/dev/null || {
  echo "Snapshot versioning page did not include the stable docs path for $VERSION." >&2
  exit 1
}

echo "Docs snapshot flow is valid for $VERSION."
