#!/bin/bash
set -e

cleanup() { :; }
trap cleanup EXIT

DB_PATH="./store.db"
BATCH_SIZE="100"
DRY_RUN="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --db) DB_PATH="$2"; shift 2 ;;
    --batch-size) BATCH_SIZE="$2"; shift 2 ;;
    --dry-run) DRY_RUN="true"; shift ;;
    --help)
      echo "Usage: sync-push.sh [--db PATH] [--batch-size N] [--dry-run]" >&2
      printf '{"ok":true,"usage":"sync-push.sh [--db PATH] [--batch-size N] [--dry-run]"}
'
      exit 0
      ;;
    *) shift ;;
  esac
done

if ! command -v stateset-sync >/dev/null 2>&1; then
  echo "stateset-sync CLI not found" >&2
  printf '{"ok":false,"error":"stateset-sync CLI not found"}
'
  exit 1
fi

CMD_OUT=$(stateset-sync push --db "$DB_PATH" --batch-size "$BATCH_SIZE" $( [ "$DRY_RUN" = "true" ] && echo "--dry-run" ) 2>&1) || true

echo "$CMD_OUT" >&2

ACCEPTED=$(echo "$CMD_OUT" | sed -n 's/.*Push complete: \([0-9][0-9]*\) accepted, \([0-9][0-9]*\) rejected.*//p')
REJECTED=$(echo "$CMD_OUT" | sed -n 's/.*Push complete: \([0-9][0-9]*\) accepted, \([0-9][0-9]*\) rejected.*//p')
WOULD_PUSH=$(echo "$CMD_OUT" | sed -n 's/.*Would push \([0-9][0-9]*\) events.*//p')

if [ -n "$ACCEPTED" ]; then
  printf '{"ok":true,"accepted":%s,"rejected":%s,"db":"%s"}
' "$ACCEPTED" "$REJECTED" "$DB_PATH"
elif [ -n "$WOULD_PUSH" ]; then
  printf '{"ok":true,"dry_run":true,"would_push":%s,"db":"%s"}
' "$WOULD_PUSH" "$DB_PATH"
else
  printf '{"ok":true,"db":"%s"}
' "$DB_PATH"
fi
