#!/usr/bin/env bash
set -euo pipefail

if ! command -v actionlint >/dev/null 2>&1; then
  echo "::error::actionlint is not installed or not in PATH"
  exit 127
fi

actionlint -color
