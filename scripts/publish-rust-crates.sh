#!/usr/bin/env bash
set -euo pipefail

MODE="${1:---dry-run}"

case "$MODE" in
  --dry-run|--publish)
    ;;
  *)
    echo "Usage: $0 [--dry-run|--publish]" >&2
    exit 1
    ;;
esac

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [[ "$MODE" == "--publish" && -z "${CARGO_REGISTRY_TOKEN:-}" ]]; then
  echo "CARGO_REGISTRY_TOKEN is required when running with --publish" >&2
  exit 1
fi

# Publish order is dependency-aware to avoid crates.io index resolution failures.
CRATES=(
  stateset-primitives
  stateset-core
  stateset-crypto
  stateset-observability
  stateset-macros
  stateset-policy
  stateset-protocol
  stateset-db
  stateset-pricing
  stateset-a2a
  stateset-sync
  stateset-jobs
  stateset-migrations
  stateset-authz
  stateset-embedded
  stateset-http
  stateset-ffi
  stateset-sdk
)

for crate in "${CRATES[@]}"; do
  publish_cmd=(cargo publish --locked -p "$crate")
  if [[ "$MODE" == "--dry-run" ]]; then
    publish_cmd+=(--dry-run)
  fi

  echo "==> ${publish_cmd[*]}"
  "${publish_cmd[@]}"

  if [[ "$MODE" == "--publish" ]]; then
    # Allow crates.io index propagation before publishing dependents.
    sleep 20
  fi
done

echo "Completed $MODE for ${#CRATES[@]} crates."
