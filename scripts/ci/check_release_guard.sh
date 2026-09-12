#!/usr/bin/env bash
# Publish-workflow guard: refuse to publish anything from a commit that is not
# a released state of the protected branch.
#
#   scripts/ci/check_release_guard.sh [--version 1.35.1] [--sha <sha>]
#
# Every publish workflow (crates.io, npm, PyPI, sigstore signing) runs this
# first. It fails unless all four hold:
#
#   1. the tagged commit is an ancestor of origin/master (v1.35.1 was published
#      from a branch commit that never landed);
#   2. every branch-protection required check is green on that commit
#      (v1.35.1's required check was red);
#   3. the three sibling tags v/cli-v/py-v all exist (v1.31.0 and v1.32.0
#      shipped with only `v*`, leaving npm and PyPI a release behind);
#   4. all three point at that same commit.
#
# Emergency escape hatch: the workflows expose a `skip_guards` dispatch input
# which sets RELEASE_GUARD_SKIP=true. It is logged loudly and never silent.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

REPO="${RELEASE_REPO:-stateset/stateset-icommerce}"
REMOTE="${RELEASE_REMOTE:-origin}"
BRANCH="${RELEASE_PROTECTED_BRANCH:-master}"
CHECKS_SCRIPT="${RELEASE_GUARD_CHECKS_SCRIPT:-${ROOT_DIR}/scripts/ci/check_required_checks.sh}"

usage() {
  cat <<'EOF'
Usage: check_release_guard.sh [--version VERSION] [--sha SHA]

Fails unless the commit being published is an ancestor of the protected branch,
its required checks are green, and the three sibling release tags all exist on
it.

Options:
  --version VERSION   Release version (1.35.1, v1.35.1, cli-v1.35.1, py-v1.35.1).
                      Defaults to RELEASE_VERSION, then the pushed tag ref.
  --sha SHA           Commit under test. Defaults to GITHUB_SHA, then HEAD.
  -h, --help          Show this help message.

Environment:
  RELEASE_GUARD_SKIP=true   Skip every check (emergency dispatch input), loudly.
  RELEASE_REPO              OWNER/NAME for the check-run query.
  RELEASE_REMOTE            Git remote to fetch (default: origin).
  RELEASE_PROTECTED_BRANCH  Branch the commit must be an ancestor of.
EOF
}

raw_version="${RELEASE_VERSION:-}"
sha="${GITHUB_SHA:-}"

while (($# > 0)); do
  case "$1" in
    --version)
      [[ $# -ge 2 ]] || {
        echo "missing value for --version" >&2
        exit 2
      }
      raw_version="$2"
      shift 2
      ;;
    --sha)
      [[ $# -ge 2 ]] || {
        echo "missing value for --sha" >&2
        exit 2
      }
      sha="$2"
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

if [[ "${RELEASE_GUARD_SKIP:-false}" == "true" ]]; then
  {
    echo '################################################################'
    echo '# RELEASE GUARDS SKIPPED via the skip_guards dispatch input.'
    echo '# This publish is UNVERIFIED: the commit may not be on master,'
    echo '# its checks may be red, and its sibling tags may not exist.'
    echo '################################################################'
  } >&2
  echo '::warning::Release guards skipped via skip_guards'
  exit 0
fi

if [[ -z "$raw_version" && "${GITHUB_REF:-}" == refs/tags/* ]]; then
  raw_version="${GITHUB_REF#refs/tags/}"
fi

version="$raw_version"
version="${version#cli-}"
version="${version#py-}"
version="${version#v}"

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "::error::Release guard could not resolve a SemVer version (got '${raw_version}')" >&2
  exit 2
fi

cd "$ROOT_DIR"

if [[ -z "$sha" ]]; then
  sha="$(git rev-parse HEAD)"
fi
sha="$(git rev-parse "${sha}^{commit}")"

echo "==> Release guard for ${version} at ${sha}"

problems=()

echo "==> Ancestry: ${sha} must be on ${REMOTE}/${BRANCH}"
if git fetch --quiet --no-tags "$REMOTE" "$BRANCH"; then
  if git merge-base --is-ancestor "$sha" FETCH_HEAD; then
    echo "    ok: commit is an ancestor of ${REMOTE}/${BRANCH}"
  else
    problems+=("commit ${sha} is not an ancestor of ${REMOTE}/${BRANCH}")
    echo "    FAILED"
  fi
else
  problems+=("could not fetch ${REMOTE}/${BRANCH} to verify ancestry")
  echo "    FAILED (fetch)"
fi

echo "==> Sibling tags: v/cli-v/py-v${version} must all point at ${sha}"
for tag in "v${version}" "cli-v${version}" "py-v${version}"; do
  if ! git fetch --quiet --force "$REMOTE" "refs/tags/${tag}:refs/tags/${tag}" 2>/dev/null; then
    problems+=("tag ${tag} does not exist on ${REMOTE}")
    echo "    MISSING ${tag}"
    continue
  fi
  resolved="$(git rev-parse -q --verify "refs/tags/${tag}^{commit}" || true)"
  if [[ "$resolved" != "$sha" ]]; then
    problems+=("tag ${tag} points at ${resolved:-<unresolvable>}, not ${sha}")
    echo "    MISMATCH ${tag} -> ${resolved:-<unresolvable>}"
    continue
  fi
  echo "    ok ${tag}"
done

echo "==> Required checks on ${sha}"
if ! bash "$CHECKS_SCRIPT" "$sha" --repo "$REPO" --branch "$BRANCH"; then
  problems+=("required checks are not green on ${sha}")
fi

if ((${#problems[@]} > 0)); then
  echo "::error::Release guard failed for ${version}" >&2
  for problem in "${problems[@]}"; do
    echo "  - ${problem}" >&2
  done
  echo '' >&2
  echo "Fix the release rather than the guard: cut tags with 'npm run release:tag -- ${version}'." >&2
  echo "For a genuine emergency, re-run via workflow_dispatch with skip_guards: true." >&2
  exit 1
fi

echo "Release guard passed for ${version} at ${sha}."
