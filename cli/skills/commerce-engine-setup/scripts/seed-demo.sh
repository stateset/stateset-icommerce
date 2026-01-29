#!/bin/bash
set -e

cleanup() { :; }
trap cleanup EXIT

DB_PATH="./store.db"
EXAMPLES_DIR="/home/dom/stateset-icommerce/examples"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --db) DB_PATH="$2"; shift 2 ;;
    --examples-dir) EXAMPLES_DIR="$2"; shift 2 ;;
    --help)
      echo "Usage: seed-demo.sh [--db PATH] [--examples-dir PATH]" >&2
      printf '{"ok":true,"usage":"seed-demo.sh [--db PATH] [--examples-dir PATH]"}
'
      exit 0
      ;;
    *) shift ;;
  esac
done

if ! command -v stateset >/dev/null 2>&1; then
  echo "stateset CLI not found" >&2
  printf '{"ok":false,"error":"stateset CLI not found"}
'
  exit 1
fi

if [ ! -x "$EXAMPLES_DIR/seed-demo-data.sh" ]; then
  echo "seed-demo-data.sh not found at $EXAMPLES_DIR" >&2
  printf '{"ok":false,"error":"seed-demo-data.sh not found"}
'
  exit 1
fi

"$EXAMPLES_DIR/seed-demo-data.sh" --db "$DB_PATH" >&2
printf '{"ok":true,"db":"%s","script":"%s"}
' "$DB_PATH" "$EXAMPLES_DIR/seed-demo-data.sh"
