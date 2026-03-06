#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

workspace_version="$(grep -E '^[[:space:]]*version = "[0-9]+\.[0-9]+\.[0-9]+"' Cargo.toml | head -n1 | sed -E 's/^[^"]*"([^"]+)".*$/\1/')"
cli_version="$(grep -E '^[[:space:]]*"version":[[:space:]]*"[0-9]+\.[0-9]+\.[0-9]+"' cli/package.json | head -n1 | sed -E 's/^[^:]+:[[:space:]]*"([^"]+)".*$/\1/')"
cli_runtime_version="$(grep -E "^export const CLI_VERSION = '[0-9]+\.[0-9]+\.[0-9]+';" cli/src/config.js | head -n1 | sed -E "s/^[^']*'([^']+)'.*$/\1/")"
node_binding_version="$(grep -E '^[[:space:]]*"version":[[:space:]]*"[0-9]+\.[0-9]+\.[0-9]+"' bindings/node/package.json | head -n1 | sed -E 's/^[^:]+:[[:space:]]*"([^"]+)".*$/\1/')"
wasm_binding_version="$(grep -E '^[[:space:]]*"version":[[:space:]]*"[0-9]+\.[0-9]+\.[0-9]+"' bindings/wasm/package.json | head -n1 | sed -E 's/^[^:]+:[[:space:]]*"([^"]+)".*$/\1/')"
python_binding_version="$(grep -E '^[[:space:]]*version = "[0-9]+\.[0-9]+\.[0-9]+"' bindings/python/pyproject.toml | head -n1 | sed -E 's/^[^"]*"([^"]+)".*$/\1/')"
cli_embedded_dep_version="$(grep -E '^[[:space:]]*"@stateset/embedded":[[:space:]]*"\^?[0-9]+\.[0-9]+\.[0-9]+"' cli/package.json | head -n1 | sed -E 's/^[^:]+:[[:space:]]*"\^?([^"]+)".*$/\1/')"

if [[ -z "$workspace_version" || -z "$cli_version" || -z "$cli_runtime_version" || -z "$node_binding_version" || -z "$wasm_binding_version" || -z "$python_binding_version" || -z "$cli_embedded_dep_version" ]]; then
  echo "::error::Failed to parse one or more release versions"
  exit 1
fi

fail=0

if [[ "$workspace_version" != "$cli_version" ]]; then
  echo "::error file=Cargo.toml::Workspace version (${workspace_version}) does not match CLI version (${cli_version})"
  fail=1
fi

if [[ "$workspace_version" != "$cli_runtime_version" ]]; then
  echo "::error file=cli/src/config.js::Workspace version (${workspace_version}) does not match CLI runtime version (${cli_runtime_version})"
  fail=1
fi

if [[ "$workspace_version" != "$node_binding_version" ]]; then
  echo "::error file=bindings/node/package.json::Workspace version (${workspace_version}) does not match Node binding version (${node_binding_version})"
  fail=1
fi

if [[ "$workspace_version" != "$wasm_binding_version" ]]; then
  echo "::error file=bindings/wasm/package.json::Workspace version (${workspace_version}) does not match WASM binding version (${wasm_binding_version})"
  fail=1
fi

if [[ "$workspace_version" != "$python_binding_version" ]]; then
  echo "::error file=bindings/python/pyproject.toml::Workspace version (${workspace_version}) does not match Python binding version (${python_binding_version})"
  fail=1
fi

if [[ "$workspace_version" != "$cli_embedded_dep_version" ]]; then
  echo "::error file=cli/package.json::CLI embedded dependency (${cli_embedded_dep_version}) does not match workspace version (${workspace_version})"
  fail=1
fi

required_readme_snippets=(
  "pip install stateset-embedded==${workspace_version}"
  "gem install stateset_embedded -v ${workspace_version}"
  "npm install @stateset/embedded@${workspace_version}"
  "npm install -g @stateset/cli@${workspace_version}"
  "## What's New in v${workspace_version}"
  "stateset-embedded = \"${workspace_version}\""
  "<version>${workspace_version}</version>"
  "implementation 'com.stateset:embedded:${workspace_version}'"
  "implementation(\"com.stateset:embedded-kotlin:${workspace_version}\")"
  ".package(url: \"https://github.com/stateset/stateset-swift.git\", from: \"${workspace_version}\")"
  "pod 'StateSet', '~> ${workspace_version}'"
  "dotnet add package StateSet.Embedded --version ${workspace_version}"
  "<PackageReference Include=\"StateSet.Embedded\" Version=\"${workspace_version}\" />"
  "go get github.com/stateset/stateset-icommerce/bindings/go/stateset@v${workspace_version}"
)

for snippet in "${required_readme_snippets[@]}"; do
  if ! grep -Fq "$snippet" README.md; then
    echo "::error file=README.md::Missing version-synced snippet: $snippet"
    fail=1
  fi
done

required_cli_snippets=(
  "**Version:** ${workspace_version}"
  "version: ${workspace_version}"
  "\"_version\": \"${workspace_version}\""
)

cli_snippet_files=(
  "cli/README.md"
  "cli/smithery.yaml"
  "cli/deploy/gateway.config.example.json"
)

for index in "${!required_cli_snippets[@]}"; do
  snippet="${required_cli_snippets[$index]}"
  file="${cli_snippet_files[$index]}"
  if ! grep -Fq "$snippet" "$file"; then
    echo "::error file=${file}::Missing version-synced snippet: $snippet"
    fail=1
  fi
done

if (( fail != 0 )); then
  exit 1
fi

echo "Version sync checks passed for ${workspace_version}."
