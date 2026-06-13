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
native_artifact_file="examples/tmp-native-artifact"
sdk_backup_file="$(mktemp)"
python_pyproject_backup_file="$(mktemp)"
publish_rust_workflow_backup_file="$(mktemp)"
original_pre_commit_mode="$(stat -c '%a' .husky/pre-commit)"
trap 'chmod "$original_pre_commit_mode" .husky/pre-commit >/dev/null 2>&1 || true; git rm --cached -f --quiet "$native_artifact_file" >/dev/null 2>&1 || true; cp "$sdk_backup_file" crates/stateset-sdk/src/lib.rs >/dev/null 2>&1 || true; cp "$python_pyproject_backup_file" bindings/python/pyproject.toml >/dev/null 2>&1 || true; cp "$publish_rust_workflow_backup_file" .github/workflows/publish-rust-crates.yml >/dev/null 2>&1 || true; rm -f "$native_artifact_file" "$version_file" "$stderr_file" "$sdk_backup_file" "$python_pyproject_backup_file" "$publish_rust_workflow_backup_file"' EXIT

cp crates/stateset-sdk/src/lib.rs "$sdk_backup_file"
cp bindings/python/pyproject.toml "$python_pyproject_backup_file"
cp .github/workflows/publish-rust-crates.yml "$publish_rust_workflow_backup_file"

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

chmod -x .husky/pre-commit

set +e
hook_failure_output="$(
  bash ./scripts/ci/check_release_hygiene.sh 2>&1
)"
hook_failure_status=$?
set -e

chmod "$original_pre_commit_mode" .husky/pre-commit

if (( hook_failure_status == 0 )); then
  echo "expected non-executable hook file to fail release hygiene" >&2
  exit 1
fi

assert_contains "$hook_failure_output" ".husky/pre-commit" "non-executable hook failure should name the offending file"
assert_contains "$hook_failure_output" "not executable" "non-executable hook failure should explain the fix"

cp /bin/true "$native_artifact_file"
git add "$native_artifact_file"

set +e
native_artifact_failure_output="$(
  bash ./scripts/ci/check_release_hygiene.sh 2>&1
)"
native_artifact_failure_status=$?
set -e

git rm --cached -f --quiet "$native_artifact_file" >/dev/null 2>&1 || true
rm -f "$native_artifact_file"

if (( native_artifact_failure_status == 0 )); then
  echo "expected tracked native artifact to fail release hygiene" >&2
  exit 1
fi

assert_contains "$native_artifact_failure_output" "Tracked native binaries detected in the repo." "tracked native artifact failure should explain the release risk"
assert_contains "$native_artifact_failure_output" "$native_artifact_file" "tracked native artifact failure should name the offending file"

node --input-type=module -e "
  import fs from 'node:fs';
  const file = 'crates/stateset-sdk/src/lib.rs';
  const original = fs.readFileSync(file, 'utf8');
  fs.writeFileSync(file, original.replace('stateset-sdk = \"${workspace_version}\"', 'stateset-sdk = \"0.9.8\"'));
"

set +e
sdk_doc_failure_output="$(
  bash ./scripts/ci/check_release_hygiene.sh 2>&1
)"
sdk_doc_failure_status=$?
set -e

cp "$sdk_backup_file" crates/stateset-sdk/src/lib.rs

if (( sdk_doc_failure_status == 0 )); then
  echo "expected stale SDK doc version to fail release hygiene" >&2
  exit 1
fi

assert_contains "$sdk_doc_failure_output" "crates/stateset-sdk/src/lib.rs" "stale SDK doc version failure should name the offending file"
assert_contains "$sdk_doc_failure_output" "stateset-sdk = \"${workspace_version}\"" "stale SDK doc version failure should explain the expected snippet"

node --input-type=module -e "
  import fs from 'node:fs';
  const file = 'bindings/python/pyproject.toml';
  const original = fs.readFileSync(file, 'utf8');
  fs.writeFileSync(
    file,
    original.replace(
      'Development Status :: 5 - Production/Stable',
      'Development Status :: 4 - Beta',
    ),
  );
"

set +e
python_classifier_failure_output="$(
  bash ./scripts/ci/check_release_hygiene.sh 2>&1
)"
python_classifier_failure_status=$?
set -e

cp "$python_pyproject_backup_file" bindings/python/pyproject.toml

if (( python_classifier_failure_status == 0 )); then
  echo "expected beta Python classifier to fail release hygiene" >&2
  exit 1
fi

assert_contains "$python_classifier_failure_output" "bindings/python/pyproject.toml" "beta Python classifier failure should name the offending file"
assert_contains "$python_classifier_failure_output" "Production/Stable" "beta Python classifier failure should mention the expected stable status"

node --input-type=module -e "
  import fs from 'node:fs';
  const file = '.github/workflows/publish-rust-crates.yml';
  const original = fs.readFileSync(file, 'utf8');
  const planted = original.replace('Version to release (e.g., ${workspace_version})', 'Version to release (e.g., 0.9.5)');
  if (planted === original) {
    console.error('fixture setup failed: could not plant stale workflow example version');
    process.exit(1);
  }
  fs.writeFileSync(file, planted);
"

set +e
workflow_example_failure_output="$(
  bash ./scripts/ci/check_release_hygiene.sh 2>&1
)"
workflow_example_failure_status=$?
set -e

cp "$publish_rust_workflow_backup_file" .github/workflows/publish-rust-crates.yml

if (( workflow_example_failure_status == 0 )); then
  echo "expected stale workflow example version to fail release hygiene" >&2
  exit 1
fi

assert_contains "$workflow_example_failure_output" ".github/workflows/publish-rust-crates.yml" "stale workflow example failure should name the offending file"
assert_contains "$workflow_example_failure_output" "Version to release (e.g., ${workspace_version})" "stale workflow example failure should mention the expected example"

echo "check_release_hygiene tests passed."
