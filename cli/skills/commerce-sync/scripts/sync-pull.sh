#!/bin/bash
set -e

cleanup() { :; }
trap cleanup EXIT

DB_PATH="./store.db"
LIMIT="1000"
FROM_SEQ=""
DRY_RUN="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --db) DB_PATH="$2"; shift 2 ;;
    --limit) LIMIT="$2"; shift 2 ;;
    --from) FROM_SEQ="$2"; shift 2 ;;
    --dry-run) DRY_RUN="true"; shift ;;
    --help)
      echo "Usage: sync-pull.sh [--db PATH] [--limit N] [--from SEQ] [--dry-run]" >&2
      printf '{"ok":true,"usage":"sync-pull.sh [--db PATH] [--limit N] [--from SEQ] [--dry-run]"}
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

PULL_ARGS=(--db "$DB_PATH" --limit "$LIMIT")
if [ -n "$FROM_SEQ" ]; then
  PULL_ARGS+=(--from "$FROM_SEQ")
fi
if [ "$DRY_RUN" = "true" ]; then
  PULL_ARGS+=(--dry-run)
fi

CMD_OUT=$(stateset-sync pull "${PULL_ARGS[@]}" 2>&1) || true

echo "$CMD_OUT" >&2

PULLED=$(echo "$CMD_OUT" | sed -n 's/.*Pull complete: \([0-9][0-9]*\) events pulled.*//p')
WOULD_PULL=$(echo "$CMD_OUT" | sed -n 's/.*Would pull \([0-9][0-9]*\) events.*//p')

if [ -n "$PULLED" ]; then
  printf '{"ok":true,"pulled":%s,"db":"%s"}
' "$PULLED" "$DB_PATH"
elif [ -n "$WOULD_PULL" ]; then
  printf '{"ok":true,"dry_run":true,"would_pull":%s,"db":"%s"}
' "$WOULD_PULL" "$DB_PATH"
else
  printf '{"ok":true,"db":"%s"}
' "$DB_PATH"
fi
