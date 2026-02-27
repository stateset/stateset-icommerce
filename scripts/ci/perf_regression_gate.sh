#!/usr/bin/env bash
set -euo pipefail

DRY_RUN="${DRY_RUN:-0}"
PERF_SKIP_BENCH_RUN="${PERF_SKIP_BENCH_RUN:-0}"
PERF_ALLOW_MISSING_ESTIMATES="${PERF_ALLOW_MISSING_ESTIMATES:-0}"
PERF_THRESHOLD_FILE="${PERF_THRESHOLD_FILE:-scripts/ci/perf-thresholds.json}"
PERF_SAMPLE_SIZE="${PERF_SAMPLE_SIZE:-10}"
PERF_WARMUP_SECONDS="${PERF_WARMUP_SECONDS:-1}"
PERF_MEASUREMENT_SECONDS="${PERF_MEASUREMENT_SECONDS:-3}"
PERF_THRESHOLD_MULTIPLIER="${PERF_THRESHOLD_MULTIPLIER:-1.0}"
PERF_REPORT_PATH="${PERF_REPORT_PATH:-artifacts/perf-regression/perf-regression-report.md}"
PERF_REPORT_JSON_PATH="${PERF_REPORT_JSON_PATH:-artifacts/perf-regression/perf-regression-report.json}"

if [[ ! -f "${PERF_THRESHOLD_FILE}" ]]; then
  echo "error: threshold file not found at ${PERF_THRESHOLD_FILE}" >&2
  exit 1
fi

mkdir -p "$(dirname "${PERF_REPORT_PATH}")" "$(dirname "${PERF_REPORT_JSON_PATH}")"

run_bench() {
  local package="$1"
  local bench="$2"
  local -a cmd=(
    cargo bench
    --locked
    --package "${package}"
    --bench "${bench}"
    --
    --noplot
    --sample-size "${PERF_SAMPLE_SIZE}"
    --warm-up-time "${PERF_WARMUP_SECONDS}"
    --measurement-time "${PERF_MEASUREMENT_SECONDS}"
  )

  echo "+ ${cmd[*]}"
  if [[ "${DRY_RUN}" == "1" ]]; then
    return 0
  fi

  "${cmd[@]}"
}

if [[ "${PERF_SKIP_BENCH_RUN}" != "1" ]]; then
  run_bench stateset-benches sqlite_batch_insert
  run_bench stateset-embedded api_benchmarks
fi

export PERF_ALLOW_MISSING_ESTIMATES PERF_REPORT_PATH PERF_REPORT_JSON_PATH
python - "${PERF_THRESHOLD_FILE}" "${PERF_THRESHOLD_MULTIPLIER}" <<'PY'
import json
import os
import sys
from pathlib import Path

threshold_file = Path(sys.argv[1])
multiplier = float(sys.argv[2])
allow_missing = os.environ.get("PERF_ALLOW_MISSING_ESTIMATES", "0") == "1"
report_path = Path(os.environ["PERF_REPORT_PATH"])
report_json_path = Path(os.environ["PERF_REPORT_JSON_PATH"])

with threshold_file.open("r", encoding="utf-8") as f:
    data = json.load(f)

benchmarks = data.get("benchmarks", [])
if not benchmarks:
    print("error: threshold file contains no benchmark entries", file=sys.stderr)
    sys.exit(1)

rows = []
failures = []
missing = []

for bench in benchmarks:
    bench_id = bench["id"]
    max_median_ns = float(bench["max_median_ns"])
    limit_ns = max_median_ns * multiplier
    estimate_path = Path("target/criterion") / bench_id / "new" / "estimates.json"
    note = bench.get("note", "")

    if not estimate_path.exists():
        status = "MISSING"
        observed_ns = None
        missing.append(bench_id)
        if not allow_missing:
            failures.append(f"missing estimate file: {estimate_path}")
    else:
        with estimate_path.open("r", encoding="utf-8") as f:
            estimate = json.load(f)
        observed_ns = float(estimate["median"]["point_estimate"])
        status = "PASS" if observed_ns <= limit_ns else "FAIL"
        if status == "FAIL":
            failures.append(
                f"{bench_id}: median {observed_ns:.0f}ns > limit {limit_ns:.0f}ns"
            )

    rows.append(
        {
            "id": bench_id,
            "status": status,
            "observed_median_ns": observed_ns,
            "base_limit_ns": max_median_ns,
            "effective_limit_ns": limit_ns,
            "note": note,
        }
    )

summary = {
    "total": len(rows),
    "passed": sum(1 for r in rows if r["status"] == "PASS"),
    "failed": sum(1 for r in rows if r["status"] == "FAIL"),
    "missing": sum(1 for r in rows if r["status"] == "MISSING"),
    "threshold_multiplier": multiplier,
}

report_json = {"summary": summary, "benchmarks": rows}
report_json_path.write_text(json.dumps(report_json, indent=2), encoding="utf-8")

lines = []
lines.append("# Perf Regression Report")
lines.append("")
lines.append(f"- Threshold multiplier: `{multiplier}`")
lines.append(f"- Threshold file: `{threshold_file}`")
lines.append("")
lines.append("| Benchmark | Status | Median (ns) | Limit (ns) |")
lines.append("|---|---|---:|---:|")
for row in rows:
    observed = "-" if row["observed_median_ns"] is None else f"{row['observed_median_ns']:.0f}"
    lines.append(
        f"| `{row['id']}` | {row['status']} | {observed} | {row['effective_limit_ns']:.0f} |"
    )
if missing:
    lines.append("")
    lines.append("Missing estimates:")
    for bench_id in missing:
        lines.append(f"- `{bench_id}`")

report_path.write_text("\n".join(lines) + "\n", encoding="utf-8")

print(f"wrote {report_path}")
print(f"wrote {report_json_path}")

if failures:
    print("perf regression gate failed:", file=sys.stderr)
    for failure in failures:
        print(f"- {failure}", file=sys.stderr)
    sys.exit(1)
PY
