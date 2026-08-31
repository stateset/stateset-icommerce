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
PERF_CRITERION_ROOT="${PERF_CRITERION_ROOT:-target/criterion}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${REPO_ROOT}"

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

PERF_COMMIT_SHA="${GITHUB_SHA:-$(git rev-parse HEAD 2>/dev/null || echo unknown)}"
PERF_RUN_ID="${GITHUB_RUN_ID:-local}"
PERF_RUN_ATTEMPT="${GITHUB_RUN_ATTEMPT:-local}"
PERF_RUNNER_OS="${RUNNER_OS:-$(uname -s)}"
PERF_RUNNER_ARCH="${RUNNER_ARCH:-$(uname -m)}"
PERF_RUNNER_IMAGE="${ImageOS:-local}"
PERF_RUNNER_IMAGE_VERSION="${ImageVersion:-unknown}"
PERF_CPU_MODEL="$(awk -F ': ' '/model name/{print $2; exit}' /proc/cpuinfo 2>/dev/null || true)"
PERF_CPU_COUNT="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo unknown)"
PERF_RUSTC_VERSION="$(rustc --version)"
PERF_CARGO_VERSION="$(cargo --version)"

export PERF_ALLOW_MISSING_ESTIMATES PERF_REPORT_PATH PERF_REPORT_JSON_PATH
export PERF_COMMIT_SHA PERF_RUN_ID PERF_RUN_ATTEMPT PERF_RUNNER_OS PERF_RUNNER_ARCH
export PERF_RUNNER_IMAGE PERF_RUNNER_IMAGE_VERSION PERF_CPU_MODEL PERF_CPU_COUNT
export PERF_RUSTC_VERSION PERF_CARGO_VERSION PERF_SAMPLE_SIZE PERF_WARMUP_SECONDS
export PERF_MEASUREMENT_SECONDS PERF_CRITERION_ROOT
python - "${PERF_THRESHOLD_FILE}" "${PERF_THRESHOLD_MULTIPLIER}" <<'PY'
import datetime
import hashlib
import json
import os
import shutil
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
raw_estimate_dir = report_json_path.parent / "criterion-estimates"
raw_estimate_dir.mkdir(parents=True, exist_ok=True)

def estimate_path_candidates(bench_id):
    exact = Path(os.environ["PERF_CRITERION_ROOT"]) / bench_id / "new" / "estimates.json"
    candidates = [exact]
    sanitized = bench_id.replace("/", "_")
    if sanitized != bench_id:
        candidates.append(Path(os.environ["PERF_CRITERION_ROOT"]) / sanitized / "new" / "estimates.json")
    return candidates

for bench in benchmarks:
    bench_id = bench["id"]
    max_median_ns = float(bench["max_median_ns"])
    limit_ns = max_median_ns * multiplier
    candidate_paths = estimate_path_candidates(bench_id)
    estimate_path = next((path for path in candidate_paths if path.exists()), candidate_paths[0])
    note = bench.get("note", "")

    if not estimate_path.exists():
        status = "MISSING"
        observed_ns = None
        missing.append(bench_id)
        if not allow_missing:
            attempted = ", ".join(str(path) for path in candidate_paths)
            failures.append(f"missing estimate file for {bench_id}; tried: {attempted}")
    else:
        with estimate_path.open("r", encoding="utf-8") as f:
            estimate = json.load(f)
        observed_ns = float(estimate["median"]["point_estimate"])
        raw_name = bench_id.replace("/", "__") + ".json"
        raw_path = raw_estimate_dir / raw_name
        shutil.copyfile(estimate_path, raw_path)
        estimate_sha256 = hashlib.sha256(raw_path.read_bytes()).hexdigest()
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
            "criterion_estimate": None if observed_ns is None else str(raw_path.relative_to(report_json_path.parent)),
            "criterion_estimate_sha256": None if observed_ns is None else estimate_sha256,
        }
    )

summary = {
    "total": len(rows),
    "passed": sum(1 for r in rows if r["status"] == "PASS"),
    "failed": sum(1 for r in rows if r["status"] == "FAIL"),
    "missing": sum(1 for r in rows if r["status"] == "MISSING"),
    "threshold_multiplier": multiplier,
}

result = "failed" if failures else ("passed_with_missing" if missing else "passed")
provenance = {
    "commit_sha": os.environ["PERF_COMMIT_SHA"],
    "github_run_id": os.environ["PERF_RUN_ID"],
    "github_run_attempt": os.environ["PERF_RUN_ATTEMPT"],
    "generated_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "runner": {
        "os": os.environ["PERF_RUNNER_OS"],
        "arch": os.environ["PERF_RUNNER_ARCH"],
        "image": os.environ["PERF_RUNNER_IMAGE"],
        "image_version": os.environ["PERF_RUNNER_IMAGE_VERSION"],
        "cpu_model": os.environ["PERF_CPU_MODEL"] or "unknown",
        "logical_cpus": os.environ["PERF_CPU_COUNT"],
    },
    "toolchain": {
        "rustc": os.environ["PERF_RUSTC_VERSION"],
        "cargo": os.environ["PERF_CARGO_VERSION"],
    },
}
configuration = {
    "threshold_file": str(threshold_file),
    "threshold_file_sha256": hashlib.sha256(threshold_file.read_bytes()).hexdigest(),
    "threshold_multiplier": multiplier,
    "sample_size": int(os.environ["PERF_SAMPLE_SIZE"]),
    "warm_up_seconds": float(os.environ["PERF_WARMUP_SECONDS"]),
    "measurement_seconds": float(os.environ["PERF_MEASUREMENT_SECONDS"]),
    "allow_missing_estimates": allow_missing,
}
report_json = {
    "schema_version": 2,
    "result": result,
    "provenance": provenance,
    "configuration": configuration,
    "summary": summary,
    "benchmarks": rows,
}
report_json_path.write_text(json.dumps(report_json, indent=2), encoding="utf-8")

lines = []
lines.append("# Perf Regression Report")
lines.append("")
lines.append(f"- Result: **{result}**")
lines.append(f"- Commit: `{provenance['commit_sha']}`")
lines.append(f"- Generated: {provenance['generated_at']}")
lines.append(
    f"- Runner: `{provenance['runner']['image']}` / `{provenance['runner']['arch']}` / "
    f"`{provenance['runner']['cpu_model']}` ({provenance['runner']['logical_cpus']} logical CPUs)"
)
lines.append(f"- Threshold multiplier: `{multiplier}`")
lines.append(
    f"- Threshold file: `{threshold_file}` "
    f"(SHA-256 `{configuration['threshold_file_sha256']}`)"
)
lines.append(
    f"- Criterion: sample size `{configuration['sample_size']}`, warm-up "
    f"`{configuration['warm_up_seconds']}s`, measurement `{configuration['measurement_seconds']}s`"
)
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
