#!/usr/bin/env bash
# Verify that every branch-protection-required status check is green on a commit.
#
#   scripts/ci/check_required_checks.sh [options] <commit-sha>
#
# v1.35.1 shipped from a commit whose required checks were red because nothing
# between "the tag exists" and "the publish workflow runs" ever looked at the
# check state. This is that missing look-up, shared by the local release
# command (scripts/release-tag.sh) and the publish-workflow guards
# (scripts/ci/check_release_guard.sh).
#
# The required-context list is read from branch protection so it can never
# drift from the real merge gate. Reading protection needs an admin token; a
# workflow's GITHUB_TOKEN does not have one, so the fallback list applies there
# (default "CI Success", the aggregate job that needs every ci.yml job).
set -euo pipefail

REPO="${RELEASE_REPO:-stateset/stateset-icommerce}"
BRANCH="${RELEASE_PROTECTED_BRANCH:-master}"
FALLBACK_CONTEXTS="${RELEASE_REQUIRED_CHECKS_FALLBACK:-CI Success}"

usage() {
  cat <<'EOF'
Usage: check_required_checks.sh [options] <commit-sha>

Fails unless every required status check reported success on <commit-sha>.

Options:
  --repo OWNER/NAME     Repository to query (default: stateset/stateset-icommerce)
  --branch NAME         Protected branch whose required checks apply (default: master)
  --allow-red CONTEXT   Treat CONTEXT as passing even when it is red or missing.
                        Repeatable. Every waiver is logged loudly.
  -h, --help            Show this help message.

Environment:
  RELEASE_REPO                      Same as --repo.
  RELEASE_PROTECTED_BRANCH          Same as --branch.
  RELEASE_REQUIRED_CHECKS_FALLBACK  Comma-separated contexts to require when
                                    branch protection cannot be read
                                    (default: "CI Success").
EOF
}

sha=""
declare -a allow_red=()

while (($# > 0)); do
  case "$1" in
    --repo)
      [[ $# -ge 2 ]] || {
        echo "missing value for --repo" >&2
        exit 2
      }
      REPO="$2"
      shift 2
      ;;
    --branch)
      [[ $# -ge 2 ]] || {
        echo "missing value for --branch" >&2
        exit 2
      }
      BRANCH="$2"
      shift 2
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
      if [[ -n "$sha" ]]; then
        echo "unexpected extra argument: $1" >&2
        exit 2
      fi
      sha="$1"
      shift
      ;;
  esac
done

if [[ -z "$sha" ]]; then
  echo "error: a commit sha is required" >&2
  usage >&2
  exit 2
fi

if ! command -v gh >/dev/null 2>&1; then
  echo "error: the gh CLI is required to read check runs" >&2
  exit 2
fi

is_waived() {
  local candidate="$1" waived
  for waived in ${allow_red[@]+"${allow_red[@]}"}; do
    [[ "$waived" == "$candidate" ]] && return 0
  done
  return 1
}

required_raw=""
if ! required_raw="$(
  gh api "repos/${REPO}/branches/${BRANCH}/protection/required_status_checks" \
    --jq '.contexts[]' 2>/dev/null
)"; then
  required_raw=""
fi

if [[ -z "${required_raw//[[:space:]]/}" ]]; then
  echo "warning: branch protection for ${REPO}@${BRANCH} is unreadable (needs an admin token)." >&2
  echo "warning: falling back to required contexts: ${FALLBACK_CONTEXTS}" >&2
  required_raw="$(printf '%s' "$FALLBACK_CONTEXTS" | tr ',' '\n' | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//')"
fi

mapfile -t required < <(printf '%s\n' "$required_raw" | grep -v '^[[:space:]]*$' | sort -u)

if ((${#required[@]} == 0)); then
  echo "error: no required contexts resolved; refusing to declare ${sha} green" >&2
  exit 1
fi

check_runs=""
if ! check_runs="$(
  gh api --paginate "repos/${REPO}/commits/${sha}/check-runs" \
    --jq '.check_runs[] | [.name, .status, (.conclusion // "pending"), (.started_at // "")] | @tsv' 2>/dev/null
)"; then
  check_runs=""
fi

# Legacy commit statuses (non-Actions integrations) can satisfy a required
# context too; look them up so the gate does not report a false red.
commit_statuses=""
if ! commit_statuses="$(
  gh api --paginate "repos/${REPO}/commits/${sha}/status" \
    --jq '.statuses[] | [.context, "completed", .state, (.updated_at // "")] | @tsv' 2>/dev/null
)"; then
  commit_statuses=""
fi

declare -A observed_status=()
declare -A observed_conclusion=()
declare -A observed_time=()

while IFS=$'\t' read -r name status conclusion started; do
  [[ -z "${name:-}" ]] && continue
  # Re-runs report the same name more than once; the newest one is the one that
  # branch protection honours.
  if [[ -z "${observed_time[$name]:-}" || "$started" > "${observed_time[$name]}" ]]; then
    observed_time["$name"]="$started"
    observed_status["$name"]="$status"
    observed_conclusion["$name"]="$conclusion"
  fi
done < <(printf '%s\n%s\n' "$check_runs" "$commit_statuses")

declare -a red=()
declare -a missing=()
declare -a waived=()

for context in "${required[@]}"; do
  status="${observed_status[$context]:-}"
  conclusion="${observed_conclusion[$context]:-}"

  if [[ -z "$status" ]]; then
    if is_waived "$context"; then
      waived+=("${context} (no check reported)")
      printf 'WAIVED  %s (no check reported)\n' "$context"
    else
      missing+=("$context")
      printf 'MISSING %s\n' "$context"
    fi
    continue
  fi

  if [[ "$status" != "completed" ]]; then
    if is_waived "$context"; then
      waived+=("${context} (${status})")
      printf 'WAIVED  %s (%s)\n' "$context" "$status"
    else
      red+=("${context} (${status})")
      printf 'PENDING %s (%s)\n' "$context" "$status"
    fi
    continue
  fi

  if [[ "$conclusion" != "success" ]]; then
    if is_waived "$context"; then
      waived+=("${context} (${conclusion})")
      printf 'WAIVED  %s (%s)\n' "$context" "$conclusion"
    else
      red+=("${context} (${conclusion})")
      printf 'FAILED  %s (%s)\n' "$context" "$conclusion"
    fi
    continue
  fi

  printf 'ok      %s\n' "$context"
done

if ((${#waived[@]} > 0)); then
  {
    echo ''
    echo '################################################################'
    echo "# RED CHECKS WAIVED on ${sha}:"
    printf '#   - %s\n' "${waived[@]}"
    echo '# A waiver ships an unverified commit. Record why in the release notes.'
    echo '################################################################'
    echo ''
  } >&2
fi

if ((${#missing[@]} > 0 || ${#red[@]} > 0)); then
  echo "error: ${sha} is not releasable from ${REPO}@${BRANCH}" >&2
  for context in ${missing[@]+"${missing[@]}"}; do
    echo "  missing required check: ${context}" >&2
  done
  for context in ${red[@]+"${red[@]}"}; do
    echo "  required check not green: ${context}" >&2
  done
  exit 1
fi

echo "All ${#required[@]} required checks are green on ${sha}."
