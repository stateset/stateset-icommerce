#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

export STATESET_PERF_GATE="${STATESET_PERF_GATE:-1}"
export STATESET_PERF_GATE_FILE="${STATESET_PERF_GATE_FILE:-$ROOT_DIR/crates/stateset-benches/perf-gates.json}"

cd "$ROOT_DIR"

cargo bench -p stateset-benches \
  --bench money_arithmetic \
  --bench jcs_canonicalize \
  --bench merkle_tree \
  --bench event_bus_throughput \
  --bench sqlite_batch_insert \
  "$@"
