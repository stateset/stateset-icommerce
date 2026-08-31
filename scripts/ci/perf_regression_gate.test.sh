#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

write_estimate() {
  local root="$1"
  local benchmark_id="$2"
  local median="$3"
  local estimate_dir="${root}/target/criterion/${benchmark_id}/new"
  mkdir -p "${estimate_dir}"
  cat > "${estimate_dir}/estimates.json" <<EOF
{"median":{"point_estimate":${median}}}
EOF
}

THRESHOLDS="${TMP_DIR}/thresholds.json"
cat > "${THRESHOLDS}" <<'EOF'
{"benchmarks":[{"id":"proof/fast","max_median_ns":1000,"note":"fixture"}]}
EOF

PASS_ROOT="${TMP_DIR}/pass"
mkdir -p "${PASS_ROOT}"
write_estimate "${PASS_ROOT}" "proof/fast" 900
(
  cd "${PASS_ROOT}"
  PERF_SKIP_BENCH_RUN=1 \
  PERF_CRITERION_ROOT="${PASS_ROOT}/target/criterion" \
  PERF_THRESHOLD_FILE="${THRESHOLDS}" \
  PERF_REPORT_PATH="${PASS_ROOT}/report.md" \
  PERF_REPORT_JSON_PATH="${PASS_ROOT}/report.json" \
    "${REPO_ROOT}/scripts/ci/perf_regression_gate.sh"
)

python - "${PASS_ROOT}/report.json" <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
assert report["schema_version"] == 2
assert report["result"] == "passed"
assert report["summary"] == {
    "total": 1,
    "passed": 1,
    "failed": 0,
    "missing": 0,
    "threshold_multiplier": 1.0,
}
row = report["benchmarks"][0]
assert row["criterion_estimate"] == "criterion-estimates/proof__fast.json"
assert len(row["criterion_estimate_sha256"]) == 64
assert report["configuration"]["threshold_file_sha256"]
assert report["provenance"]["commit_sha"]
PY

FAIL_ROOT="${TMP_DIR}/fail"
mkdir -p "${FAIL_ROOT}"
write_estimate "${FAIL_ROOT}" "proof/fast" 1100
if (
  cd "${FAIL_ROOT}"
  PERF_SKIP_BENCH_RUN=1 \
  PERF_CRITERION_ROOT="${FAIL_ROOT}/target/criterion" \
  PERF_THRESHOLD_FILE="${THRESHOLDS}" \
  PERF_REPORT_PATH="${FAIL_ROOT}/report.md" \
  PERF_REPORT_JSON_PATH="${FAIL_ROOT}/report.json" \
    "${REPO_ROOT}/scripts/ci/perf_regression_gate.sh"
); then
  echo "error: over-budget fixture unexpectedly passed" >&2
  exit 1
fi

python - "${FAIL_ROOT}/report.json" <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
assert report["result"] == "failed"
assert report["summary"]["failed"] == 1
assert report["benchmarks"][0]["status"] == "FAIL"
PY

echo "perf regression gate fixtures passed"
