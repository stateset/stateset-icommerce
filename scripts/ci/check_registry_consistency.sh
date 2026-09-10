#!/usr/bin/env bash
# Compare the latest `v*` tag against every public registry we publish to.
#
#   scripts/ci/check_registry_consistency.sh [--version 1.35.0] [--report PATH]
#
# v1.31.0 and v1.32.0 were tagged with only `v*`, so crates.io moved to 1.32.0
# while npm and PyPI silently stayed on 1.30.0 for two releases. Nothing noticed
# until a user did. This is the thing that notices: it runs daily, compares the
# tag against what each registry actually serves, and exits non-zero on drift so
# the workflow can open the "Registry drift" issue.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REPORT_PATH="${REGISTRY_CONSISTENCY_REPORT:-${ROOT_DIR}/artifacts/registry-consistency/report.md}"
USER_AGENT='stateset-icommerce release-consistency (https://github.com/stateset/stateset-icommerce)'

usage() {
  cat <<'EOF'
Usage: check_registry_consistency.sh [options]

Fails when a published registry version does not match the latest v* tag.

Options:
  --version VERSION   Expected version (default: latest v* tag reachable here).
  --report PATH       Markdown report destination
                      (default: artifacts/registry-consistency/report.md).
  -h, --help          Show this help message.
EOF
}

expected=""

while (($# > 0)); do
  case "$1" in
    --version)
      [[ $# -ge 2 ]] || {
        echo "missing value for --version" >&2
        exit 2
      }
      expected="${2#v}"
      shift 2
      ;;
    --report)
      [[ $# -ge 2 ]] || {
        echo "missing value for --report" >&2
        exit 2
      }
      REPORT_PATH="$2"
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

cd "$ROOT_DIR"

if [[ -z "$expected" ]]; then
  expected="$(git tag --list 'v[0-9]*' --sort=-v:refname | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' | head -n1 || true)"
  expected="${expected#v}"
fi

if [[ ! "$expected" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "::error::Could not determine the expected release version (no v* tag found?)" >&2
  exit 2
fi

# Read a string field out of a JSON API response. Registries answer with an
# error page often enough that "unreachable" has to be distinguishable from
# "drifted": a failure here is reported as unreachable, never as a match.
json_field() {
  local url="$1" path="$2" body
  body="$(curl -fsSL --max-time 30 --retry 3 --retry-delay 5 -H "User-Agent: ${USER_AGENT}" "$url" 2>/dev/null)" || return 1
  printf '%s' "$body" | node -e '
    let raw = "";
    process.stdin.on("data", (chunk) => { raw += chunk; });
    process.stdin.on("end", () => {
      try {
        let value = JSON.parse(raw);
        for (const key of process.argv[1].split(".")) {
          value = value === null || value === undefined ? undefined : value[key];
        }
        if (typeof value !== "string") { process.exit(1); }
        process.stdout.write(value);
      } catch {
        process.exit(1);
      }
    });
  ' "$path"
}

npm_version() {
  local package="$1" attempt output
  for attempt in 1 2 3; do
    if output="$(npm view "$package" version 2>/dev/null)" && [[ -n "$output" ]]; then
      printf '%s' "$output"
      return 0
    fi
    sleep $((attempt * 5))
  done
  return 1
}

declare -a rows=()
declare -a drifted=()
declare -a unreachable=()

record() {
  local surface="$1" published="$2" state="$3"
  rows+=("| ${surface} | ${expected} | ${published} | ${state} |")
}

check() {
  local surface="$1" published="$2" ok="$3"
  if ((ok != 0)); then
    unreachable+=("$surface")
    record "$surface" "unreachable" "unreachable"
    echo "UNREACHABLE ${surface}"
    return
  fi
  if [[ "$published" != "$expected" ]]; then
    drifted+=("${surface} serves ${published}, tag is ${expected}")
    record "$surface" "$published" "**drifted**"
    echo "DRIFT       ${surface}: ${published} != ${expected}"
    return
  fi
  record "$surface" "$published" "ok"
  echo "ok          ${surface}: ${published}"
}

echo "==> Expected release version: ${expected}"

for package in '@stateset/cli' '@stateset/embedded' 'create-stateset-app'; do
  published=""
  status=0
  published="$(npm_version "$package")" || status=$?
  check "npm ${package}" "$published" "$status"
done

published=""
status=0
published="$(json_field 'https://pypi.org/pypi/stateset-embedded/json' 'info.version')" || status=$?
check "PyPI stateset-embedded" "$published" "$status"

published=""
status=0
published="$(json_field 'https://crates.io/api/v1/crates/stateset-embedded' 'crate.max_version')" || status=$?
check "crates.io stateset-embedded" "$published" "$status"

mkdir -p "$(dirname "$REPORT_PATH")"
{
  echo "# Registry drift"
  echo ''
  echo "Latest \`v*\` tag: **${expected}**"
  echo ''
  echo '| Surface | Expected | Published | Status |'
  echo '| --- | --- | --- | --- |'
  printf '%s\n' "${rows[@]}"
  echo ''
  if ((${#drifted[@]} == 0 && ${#unreachable[@]} == 0)); then
    echo "All registries serve ${expected}."
  else
    for entry in ${drifted[@]+"${drifted[@]}"}; do
      echo "- Drift: ${entry}"
    done
    for entry in ${unreachable[@]+"${unreachable[@]}"}; do
      echo "- Unreachable: ${entry}"
    done
    cat <<'GUIDANCE'

A missing sibling tag is the usual cause: `v*` publishes crates.io, `cli-v*`
publishes npm and `py-v*` publishes PyPI. Re-cut the release with
`npm run release:tag -- <version>`, which pushes all three in one push.
GUIDANCE
  fi
  echo ''
  echo "_Generated $(date -u +'%Y-%m-%dT%H:%M:%SZ')_"
} >"$REPORT_PATH"

echo "==> Report written to ${REPORT_PATH}"

if ((${#drifted[@]} > 0 || ${#unreachable[@]} > 0)); then
  echo "::error::Registry drift detected against ${expected}" >&2
  exit 1
fi

echo "All registries serve ${expected}."
