#!/usr/bin/env bash
# Cut a release: verify every precondition, then create and push the three
# sibling tags in one atomic push.
#
#   scripts/release-tag.sh <version> [--dry-run] [--allow-red <context>]
#   npm run release:tag -- 1.34.0
#
# Why this exists (each rule is a shipped incident):
#   * v1.31.0 and v1.32.0 were pushed with only the `v*` tag, so npm and PyPI
#     stayed on 1.30.0 while crates.io moved on. The three tags are now created
#     and pushed together or not at all.
#   * v1.33.0 was tagged from a non-master branch whose required checks were
#     red. HEAD must now equal origin/master and every branch-protection
#     required context must be green on that exact commit.
#   * Release hygiene (version sync across ~20 surfaces) is re-run here so a
#     half-bumped tree cannot be tagged.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

REMOTE="${RELEASE_REMOTE:-origin}"
BRANCH="${RELEASE_PROTECTED_BRANCH:-master}"
HYGIENE_SCRIPT="${RELEASE_TAG_HYGIENE_SCRIPT:-${ROOT_DIR}/scripts/ci/check_release_hygiene.sh}"
CHECKS_SCRIPT="${RELEASE_TAG_CHECKS_SCRIPT:-${ROOT_DIR}/scripts/ci/check_required_checks.sh}"

usage() {
  cat <<'EOF'
Usage: release-tag.sh <version> [options]

Creates and pushes the annotated release tags v<version>, cli-v<version> and
py-v<version> in a single push, after verifying that:

  1. the working tree is clean;
  2. HEAD equals <remote>/<branch> after a fetch;
  3. scripts/ci/check_release_hygiene.sh passes for <version>;
  4. every branch-protection required check is green on HEAD;
  5. none of the three tags already points at a different commit.

Arguments:
  version               1.34.0 or v1.34.0

Options:
  --dry-run             Run every check, then report what would be tagged and
                        pushed without touching any ref.
  --allow-red CONTEXT   Accept a named red/missing required check. Repeatable.
                        Logged loudly; use only for a known-flaky context.
  -h, --help            Show this help message.

Environment:
  RELEASE_REMOTE              Git remote to fetch and push (default: origin).
  RELEASE_PROTECTED_BRANCH    Branch HEAD must equal (default: master).
  RELEASE_REPO                OWNER/NAME used for the check-run query.
EOF
}

raw_version=""
dry_run=0
declare -a allow_red=()

while (($# > 0)); do
  case "$1" in
    --dry-run)
      dry_run=1
      shift
      ;;
    --allow-red)
      [[ $# -ge 2 ]] || {
        echo "missing value for --allow-red" >&2
        exit 2
      }
      allow_red+=("$2")
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    -*)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      if [[ -n "$raw_version" ]]; then
        echo "unexpected extra argument: $1" >&2
        exit 2
      fi
      raw_version="$1"
      shift
      ;;
  esac
done

if [[ -z "$raw_version" ]]; then
  echo "error: a release version is required" >&2
  usage >&2
  exit 2
fi

version="${raw_version#v}"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "error: release version must be plain SemVer, got '${raw_version}'" >&2
  exit 2
fi

cd "$ROOT_DIR"

tags=("v${version}" "cli-v${version}" "py-v${version}")

echo "==> Release ${version} from ${REMOTE}/${BRANCH}"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "error: working tree is dirty; commit or stash before tagging" >&2
  git status --short >&2
  exit 1
fi
echo "    working tree clean"

echo "==> Fetching ${REMOTE}"
git fetch --quiet --tags "$REMOTE" "$BRANCH"

head_sha="$(git rev-parse HEAD)"
remote_sha="$(git rev-parse "${REMOTE}/${BRANCH}")"
if [[ "$head_sha" != "$remote_sha" ]]; then
  echo "error: HEAD (${head_sha}) is not ${REMOTE}/${BRANCH} (${remote_sha})" >&2
  echo "       releases are cut from the protected branch only" >&2
  exit 1
fi
echo "    HEAD == ${REMOTE}/${BRANCH} (${head_sha})"

echo "==> Release hygiene"
bash "$HYGIENE_SCRIPT" "$version"

echo "==> Required checks on ${head_sha}"
checks_args=("$head_sha" "--branch" "$BRANCH")
for context in ${allow_red[@]+"${allow_red[@]}"}; do
  checks_args+=("--allow-red" "$context")
done
bash "$CHECKS_SCRIPT" "${checks_args[@]}"

echo "==> Tag preflight"
declare -a tags_to_create=()
for tag in "${tags[@]}"; do
  existing="$(git rev-parse -q --verify "refs/tags/${tag}^{commit}" || true)"
  if [[ -z "$existing" ]]; then
    tags_to_create+=("$tag")
    echo "    ${tag}: will create"
    continue
  fi
  if [[ "$existing" != "$head_sha" ]]; then
    echo "error: ${tag} already exists and points at ${existing}, not ${head_sha}" >&2
    echo "       delete the tag deliberately (locally and on ${REMOTE}) before re-cutting" >&2
    exit 1
  fi
  # A partially pushed release (the v1.31/v1.32 failure mode) is resumable:
  # the existing tag already points at this commit, so keep it and push it.
  echo "    ${tag}: already at ${head_sha}, will re-push"
done

if ((dry_run != 0)); then
  echo ''
  echo "DRY RUN: no refs were created or pushed."
  echo "  would create: ${tags_to_create[*]:-<none>}"
  echo "  would push:   git push --atomic ${REMOTE} refs/tags/${tags[0]} refs/tags/${tags[1]} refs/tags/${tags[2]}"
  exit 0
fi

created=()
cleanup_created_tags() {
  local status=$?
  if ((status != 0)) && ((${#created[@]} > 0)); then
    echo "error: tagging failed; removing locally created tags: ${created[*]}" >&2
    for tag in "${created[@]}"; do
      git tag -d "$tag" >/dev/null 2>&1 || true
    done
  fi
  return "$status"
}
trap cleanup_created_tags EXIT

echo "==> Creating annotated tags"
for tag in ${tags_to_create[@]+"${tags_to_create[@]}"}; do
  git tag -a "$tag" -m "stateset-icommerce ${version}"
  created+=("$tag")
  echo "    ${tag}"
done

echo "==> Pushing all three tags in one atomic push"
# --atomic makes the header's "together or not at all" literally true: without
# it the remote applies each ref independently, so a rejection of one tag (the
# v1.31/v1.32 failure mode) still leaves the others published.
git push --atomic "$REMOTE" "refs/tags/${tags[0]}" "refs/tags/${tags[1]}" "refs/tags/${tags[2]}"

trap - EXIT

cat <<EOF

Release ${version} tagged and pushed:
  ${tags[0]}      -> Publish Rust Crates, release-sign
  ${tags[1]}  -> Publish CLI (@stateset/embedded, @stateset/cli, create-stateset-app)
  ${tags[2]}   -> Publish Python

Next: watch the three publish workflows, then confirm the registries with
  .github/workflows/release-consistency.yml (Registry drift).
EOF
