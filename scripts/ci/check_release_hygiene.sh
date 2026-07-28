#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

CURRENT_DOC_PATHS=(
  README.md
  QUICKSTART.md
  comparison_doc.md
  cli/README.md
  docs/README.md
  docs/src
)

usage() {
  cat <<'EOF'
Usage: check_release_hygiene.sh [--github-output PATH] [VERSION_OR_TAG]

Validates repo-wide version sync plus release metadata hygiene.

Arguments:
  VERSION_OR_TAG   Optional semantic version or tag name such as:
                   1.23.4, v1.23.4, cli-v1.23.4, py-v1.23.4, java-v1.23.4,
                   php-v1.23.4, ruby-v1.23.4

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
if grep -RIn '520\+' "${CURRENT_DOC_PATHS[@]}" --exclude='mcp-tool-inventory.md' >"$legacy_tool_count_file" 2>/dev/null; then
  echo "::error::Current docs still contain legacy hard-coded MCP tool counts. Replace them with generated inventory references." >&2
  cat "$legacy_tool_count_file" >&2
  rm -f "$legacy_tool_count_file"
  exit 1
fi
rm -f "$legacy_tool_count_file"

node ./scripts/ci/generate_mcp_inventory.mjs --check >/dev/null
node ./scripts/ci/generate_agent_inventory.mjs --check >/dev/null
node ./scripts/ci/generate_api_command_coverage.mjs --check >/dev/null
node ./scripts/ci/generate_binding_api_inventory.mjs --check >/dev/null
node ./scripts/ci/generate_http_gateway_inventory.mjs --check >/dev/null
node ./scripts/ci/generate_mcp_api_coverage.mjs --check >/dev/null
node ./scripts/ci/generate_workspace_inventory.mjs --check >/dev/null
node ./scripts/ci/generate_rust_openapi_inventory.mjs --check >/dev/null
node ./scripts/ci/check_doc_tool_refs.mjs >/dev/null
node ./scripts/ci/check_workflow_job_refs.mjs >/dev/null

required_release_surface_snippets=(
  "bindings/python/pyproject.toml|\"Development Status :: 5 - Production/Stable\""
  ".github/workflows/publish-cli.yml|description: 'Version to release (e.g., ${workspace_version})'"
  ".github/workflows/publish-python.yml|description: \"Version to release (e.g., ${workspace_version})\""
  ".github/workflows/publish-rust-crates.yml|description: 'Version to release (e.g., ${workspace_version})'"
  "scripts/ci/check_release_hygiene.sh|                   ${workspace_version}, v${workspace_version}, cli-v${workspace_version}, py-v${workspace_version}, java-v${workspace_version},"
  "scripts/ci/check_release_hygiene.sh|                   php-v${workspace_version}, ruby-v${workspace_version}"
)

for entry in "${required_release_surface_snippets[@]}"; do
  file="${entry%%|*}"
  snippet="${entry#*|}"
  if ! grep -Fq -- "$snippet" "$file"; then
    echo "::error file=${file}::Missing release-hygiene snippet: $snippet" >&2
    exit 1
  fi
done

if grep -Fq '"Development Status :: 4 - Beta"' bindings/python/pyproject.toml; then
  echo "::error file=bindings/python/pyproject.toml::Python binding still advertises Beta status in a 1.0 release line." >&2
  exit 1
fi

legacy_arch_count_file="$(mktemp)"
if grep -RInE '4,000\+ tests|3,477 passing tests|15,300\+ tests|261 tests|41 domain APIs|18 AI agents|41 CLI entry points|41 accessor methods|17 specialized agents|18 specialized agents|90\+ MCP tools|87\+ tools|87 MCP tools|8 specialized agents|37 tests|26 CLI programs|254 types|53 tables|671\+ methods|53\+ REST endpoints|11 language bindings|Eighteen specialized agents' "${CURRENT_DOC_PATHS[@]}" --exclude='mcp-tool-inventory.md' --exclude='workspace-inventory.md' >"$legacy_arch_count_file" 2>/dev/null; then
  echo "::error::Current docs still contain legacy hard-coded architecture or test counts. Replace them with generated workspace inventory references or stable wording." >&2
  cat "$legacy_arch_count_file" >&2
  rm -f "$legacy_arch_count_file"
  exit 1
fi
rm -f "$legacy_arch_count_file"

required_hook_files=(
  .husky/pre-commit
  .husky/commit-msg
  .husky/node-env.sh
)

for hook_file in "${required_hook_files[@]}"; do
  if [[ ! -x "$hook_file" ]]; then
    echo "::error file=${hook_file}::Required Git hook helper is not executable. Restore the executable bit before cutting a release." >&2
    exit 1
  fi
done

disallowed_tracked_artifacts=(
  examples/go/go
  examples/go/go.exe
  bindings/go/example/example
  bindings/go/example/example.exe
)

for artifact in "${disallowed_tracked_artifacts[@]}"; do
  if git ls-files --error-unmatch -- "$artifact" >/dev/null 2>&1; then
    echo "::error file=${artifact}::Tracked generated artifact detected. Remove it before cutting a release." >&2
    exit 1
  fi
done

if command -v file >/dev/null 2>&1; then
  tracked_native_artifacts="$(
    git ls-files -z |
      xargs -0 file |
      grep -E 'ELF|Mach-O|PE32' || true
  )"
  if [[ -n "$tracked_native_artifacts" ]]; then
    echo "::error::Tracked native binaries detected in the repo. Remove generated artifacts before cutting a release." >&2
    printf '%s\n' "$tracked_native_artifacts" >&2
    exit 1
  fi
fi

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
