#!/bin/bash
set -e

cleanup() { :; }
trap cleanup EXIT

DB_PATH="./store.db"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --db) DB_PATH="$2"; shift 2 ;;
    --help)
      echo "Usage: sync-status.sh [--db PATH]" >&2
      printf '{"ok":true,"usage":"sync-status.sh [--db PATH]"}
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

stateset-sync status --json --db "$DB_PATH"
