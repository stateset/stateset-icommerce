#!/usr/bin/env bash
# Precondition tests for scripts/release-tag.sh and
# scripts/ci/check_required_checks.sh.
#
# `git` and `gh` are stubbed on PATH, so every case runs offline and no ref is
# ever created in the real repository. The stubs record each invocation so the
# tests can assert that a dry run pushes nothing and that a real run pushes all
# three tags in exactly one push.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

STUB_BIN="${WORK_DIR}/bin"
mkdir -p "$STUB_BIN"

HEAD_SHA="1111111111111111111111111111111111111111"
OTHER_SHA="2222222222222222222222222222222222222222"

cat >"${STUB_BIN}/git" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
printf 'git %s\n' "$*" >> "$STUB_LOG"

case "$1" in
  status)
    [[ "${FAKE_DIRTY:-0}" == "1" ]] && echo " M cli/package.json"
    exit 0
    ;;
  fetch)
    exit 0
    ;;
  rev-parse)
    shift
    case "$*" in
      HEAD) echo "$FAKE_HEAD_SHA" ;;
      *"/${FAKE_BRANCH:-master}") echo "$FAKE_REMOTE_SHA" ;;
      *"refs/tags/"*)
        ref="${*: -1}"
        tag="${ref#refs/tags/}"
        tag="${tag%^\{commit\}}"
        line="$(grep -E "^${tag} " "$FAKE_TAGS_FILE" 2>/dev/null || true)"
        if [[ -z "$line" ]]; then
          exit 1
        fi
        echo "${line#* }"
        ;;
      *)
        echo "unexpected git rev-parse: $*" >&2
        exit 64
        ;;
    esac
    exit 0
    ;;
  tag)
    if [[ "${2:-}" == "-a" ]]; then
      printf '%s %s\n' "$3" "$FAKE_HEAD_SHA" >> "$FAKE_TAGS_FILE"
      exit 0
    fi
    if [[ "${2:-}" == "-d" ]]; then
      exit 0
    fi
    exit 0
    ;;
  push)
    [[ "${FAKE_PUSH_FAILS:-0}" == "1" ]] && exit 1
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
      [[ "${FAKE_PROTECTION_READABLE:-1}" == "1" ]] || exit 1
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

cat >"${WORK_DIR}/hygiene-pass.sh" <<'STUB'
#!/usr/bin/env bash
echo "Release hygiene checks passed for $1."
STUB
chmod +x "${WORK_DIR}/hygiene-pass.sh"

cat >"${WORK_DIR}/hygiene-fail.sh" <<'STUB'
#!/usr/bin/env bash
echo "::error::version sync failed" >&2
exit 1
STUB
chmod +x "${WORK_DIR}/hygiene-fail.sh"

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
  : >"$FAKE_TAGS_FILE"
  {
    printf 'Formatting\tcompleted\tsuccess\t2026-09-08T10:00:00Z\n'
    printf 'CLI Tests\tcompleted\tsuccess\t2026-09-08T10:00:00Z\n'
    printf 'Code Coverage\tcompleted\tsuccess\t2026-09-08T10:00:00Z\n'
  } >"$FAKE_CHECK_RUNS_FILE"
  export STUB_LOG FAKE_TAGS_FILE FAKE_CHECK_RUNS_FILE
  export FAKE_DIRTY=0
  export FAKE_HEAD_SHA="$HEAD_SHA"
  export FAKE_REMOTE_SHA="$HEAD_SHA"
  export FAKE_BRANCH=master
  export FAKE_PROTECTION_READABLE=1
  export FAKE_REQUIRED_CONTEXTS='Formatting,CLI Tests,Code Coverage'
  export FAKE_PUSH_FAILS=0
  export RELEASE_TAG_HYGIENE_SCRIPT="${WORK_DIR}/hygiene-pass.sh"
  export RELEASE_REPO="stateset/stateset-icommerce"
  export RELEASE_REMOTE=origin
}

# Runs release-tag.sh with the stubs first on PATH. Sets `status` and `output`.
run_release_tag() {
  set +e
  output="$(PATH="${STUB_BIN}:${PATH}" bash "${ROOT_DIR}/scripts/release-tag.sh" "$@" 2>&1)"
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

assert_log_contains() {
  local needle="$1" message="$2"
  if ! grep -Fq -- "$needle" "$STUB_LOG"; then
    fail "${message} (missing from stub log: ${needle})"
    cat "$STUB_LOG" >&2
  fi
}

assert_log_missing() {
  local needle="$1" message="$2"
  if grep -Fq -- "$needle" "$STUB_LOG"; then
    fail "${message} (unexpected in stub log: ${needle})"
    cat "$STUB_LOG" >&2
  fi
}

echo "==> rejects a non-SemVer version"
reset_state
run_release_tag "1.34"
assert_status 2 "a malformed version must be rejected before anything runs"
assert_output_contains "plain SemVer" "the version error must name the expected format"

echo "==> rejects a dirty working tree"
reset_state
FAKE_DIRTY=1
run_release_tag "1.34.0" --dry-run
assert_status 1 "a dirty tree must abort the release"
assert_output_contains "working tree is dirty" "the dirty-tree error must be explicit"

echo "==> rejects HEAD that is not origin/master"
reset_state
FAKE_REMOTE_SHA="$OTHER_SHA"
run_release_tag "1.34.0" --dry-run
assert_status 1 "tagging off the protected branch must abort (the v1.33.0 failure)"
assert_output_contains "is not origin/master" "the branch error must name the remote branch"

echo "==> rejects a failing release-hygiene gate"
reset_state
RELEASE_TAG_HYGIENE_SCRIPT="${WORK_DIR}/hygiene-fail.sh"
run_release_tag "1.34.0" --dry-run
assert_status 1 "a failing hygiene gate must abort the release"

echo "==> rejects a red required check"
reset_state
printf 'Formatting\tcompleted\tfailure\t2026-09-08T11:00:00Z\n' >>"$FAKE_CHECK_RUNS_FILE"
run_release_tag "1.34.0" --dry-run
assert_status 1 "a red required check must abort the release (the v1.33.0 failure)"
assert_output_contains "required check not green: Formatting" "the red check must be named"

echo "==> rejects a required check that never reported"
reset_state
printf 'Formatting\tcompleted\tsuccess\t2026-09-08T10:00:00Z\n' >"$FAKE_CHECK_RUNS_FILE"
printf 'CLI Tests\tcompleted\tsuccess\t2026-09-08T10:00:00Z\n' >>"$FAKE_CHECK_RUNS_FILE"
run_release_tag "1.34.0" --dry-run
assert_status 1 "a required check with no run must abort the release"
assert_output_contains "missing required check: Code Coverage" "the missing check must be named"

echo "==> honours the newest run of a re-run check"
reset_state
printf 'Formatting\tcompleted\tfailure\t2026-09-08T09:00:00Z\n' >>"$FAKE_CHECK_RUNS_FILE"
run_release_tag "1.34.0" --dry-run
assert_status 0 "an older failed attempt must not mask the newer green re-run"

echo "==> --allow-red waives a named context, loudly"
reset_state
printf 'Formatting\tcompleted\tfailure\t2026-09-08T11:00:00Z\n' >>"$FAKE_CHECK_RUNS_FILE"
run_release_tag "1.34.0" --dry-run --allow-red "Formatting"
assert_status 0 "an explicitly waived context must not block the release"
assert_output_contains "RED CHECKS WAIVED" "a waiver must be logged loudly"
assert_output_contains "Formatting (failure)" "the waiver must name the context and its conclusion"

echo "==> --allow-red does not waive every other context"
reset_state
printf 'CLI Tests\tcompleted\tfailure\t2026-09-08T11:00:00Z\n' >>"$FAKE_CHECK_RUNS_FILE"
run_release_tag "1.34.0" --dry-run --allow-red "Formatting"
assert_status 1 "a waiver must apply only to the named context"

echo "==> a dry run creates and pushes nothing"
reset_state
run_release_tag "1.34.0" --dry-run
assert_status 0 "the happy path must pass every precondition"
assert_output_contains "DRY RUN" "a dry run must say so"
assert_log_missing "git tag -a" "a dry run must not create tags"
assert_log_missing "git push" "a dry run must not push"

echo "==> a real run pushes all three tags in one push"
reset_state
run_release_tag "1.34.0"
assert_status 0 "the happy path must succeed"
assert_log_contains "git tag -a v1.34.0 -m" "the v tag must be annotated"
assert_log_contains "git tag -a cli-v1.34.0 -m" "the cli-v tag must be annotated"
assert_log_contains "git tag -a py-v1.34.0 -m" "the py-v tag must be annotated"
assert_log_contains "git push --atomic origin refs/tags/v1.34.0 refs/tags/cli-v1.34.0 refs/tags/py-v1.34.0" \
  "all three tags must go out in a single push (the v1.31/v1.32 failure)"
push_count="$(grep -c '^git push' "$STUB_LOG" || true)"
if [[ "$push_count" != "1" ]]; then
  fail "expected exactly one push, got ${push_count}"
fi

echo "==> refuses a sibling tag that points somewhere else"
reset_state
printf 'cli-v1.34.0 %s\n' "$OTHER_SHA" >"$FAKE_TAGS_FILE"
run_release_tag "1.34.0" --dry-run
assert_status 1 "an existing tag on another commit must abort the release"
assert_output_contains "already exists and points at" "the conflicting tag must be explained"

echo "==> resumes a partially pushed release"
reset_state
printf 'v1.34.0 %s\n' "$HEAD_SHA" >"$FAKE_TAGS_FILE"
run_release_tag "1.34.0"
assert_status 0 "a tag already on HEAD must be re-pushed rather than block the release"
assert_log_missing "git tag -a v1.34.0" "the existing tag must not be recreated"
assert_log_contains "git tag -a cli-v1.34.0 -m" "the missing sibling tags must still be created"
assert_log_contains "git push --atomic origin refs/tags/v1.34.0 refs/tags/cli-v1.34.0 refs/tags/py-v1.34.0" \
  "the resumed release must still push all three tags together"

echo "==> a failed push leaves no dangling local tags"
reset_state
FAKE_PUSH_FAILS=1
run_release_tag "1.34.0"
assert_status 1 "a failed push must fail the command"
assert_log_contains "git tag -d" "locally created tags must be rolled back after a failed push"

echo "==> the check gate falls back when branch protection is unreadable"
reset_state
FAKE_PROTECTION_READABLE=0
printf 'CI Success\tcompleted\tsuccess\t2026-09-08T10:00:00Z\n' >>"$FAKE_CHECK_RUNS_FILE"
run_release_tag "1.34.0" --dry-run
assert_status 0 "an unreadable protection API must fall back to the aggregate context"
assert_output_contains "falling back to required contexts" "the fallback must be announced"

echo "==> the fallback still fails when the aggregate check is red"
reset_state
FAKE_PROTECTION_READABLE=0
printf 'CI Success\tcompleted\tfailure\t2026-09-08T10:00:00Z\n' >>"$FAKE_CHECK_RUNS_FILE"
run_release_tag "1.34.0" --dry-run
assert_status 1 "the fallback context must still gate the release"

if ((failures > 0)); then
  echo "release-tag precondition tests failed: ${failures}" >&2
  exit 1
fi

echo "release-tag precondition tests passed."
