#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

extract_with_regex() {
  local pattern="$1"
  local file="$2"
  local sed_expr="$3"
  { grep -E "$pattern" "$file" | head -n1 | sed -E "$sed_expr"; } || true
}

assert_eq() {
  local expected="$1"
  local actual="$2"
  local message="$3"
  if [[ "$expected" != "$actual" ]]; then
    echo "assertion failed: ${message}" >&2
    echo "expected: ${expected}" >&2
    echo "actual:   ${actual}" >&2
    exit 1
  fi
}

assert_contains() {
  local haystack="$1"
  local needle="$2"
  local message="$3"
  if [[ "$haystack" != *"$needle"* ]]; then
    echo "assertion failed: ${message}" >&2
    echo "missing substring: ${needle}" >&2
    echo "haystack: ${haystack}" >&2
    exit 1
  fi
}

workspace_version="$(extract_with_regex '^[[:space:]]*version = "[0-9]+\.[0-9]+\.[0-9]+"' Cargo.toml 's/^[^"]*"([^"]+)".*$/\1/')"
if [[ -z "$workspace_version" ]]; then
  echo "failed to determine workspace version" >&2
  exit 1
fi

version_file="$(mktemp)"
stderr_file="$(mktemp)"
trap 'rm -f "$version_file" "$stderr_file"' EXIT

printed_version="$(
  RELEASE_VERSION="java-v${workspace_version}" \
    bash ./scripts/ci/check_release_hygiene.sh --print-version 2>"$stderr_file"
)"
assert_eq "$workspace_version" "$printed_version" "prefixed java tag should normalize to the workspace version"
assert_contains "$(cat "$stderr_file")" "Release hygiene checks passed for ${workspace_version}." "print-version mode should still emit a success message to stderr"

RELEASE_VERSION="cli-v${workspace_version}" \
  bash ./scripts/ci/check_release_hygiene.sh --github-output "$version_file" >/dev/null
assert_eq "version=${workspace_version}" "$(cat "$version_file")" "--github-output should receive the normalized version"

set +e
failure_output="$(
  RELEASE_VERSION="ruby-v999.999.999" \
    bash ./scripts/ci/check_release_hygiene.sh 2>&1
)"
failure_status=$?
set -e

if (( failure_status == 0 )); then
  echo "expected mismatched release version to fail" >&2
  exit 1
fi

assert_contains "$failure_output" "does not match workspace version" "mismatched release version should explain the failure"

echo "check_release_hygiene tests passed."
