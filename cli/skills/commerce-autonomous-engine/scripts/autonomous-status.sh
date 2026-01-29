#!/bin/bash
set -e

cleanup() { :; }
trap cleanup EXIT

if ! command -v stateset-autonomous >/dev/null 2>&1; then
  echo "stateset-autonomous CLI not found" >&2
  printf '{"ok":false,"error":"stateset-autonomous CLI not found"}
'
  exit 1
fi

stateset-autonomous status 1>&2 || true
printf '{"ok":true}
'
