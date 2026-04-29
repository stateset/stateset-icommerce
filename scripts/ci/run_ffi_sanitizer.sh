#!/usr/bin/env bash
set -euo pipefail

SANITIZER="${SANITIZER:-${1:-}}"
DRY_RUN="${DRY_RUN:-0}"
SANITIZER_TARGET="${SANITIZER_TARGET:-x86_64-unknown-linux-gnu}"

if [[ -z "${SANITIZER}" ]]; then
  echo "error: set SANITIZER=address or pass as first argument" >&2
  exit 1
fi

case "${SANITIZER}" in
  address)
    ;;
  *)
    echo "error: unsupported SANITIZER '${SANITIZER}' (expected address)" >&2
    exit 1
    ;;
esac

export RUSTFLAGS="-Zsanitizer=${SANITIZER}"
export RUSTDOCFLAGS="-Zsanitizer=${SANITIZER}"

if [[ "${SANITIZER}" == "address" ]]; then
  export ASAN_OPTIONS="${ASAN_OPTIONS:-detect_leaks=0:halt_on_error=1}"
fi

cmd=(
  cargo +nightly test
  --locked
  -Zbuild-std
  --target "${SANITIZER_TARGET}"
  -p stateset-ffi
  --lib
)

echo "+ ${cmd[*]}"
if [[ "${DRY_RUN}" == "1" ]]; then
  exit 0
fi

"${cmd[@]}"
