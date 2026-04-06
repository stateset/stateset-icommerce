#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

usage() {
  cat <<'EOF'
Usage: check_release_hygiene.sh [--github-output PATH] [VERSION_OR_TAG]

Validates repo-wide version sync plus release metadata hygiene.

Arguments:
  VERSION_OR_TAG   Optional semantic version or tag name such as:
                   0.9.3, v0.9.3, cli-v0.9.3, py-v0.9.3, java-v0.9.3,
                   php-v0.9.3, ruby-v0.9.3

Options:
  --github-output PATH   Write version=<normalized-version> to the given file.
  --print-version        Print the normalized version to stdout.
  -h, --help             Show this help message.
EOF
}

extract_with_regex() {
  local pattern="$1"
  local file="$2"
  local sed_expr="$3"
  { grep -E "$pattern" "$file" | head -n1 | sed -E "$sed_expr"; } || true
}

normalize_release_version() {
  local raw="$1"
  case "$raw" in
    v[0-9]*)
      printf '%s\n' "${raw#v}"
      ;;
    cli-v[0-9]*)
      printf '%s\n' "${raw#cli-v}"
      ;;
    py-v[0-9]*)
      printf '%s\n' "${raw#py-v}"
      ;;
    java-v[0-9]*)
      printf '%s\n' "${raw#java-v}"
      ;;
    php-v[0-9]*)
      printf '%s\n' "${raw#php-v}"
      ;;
    ruby-v[0-9]*)
      printf '%s\n' "${raw#ruby-v}"
      ;;
    *)
      printf '%s\n' "$raw"
      ;;
  esac
}

github_output=""
print_version=0
raw_release_version=""

while (($# > 0)); do
  case "$1" in
    --github-output)
      if (($# < 2)); then
        echo "missing value for --github-output" >&2
        exit 1
      fi
      github_output="$2"
      shift 2
      ;;
    --print-version)
      print_version=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      if [[ -n "$raw_release_version" ]]; then
        echo "unexpected extra argument: $1" >&2
        exit 1
      fi
      raw_release_version="$1"
      shift
      ;;
  esac
done

if [[ -z "$raw_release_version" && -n "${RELEASE_VERSION:-}" ]]; then
  raw_release_version="$RELEASE_VERSION"
fi

if [[ -z "$raw_release_version" && "${GITHUB_REF:-}" == refs/tags/* ]]; then
  raw_release_version="${GITHUB_REF#refs/tags/}"
fi

if (( print_version != 0 )); then
  bash ./scripts/ci/check_version_sync.sh >&2
else
  bash ./scripts/ci/check_version_sync.sh
fi

workspace_version="$(extract_with_regex '^[[:space:]]*version = "[0-9]+\.[0-9]+\.[0-9]+"' Cargo.toml 's/^[^"]*"([^"]+)".*$/\1/')"
latest_changelog_version="$(extract_with_regex '^## \[[0-9]+\.[0-9]+\.[0-9]+\]' CHANGELOG.md 's/^## \[([0-9]+\.[0-9]+\.[0-9]+)\].*$/\1/')"

if [[ -z "$workspace_version" ]]; then
  echo "::error file=Cargo.toml::Failed to parse workspace version" >&2
  exit 1
fi

if ! grep -Fq '## [Unreleased]' CHANGELOG.md; then
  echo "::error file=CHANGELOG.md::Missing required Unreleased section" >&2
  exit 1
fi

if [[ -z "$latest_changelog_version" ]]; then
  echo "::error file=CHANGELOG.md::Failed to parse latest released version" >&2
  exit 1
fi

if [[ "$workspace_version" != "$latest_changelog_version" ]]; then
  echo "::error file=CHANGELOG.md::Latest released changelog version (${latest_changelog_version}) does not match workspace version (${workspace_version})" >&2
  exit 1
fi

legacy_tool_count_file="$(mktemp)"
if grep -RIn '520\+' README.md docs/src --exclude='mcp-tool-inventory.md' >"$legacy_tool_count_file" 2>/dev/null; then
  echo "::error::Current docs still contain legacy hard-coded MCP tool counts. Replace them with generated inventory references." >&2
  cat "$legacy_tool_count_file" >&2
  rm -f "$legacy_tool_count_file"
  exit 1
fi
rm -f "$legacy_tool_count_file"

node ./scripts/ci/generate_mcp_inventory.mjs --check >/dev/null

normalized_release_version="$workspace_version"
if [[ -n "$raw_release_version" ]]; then
  normalized_release_version="$(normalize_release_version "$raw_release_version")"
  if [[ ! "$normalized_release_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "::error::Release version must be SemVer after normalization, got '${raw_release_version}'" >&2
    exit 1
  fi
  if [[ "$normalized_release_version" != "$workspace_version" ]]; then
    echo "::error::Release version (${normalized_release_version}) does not match workspace version (${workspace_version})" >&2
    exit 1
  fi
fi

if [[ -n "$github_output" ]]; then
  printf 'version=%s\n' "$normalized_release_version" >> "$github_output"
fi

if (( print_version != 0 )); then
  printf '%s\n' "$normalized_release_version"
  echo "Release hygiene checks passed for ${normalized_release_version}." >&2
else
  echo "Release hygiene checks passed for ${normalized_release_version}."
fi
