#!/bin/bash
set -e

cleanup() { :; }
trap cleanup EXIT

DB_PATH="./store.db"
SEQUENCER_URL="http://localhost:8080"
EXAMPLES_DIR="/home/dom/stateset-icommerce/examples"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --db) DB_PATH="$2"; shift 2 ;;
    --sequencer-url) SEQUENCER_URL="$2"; shift 2 ;;
    --examples-dir) EXAMPLES_DIR="$2"; shift 2 ;;
    --help)
      echo "Usage: verify-setup.sh [--db PATH] [--sequencer-url URL] [--examples-dir PATH]" >&2
      printf '{"ok":true,"usage":"verify-setup.sh [--db PATH] [--sequencer-url URL] [--examples-dir PATH]"}
'
      exit 0
      ;;
    *) shift ;;
  esac
done

if [ ! -x "$EXAMPLES_DIR/verify-setup.sh" ]; then
  echo "verify-setup.sh not found at $EXAMPLES_DIR" >&2
  printf '{"ok":false,"error":"verify-setup.sh not found"}
'
  exit 1
fi

"$EXAMPLES_DIR/verify-setup.sh" --db "$DB_PATH" --sequencer-url "$SEQUENCER_URL" >&2 || true
printf '{"ok":true,"db":"%s","sequencer":"%s","script":"%s"}
' "$DB_PATH" "$SEQUENCER_URL" "$EXAMPLES_DIR/verify-setup.sh"
