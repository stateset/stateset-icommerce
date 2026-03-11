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

run_dry_run_checks() {
  local crate="$1"

  echo "==> cargo package --list -p ${crate}"
  cargo package --list --locked -p "$crate" >/dev/null

  echo "==> cargo check --locked -p ${crate} --all-targets"
  cargo check --locked -p "$crate" --all-targets
}

publish_with_retries() {
  local crate="$1"
  local max_attempts=12
  local attempt=1
  local log_file
  local status

  while true; do
    log_file="$(mktemp)"
    set +e
    cargo publish --locked -p "$crate" 2>&1 | tee "$log_file"
    status=${PIPESTATUS[0]}
    set -e

    if [[ $status -eq 0 ]]; then
      rm -f "$log_file"
      return 0
    fi

    if grep -Eq 'already uploaded|already exists' "$log_file"; then
      echo "crate ${crate} is already published; continuing"
      rm -f "$log_file"
      return 0
    fi

    if grep -Eq 'failed to select a version for the requirement|no matching package named|could not be found in registry index' "$log_file"; then
      if (( attempt >= max_attempts )); then
        echo "Timed out waiting for crates.io index propagation for ${crate}" >&2
        rm -f "$log_file"
        return "$status"
      fi

      echo "crates.io index not ready for ${crate}; retry ${attempt}/${max_attempts} in 20s"
      attempt=$((attempt + 1))
      rm -f "$log_file"
      sleep 20
      continue
    fi

    rm -f "$log_file"
    return "$status"
  done
}

for crate in "${CRATES[@]}"; do
  if [[ "$MODE" == "--dry-run" ]]; then
    run_dry_run_checks "$crate"
  else
    echo "==> cargo publish --locked -p ${crate}"
    publish_with_retries "$crate"
    # Allow crates.io index propagation before publishing dependents.
    sleep 20
  fi
done

echo "Completed $MODE for ${#CRATES[@]} crates."
