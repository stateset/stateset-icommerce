#!/usr/bin/env bash
set -euo pipefail

# runs shellcheck and prints GitHub Actions annotations for each warning and error
# https://github.com/koalaman/shellcheck
if ! command -v shellcheck >/dev/null 2>&1; then
  echo "::error::shellcheck is not installed or not in PATH"
  exit 127
fi

IGNORE_DIRS=(
  "./.git/*"
  "./target/*"
  "./node_modules/*"
)

ignore_args=()
for dir in "${IGNORE_DIRS[@]}"; do
  ignore_args+=(-not -path "$dir")
done

find . -name "*.sh" "${ignore_args[@]}" -exec shellcheck -f gcc {} + | \
  while IFS=: read -r file line col severity msg; do
    level="warning"
    [[ "$severity" == *error* ]] && level="error"
    file="${file#./}"
    echo "::${level} file=${file},line=${line},col=${col}::${file}:${line}:${col}:${msg}"
  done

exit "${PIPESTATUS[0]}"
