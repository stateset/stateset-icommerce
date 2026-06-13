#!/usr/bin/env bash
set -euo pipefail

COMPAT_DRY_RUN="${COMPAT_DRY_RUN:-0}"
COMPAT_FAIL_ON_CHECK_FAILURE="${COMPAT_FAIL_ON_CHECK_FAILURE:-1}"
COMPAT_REPORT_DIR="${COMPAT_REPORT_DIR:-artifacts/compatibility}"
COMPAT_REPORT_MD="${COMPAT_REPORT_MD:-${COMPAT_REPORT_DIR}/compatibility-report.md}"
COMPAT_REPORT_JSON="${COMPAT_REPORT_JSON:-${COMPAT_REPORT_DIR}/compatibility-report.json}"

mkdir -p "${COMPAT_REPORT_DIR}"

metadata_file="$(mktemp)"
results_file="$(mktemp)"
trap 'rm -f "${metadata_file}" "${results_file}"' EXIT

cargo metadata --format-version 1 --no-deps > "${metadata_file}"

matrix=(
  "stateset-primitives|default|"
  "stateset-core|default|"
  "stateset-core|metrics|--features metrics"
  "stateset-db|default|"
  "stateset-db|postgres|--no-default-features --features postgres"
  "stateset-db|postgres+saga|--no-default-features --features postgres,saga"
  "stateset-embedded|default|"
  "stateset-embedded|postgres|--features postgres"
  "stateset-embedded|postgres+events|--no-default-features --features postgres,events"
  "stateset-ffi|default|"
  "stateset-sdk|default|"
  "stateset-http|default|"
  "stateset-migrations|default|"
  "stateset-authz|default|"
)

overall_fail=0

for entry in "${matrix[@]}"; do
  IFS='|' read -r crate feature_label extra_args <<< "${entry}"

  cmd=(cargo check --locked -p "${crate}" --all-targets)
  if [[ -n "${extra_args}" ]]; then
    # shellcheck disable=SC2206
    extra_arr=(${extra_args})
    cmd+=("${extra_arr[@]}")
  fi

  cmd_str="${cmd[*]}"
  echo "+ ${cmd_str}"

  start_ts="$(date +%s)"
  status="PASS"

  if [[ "${COMPAT_DRY_RUN}" == "1" ]]; then
    status="SKIPPED"
  else
    set +e
    "${cmd[@]}"
    exit_code=$?
    set -e
    if [[ ${exit_code} -ne 0 ]]; then
      status="FAIL"
      overall_fail=1
    fi
  fi

  end_ts="$(date +%s)"
  duration_seconds=$((end_ts - start_ts))

  printf '%s\t%s\t%s\t%s\t%s\n' \
    "${crate}" \
    "${feature_label}" \
    "${status}" \
    "${duration_seconds}" \
    "${cmd_str}" >> "${results_file}"
done

python - "${metadata_file}" "${results_file}" "${COMPAT_REPORT_MD}" "${COMPAT_REPORT_JSON}" <<'PY'
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

metadata_path = Path(sys.argv[1])
results_path = Path(sys.argv[2])
report_md_path = Path(sys.argv[3])
report_json_path = Path(sys.argv[4])

with metadata_path.open("r", encoding="utf-8") as f:
    metadata = json.load(f)

crate_features = {}
crate_defaults = {}
for package in metadata.get("packages", []):
    name = package["name"]
    features = package.get("features", {})
    crate_features[name] = sorted(features.keys())
    crate_defaults[name] = sorted(features.get("default", []))

rows = []
with results_path.open("r", encoding="utf-8") as f:
    for line in f:
        crate, feature_set, status, duration_seconds, command = line.rstrip("\n").split("\t")
        rows.append(
            {
                "crate": crate,
                "feature_set": feature_set,
                "status": status,
                "duration_seconds": int(duration_seconds),
                "command": command,
                "declared_features": crate_features.get(crate, []),
                "default_feature_members": crate_defaults.get(crate, []),
            }
        )

summary = {
    "generated_at_utc": datetime.now(timezone.utc).isoformat(),
    "total": len(rows),
    "passed": sum(1 for row in rows if row["status"] == "PASS"),
    "failed": sum(1 for row in rows if row["status"] == "FAIL"),
    "skipped": sum(1 for row in rows if row["status"] == "SKIPPED"),
}

report_json = {
    "summary": summary,
    "rows": rows,
}
report_json_path.write_text(json.dumps(report_json, indent=2), encoding="utf-8")

lines = []
lines.append("# Crate Compatibility Report")
lines.append("")
lines.append(f"- Generated (UTC): `{summary['generated_at_utc']}`")
lines.append(f"- Total rows: `{summary['total']}`")
lines.append(f"- Passed: `{summary['passed']}`")
lines.append(f"- Failed: `{summary['failed']}`")
lines.append(f"- Skipped: `{summary['skipped']}`")
lines.append("")
lines.append("| Crate | Feature Set | Status | Duration (s) |")
lines.append("|---|---|---|---:|")
for row in rows:
    lines.append(
        f"| `{row['crate']}` | `{row['feature_set']}` | {row['status']} | {row['duration_seconds']} |"
    )
lines.append("")
lines.append("## Declared Features")
lines.append("")
lines.append("| Crate | Features | Default Feature Members |")
lines.append("|---|---|---|")
for crate in sorted({row["crate"] for row in rows}):
    features = ", ".join(crate_features.get(crate, [])) or "-"
    defaults = ", ".join(crate_defaults.get(crate, [])) or "-"
    lines.append(f"| `{crate}` | `{features}` | `{defaults}` |")

report_md_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
print(f"wrote {report_md_path}")
print(f"wrote {report_json_path}")
PY

if [[ "${overall_fail}" -ne 0 && "${COMPAT_DRY_RUN}" != "1" && "${COMPAT_FAIL_ON_CHECK_FAILURE}" == "1" ]]; then
  echo "compatibility report generation detected one or more failed matrix checks" >&2
  exit 1
fi
