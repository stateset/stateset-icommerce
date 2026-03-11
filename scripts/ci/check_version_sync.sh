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
python_wrapper_version="$(grep -E '^__version__ = "[0-9]+\.[0-9]+\.[0-9]+"$' bindings/python/python/stateset_embedded/__init__.py | head -n1 | sed -E 's/^[^"]*"([^"]+)".*$/\1/')"
ruby_binding_version="$(grep -E "VERSION = '[0-9]+\.[0-9]+\.[0-9]+'" bindings/ruby/lib/stateset_embedded.rb | head -n1 | sed -E "s/^[^']*'([^']+)'.*$/\1/")"
php_binding_version="$(grep -E '^[[:space:]]*"version":[[:space:]]*"[0-9]+\.[0-9]+\.[0-9]+"' bindings/php/composer.json | head -n1 | sed -E 's/^[^:]+:[[:space:]]*"([^"]+)".*$/\1/')"
generator_spec_version="$(grep -E '^version: "[0-9]+\.[0-9]+\.[0-9]+"' bindings/generator/spec.yaml | head -n1 | sed -E 's/^[^"]*"([^"]+)".*$/\1/')"
cli_embedded_dep_version="$(grep -E '^[[:space:]]*"@stateset/embedded":[[:space:]]*"\^?[0-9]+\.[0-9]+\.[0-9]+"' cli/package.json | head -n1 | sed -E 's/^[^:]+:[[:space:]]*"\^?([^"]+)".*$/\1/')"

if [[ -z "$workspace_version" || -z "$cli_version" || -z "$cli_runtime_version" || -z "$node_binding_version" || -z "$wasm_binding_version" || -z "$python_binding_version" || -z "$python_wrapper_version" || -z "$ruby_binding_version" || -z "$php_binding_version" || -z "$generator_spec_version" || -z "$cli_embedded_dep_version" ]]; then
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

if [[ "$workspace_version" != "$python_wrapper_version" ]]; then
  echo "::error file=bindings/python/python/stateset_embedded/__init__.py::Workspace version (${workspace_version}) does not match Python wrapper version (${python_wrapper_version})"
  fail=1
fi

if [[ "$workspace_version" != "$ruby_binding_version" ]]; then
  echo "::error file=bindings/ruby/lib/stateset_embedded.rb::Workspace version (${workspace_version}) does not match Ruby binding version (${ruby_binding_version})"
  fail=1
fi

if [[ "$workspace_version" != "$php_binding_version" ]]; then
  echo "::error file=bindings/php/composer.json::Workspace version (${workspace_version}) does not match PHP binding version (${php_binding_version})"
  fail=1
fi

if [[ "$workspace_version" != "$generator_spec_version" ]]; then
  echo "::error file=bindings/generator/spec.yaml::Workspace version (${workspace_version}) does not match generator spec version (${generator_spec_version})"
  fail=1
fi

if [[ "$workspace_version" != "$cli_embedded_dep_version" ]]; then
  echo "::error file=cli/package.json::CLI embedded dependency (${cli_embedded_dep_version}) does not match workspace version (${workspace_version})"
  fail=1
fi

required_version_snippets=(
  "README.md|pip install stateset-embedded==${workspace_version}"
  "README.md|gem install stateset_embedded -v ${workspace_version}"
  "README.md|npm install @stateset/embedded@${workspace_version}"
  "README.md|npm install -g @stateset/cli@${workspace_version}"
  "README.md|## What's New in v${workspace_version}"
  "README.md|stateset-embedded = \"${workspace_version}\""
  "README.md|<version>${workspace_version}</version>"
  "README.md|implementation 'com.stateset:embedded:${workspace_version}'"
  "README.md|implementation(\"com.stateset:embedded-kotlin:${workspace_version}\")"
  "README.md|.package(url: \"https://github.com/stateset/stateset-swift.git\", from: \"${workspace_version}\")"
  "README.md|pod 'StateSet', '~> ${workspace_version}'"
  "README.md|dotnet add package StateSet.Embedded --version ${workspace_version}"
  "README.md|<PackageReference Include=\"StateSet.Embedded\" Version=\"${workspace_version}\" />"
  "README.md|go get github.com/stateset/stateset-icommerce/bindings/go/stateset@v${workspace_version}"
  "docs/src/index.md|Current release: **${workspace_version}**"
  "docs/src/getting-started.md|npm install @stateset/embedded@${workspace_version}"
  "docs/src/getting-started.md|pip install stateset-embedded==${workspace_version}"
  "docs/src/getting-started.md|npm install @stateset/cli@${workspace_version} @stateset/embedded@${workspace_version}"
  "docs/src/ai-agents.md|npm install @stateset/cli@${workspace_version} @stateset/embedded@${workspace_version}"
  "docs/src/api/java.md|<version>${workspace_version}</version>"
  "docs/src/api/kotlin.md|implementation(\"com.stateset:stateset-embedded:${workspace_version}\")"
  "docs/src/api/swift.md|from: \"${workspace_version}\""
  "bindings/java/README.md|<version>${workspace_version}</version>"
  "bindings/swift/README.md|from: \"${workspace_version}\""
  "examples/node/package.json|\"version\": \"${workspace_version}\""
  "examples/node/package.json|\"@stateset/embedded\": \"^${workspace_version}\""
  "packages/create-stateset-app/templates/storefront/package.json|\"@stateset/embedded\": \"^${workspace_version}\""
)

for entry in "${required_version_snippets[@]}"; do
  file="${entry%%|*}"
  snippet="${entry#*|}"
  if ! grep -Fq "$snippet" "$file"; then
    echo "::error file=${file}::Missing version-synced snippet: $snippet"
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

if ! node --input-type=module -e "await import('./cli/src/standalone.js'); await import('./cli/src/agent-toolkit.js');"; then
  echo "::error file=cli/src/standalone.js::Standalone or agent-toolkit import smoke test failed"
  exit 1
fi

if ! (
  cd "$ROOT_DIR/cli" &&
    node --input-type=module -e "await import('@stateset/cli/standalone'); await import('@stateset/cli/agent-toolkit');"
); then
  echo "::error file=cli/package.json::CLI package self-reference export smoke test failed"
  exit 1
fi

echo "Version sync checks passed for ${workspace_version}."
