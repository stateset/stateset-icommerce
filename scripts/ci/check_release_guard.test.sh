#!/usr/bin/env bash
# Tests for scripts/ci/check_release_guard.sh, the guard that every publish
# workflow runs before it can push a package to a registry.
#
# `git` and `gh` are stubbed on PATH so the cases run offline against a fake
# tag/check-run world; no ref in the real repository is read or written.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

STUB_BIN="${WORK_DIR}/bin"
mkdir -p "$STUB_BIN"

TAG_SHA="1111111111111111111111111111111111111111"
OTHER_SHA="2222222222222222222222222222222222222222"

cat >"${STUB_BIN}/git" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
printf 'git %s\n' "$*" >> "$STUB_LOG"

case "$1" in
  rev-parse)
    shift
    ref="${*: -1}"
    ref="${ref%^\{commit\}}"
    case "$ref" in
      HEAD) echo "$FAKE_HEAD_SHA" ;;
      refs/tags/*)
        tag="${ref#refs/tags/}"
        line="$(grep -E "^${tag} " "$FAKE_TAGS_FILE" 2>/dev/null || true)"
        [[ -n "$line" ]] || exit 1
        echo "${line#* }"
        ;;
      *) echo "$ref" ;;
    esac
    exit 0
    ;;
  fetch)
    for arg in "$@"; do
      case "$arg" in
        refs/tags/*)
          tag="${arg#refs/tags/}"
          tag="${tag%%:*}"
          grep -qE "^${tag} " "$FAKE_TAGS_FILE" || exit 1
          exit 0
          ;;
      esac
    done
    [[ "${FAKE_FETCH_BRANCH_FAILS:-0}" == "1" ]] && exit 1
    exit 0
    ;;
  merge-base)
    [[ "${FAKE_IS_ANCESTOR:-1}" == "1" ]] || exit 1
    exit 0
    ;;
  *)
    echo "unexpected git invocation: $*" >&2
    exit 64
    ;;
esac
STUB
chmod +x "${STUB_BIN}/git"

cat >"${STUB_BIN}/gh" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
printf 'gh %s\n' "$*" >> "$STUB_LOG"

for arg in "$@"; do
  case "$arg" in
    *required_status_checks)
      printf '%s\n' "${FAKE_REQUIRED_CONTEXTS:-}" | tr ',' '\n'
      exit 0
      ;;
    *check-runs)
      cat "$FAKE_CHECK_RUNS_FILE"
      exit 0
      ;;
    */status)
      exit 0
      ;;
  esac
done
echo "unexpected gh invocation: $*" >&2
exit 64
STUB
chmod +x "${STUB_BIN}/gh"

failures=0

fail() {
  echo "FAIL: $*" >&2
  failures=$((failures + 1))
}

reset_state() {
  STUB_LOG="${WORK_DIR}/stub.log"
  FAKE_TAGS_FILE="${WORK_DIR}/tags.txt"
  FAKE_CHECK_RUNS_FILE="${WORK_DIR}/check-runs.tsv"
  : >"$STUB_LOG"
  {
    printf 'v1.34.0 %s\n' "$TAG_SHA"
    printf 'cli-v1.34.0 %s\n' "$TAG_SHA"
    printf 'py-v1.34.0 %s\n' "$TAG_SHA"
  } >"$FAKE_TAGS_FILE"
  printf 'CI Success\tcompleted\tsuccess\t2026-09-08T10:00:00Z\n' >"$FAKE_CHECK_RUNS_FILE"
  export STUB_LOG FAKE_TAGS_FILE FAKE_CHECK_RUNS_FILE
  export FAKE_HEAD_SHA="$TAG_SHA"
  export FAKE_IS_ANCESTOR=1
  export FAKE_FETCH_BRANCH_FAILS=0
  export FAKE_REQUIRED_CONTEXTS='CI Success'
  export GITHUB_SHA="$TAG_SHA"
  export GITHUB_REF="refs/tags/cli-v1.34.0"
  unset RELEASE_VERSION RELEASE_GUARD_SKIP
}

run_guard() {
  set +e
  output="$(PATH="${STUB_BIN}:${PATH}" bash "${ROOT_DIR}/scripts/ci/check_release_guard.sh" "$@" 2>&1)"
  status=$?
  set -e
}

assert_status() {
  local expected="$1" message="$2"
  if [[ "$status" != "$expected" ]]; then
    fail "${message} (expected exit ${expected}, got ${status})"
    printf '%s\n' "$output" >&2
  fi
}

assert_output_contains() {
  local needle="$1" message="$2"
  if [[ "$output" != *"$needle"* ]]; then
    fail "${message} (missing: ${needle})"
    printf '%s\n' "$output" >&2
  fi
}

echo "==> passes when the tag is on master, green, and has both siblings"
reset_state
run_guard
assert_status 0 "a well-formed release must pass the guard"
assert_output_contains "Release guard passed for 1.34.0" "the guard must report the resolved version"

echo "==> derives the version from any of the three tag refs"
reset_state
GITHUB_REF="refs/tags/py-v1.34.0"
run_guard
assert_status 0 "a py-v tag must resolve to the same release version"

echo "==> rejects a commit that is not on master"
reset_state
FAKE_IS_ANCESTOR=0
run_guard
assert_status 1 "publishing from a commit that never landed must fail (the v1.33.0 failure)"
assert_output_contains "is not an ancestor of origin/master" "the ancestry failure must be explicit"

echo "==> rejects a release whose sibling tags are missing"
reset_state
printf 'v1.34.0 %s\n' "$TAG_SHA" >"$FAKE_TAGS_FILE"
run_guard
assert_status 1 "a v-only release must fail (the v1.31/v1.32 failure)"
assert_output_contains "tag cli-v1.34.0 does not exist" "the missing cli tag must be named"
assert_output_contains "tag py-v1.34.0 does not exist" "the missing py tag must be named"

echo "==> rejects sibling tags that point at different commits"
reset_state
printf 'py-v1.34.0 %s\n' "$OTHER_SHA" >>"$FAKE_TAGS_FILE"
sed -i '/^py-v1.34.0 1111/d' "$FAKE_TAGS_FILE"
run_guard
assert_status 1 "tags spread across commits must fail the guard"
assert_output_contains "points at ${OTHER_SHA}" "the divergent tag must be named"

echo "==> rejects a red required check"
reset_state
printf 'CI Success\tcompleted\tfailure\t2026-09-08T10:00:00Z\n' >"$FAKE_CHECK_RUNS_FILE"
run_guard
assert_status 1 "a red required check must block the publish (the v1.33.0 failure)"
assert_output_contains "required checks are not green" "the check failure must be summarised"

echo "==> reports every problem at once"
reset_state
FAKE_IS_ANCESTOR=0
printf 'v1.34.0 %s\n' "$TAG_SHA" >"$FAKE_TAGS_FILE"
printf 'CI Success\tcompleted\tfailure\t2026-09-08T10:00:00Z\n' >"$FAKE_CHECK_RUNS_FILE"
run_guard
assert_status 1 "a broken release must fail"
assert_output_contains "is not an ancestor" "ancestry problems must be reported"
assert_output_contains "does not exist" "missing tags must be reported in the same run"
assert_output_contains "required checks are not green" "check problems must be reported in the same run"

echo "==> skip_guards short-circuits loudly"
reset_state
FAKE_IS_ANCESTOR=0
export RELEASE_GUARD_SKIP=true
run_guard
assert_status 0 "the emergency escape hatch must not fail the workflow"
assert_output_contains "RELEASE GUARDS SKIPPED" "skipping must be logged loudly"
unset RELEASE_GUARD_SKIP

echo "==> refuses to run without a resolvable version"
reset_state
GITHUB_REF="refs/heads/master"
run_guard
assert_status 2 "a run with no release version must abort"

if ((failures > 0)); then
  echo "release guard tests failed: ${failures}" >&2
  exit 1
fi

echo "release guard tests passed."
