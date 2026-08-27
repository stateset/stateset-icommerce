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

workspace_version="$(extract_with_regex '^[[:space:]]*version = "[0-9]+\.[0-9]+\.[0-9]+"' Cargo.toml 's/^[^"]*"([^"]+)".*$/\1/')"
cli_version="$(extract_with_regex '^[[:space:]]*"version":[[:space:]]*"[0-9]+\.[0-9]+\.[0-9]+"' cli/package.json 's/^[^:]+:[[:space:]]*"([^"]+)".*$/\1/')"
cli_runtime_version="$(node --input-type=module -e "import { CLI_VERSION } from './cli/src/config.js'; console.log(CLI_VERSION);" 2>/dev/null || true)"
node_binding_version="$(extract_with_regex '^[[:space:]]*"version":[[:space:]]*"[0-9]+\.[0-9]+\.[0-9]+"' bindings/node/package.json 's/^[^:]+:[[:space:]]*"([^"]+)".*$/\1/')"
wasm_binding_version="$(extract_with_regex '^[[:space:]]*"version":[[:space:]]*"[0-9]+\.[0-9]+\.[0-9]+"' bindings/wasm/package.json 's/^[^:]+:[[:space:]]*"([^"]+)".*$/\1/')"
python_binding_version="$(extract_with_regex '^[[:space:]]*version = "[0-9]+\.[0-9]+\.[0-9]+"' bindings/python/pyproject.toml 's/^[^"]*"([^"]+)".*$/\1/')"
python_wrapper_version="$(extract_with_regex '^__version__ = "[0-9]+\.[0-9]+\.[0-9]+"$' bindings/python/python/stateset_embedded/__init__.py 's/^[^"]*"([^"]+)".*$/\1/')"
ruby_binding_version="$(extract_with_regex "VERSION = '[0-9]+\.[0-9]+\.[0-9]+'" bindings/ruby/lib/stateset_embedded.rb "s/^[^']*'([^']+)'.*$/\1/")"
ruby_gemspec_version="$(extract_with_regex "s.version[[:space:]]*=[[:space:]]*'[0-9]+\.[0-9]+\.[0-9]+'" bindings/ruby/stateset_embedded.gemspec "s/^[^']*'([^']+)'.*$/\1/")"
php_binding_version="$(extract_with_regex '^[[:space:]]*"version":[[:space:]]*"[0-9]+\.[0-9]+\.[0-9]+"' bindings/php/composer.json 's/^[^:]+:[[:space:]]*"([^"]+)".*$/\1/')"
php_stub_version="$(extract_with_regex '^[[:space:]]*\* @version [0-9]+\.[0-9]+\.[0-9]+$' bindings/php/stubs/StateSet.php 's/^[^0-9]*([0-9]+\.[0-9]+\.[0-9]+).*$/\1/')"
php_branch_alias="$(extract_with_regex '^[[:space:]]*"dev-main":[[:space:]]*"[0-9]+\.[0-9]+\.x-dev"' bindings/php/composer.json 's/^[[:space:]]*"dev-main":[[:space:]]*"([^"]+)".*$/\1/')"
java_binding_version="$(extract_with_regex "^version = '[0-9]+\.[0-9]+\.[0-9]+'$" bindings/java/java/build.gradle "s/^[^']*'([^']+)'.*$/\1/")"
java_maven_artifact="$(extract_with_regex "^[[:space:]]*artifactId = '[^']+'$" bindings/java/java/build.gradle "s/^[^']*'([^']+)'.*$/\1/")"
java_jar_basename="$(extract_with_regex "^rootProject.name = '[^']+'$" bindings/java/java/settings.gradle "s/^[^']*'([^']+)'.*$/\1/")"
kotlin_binding_version="$(extract_with_regex '^[[:space:]]*version = "[0-9]+\.[0-9]+\.[0-9]+"$' bindings/kotlin/kotlin/build.gradle.kts 's/^[^"]*"([^"]+)".*$/\1/')"
kotlin_maven_artifact="$(extract_with_regex '^[[:space:]]*artifactId = "[^"]+"$' bindings/kotlin/kotlin/build.gradle.kts 's/^[^"]*"([^"]+)".*$/\1/')"
dotnet_binding_version="$(extract_with_regex '^[[:space:]]*<Version>[0-9]+\.[0-9]+\.[0-9]+</Version>$' bindings/dotnet/dotnet/StateSet/StateSet.csproj 's/^[^>]*>([^<]+).*/\1/')"
generator_spec_version="$(extract_with_regex '^version: "[0-9]+\.[0-9]+\.[0-9]+"' bindings/generator/spec.yaml 's/^[^"]*"([^"]+)".*$/\1/')"
cli_embedded_dep_version="$(extract_with_regex '^[[:space:]]*"@stateset/embedded":[[:space:]]*"\^?[0-9]+\.[0-9]+\.[0-9]+"' cli/package.json 's/^[^:]+:[[:space:]]*"\^?([^"]+)".*$/\1/')"
embedded_cli_peer_version="$(extract_with_regex '^[[:space:]]*"@stateset/cli":[[:space:]]*"\^?[0-9]+\.[0-9]+\.[0-9]+"' bindings/node/package.json 's/^[^:]+:[[:space:]]*"\^?([^"]+)".*$/\1/')"
embedded_platform_pkg_version="$(extract_with_regex '^[[:space:]]*"version":[[:space:]]*"[0-9]+\.[0-9]+\.[0-9]+"' bindings/node/npm/linux-x64-gnu/package.json 's/^[^:]+:[[:space:]]*"([^"]+)".*$/\1/')"
admin_version="$(extract_with_regex '^[[:space:]]*"version":[[:space:]]*"[0-9]+\.[0-9]+\.[0-9]+"' admin/package.json 's/^[^:]+:[[:space:]]*"([^"]+)".*$/\1/')"

if [[ -z "$workspace_version" || -z "$cli_version" || -z "$cli_runtime_version" || -z "$node_binding_version" || -z "$wasm_binding_version" || -z "$python_binding_version" || -z "$python_wrapper_version" || -z "$ruby_binding_version" || -z "$ruby_gemspec_version" || -z "$php_binding_version" || -z "$php_stub_version" || -z "$php_branch_alias" || -z "$java_binding_version" || -z "$java_maven_artifact" || -z "$java_jar_basename" || -z "$kotlin_binding_version" || -z "$kotlin_maven_artifact" || -z "$dotnet_binding_version" || -z "$generator_spec_version" || -z "$cli_embedded_dep_version" || -z "$admin_version" ]]; then
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

if [[ "$workspace_version" != "$ruby_gemspec_version" ]]; then
  echo "::error file=bindings/ruby/stateset_embedded.gemspec::Workspace version (${workspace_version}) does not match Ruby gemspec version (${ruby_gemspec_version})"
  fail=1
fi

if [[ "$workspace_version" != "$php_binding_version" ]]; then
  echo "::error file=bindings/php/composer.json::Workspace version (${workspace_version}) does not match PHP binding version (${php_binding_version})"
  fail=1
fi

if [[ "$workspace_version" != "$php_stub_version" ]]; then
  echo "::error file=bindings/php/stubs/StateSet.php::Workspace version (${workspace_version}) does not match PHP stub version (${php_stub_version})"
  fail=1
fi

expected_php_branch_alias="${workspace_version%.*}.x-dev"
if [[ "$expected_php_branch_alias" != "$php_branch_alias" ]]; then
  echo "::error file=bindings/php/composer.json::Workspace version (${workspace_version}) does not match PHP branch alias (${php_branch_alias})"
  fail=1
fi

if [[ "$workspace_version" != "$java_binding_version" ]]; then
  echo "::error file=bindings/java/java/build.gradle::Workspace version (${workspace_version}) does not match Java binding version (${java_binding_version})"
  fail=1
fi

if [[ "$java_maven_artifact" != "embedded" ]]; then
  echo "::error file=bindings/java/java/build.gradle::Java Maven artifact (${java_maven_artifact}) must remain embedded"
  fail=1
fi

if [[ "$workspace_version" != "$kotlin_binding_version" ]]; then
  echo "::error file=bindings/kotlin/kotlin/build.gradle.kts::Workspace version (${workspace_version}) does not match Kotlin binding version (${kotlin_binding_version})"
  fail=1
fi

if [[ "$kotlin_maven_artifact" != "embedded-kotlin" ]]; then
  echo "::error file=bindings/kotlin/kotlin/build.gradle.kts::Kotlin Maven artifact (${kotlin_maven_artifact}) must remain embedded-kotlin"
  fail=1
fi

if [[ "$workspace_version" != "$dotnet_binding_version" ]]; then
  echo "::error file=bindings/dotnet/dotnet/StateSet/StateSet.csproj::Workspace version (${workspace_version}) does not match .NET binding version (${dotnet_binding_version})"
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

# The reverse direction went unchecked for months: embedded's peer pin on the
# CLI sat at ^1.7.0 while everything else moved to 1.23.x.
if [[ "$workspace_version" != "$embedded_cli_peer_version" ]]; then
  echo "::error file=bindings/node/package.json::Embedded @stateset/cli peer (${embedded_cli_peer_version}) does not match workspace version (${workspace_version})"
  fail=1
fi

if [[ "$workspace_version" != "$embedded_platform_pkg_version" ]]; then
  echo "::error file=bindings/node/npm/linux-x64-gnu/package.json::Platform package version (${embedded_platform_pkg_version}) does not match workspace version (${workspace_version})"
  fail=1
fi

if [[ "$workspace_version" != "$admin_version" ]]; then
  echo "::error file=admin/package.json::Workspace version (${workspace_version}) does not match Admin version (${admin_version})"
  fail=1
fi

required_version_snippets=(
  "crates/stateset-sdk/src/lib.rs|//! stateset-sdk = \"${workspace_version}\""
  "README.md|cargo add stateset-sdk --features full"
  "README.md|pip install stateset-embedded==${workspace_version}"
  "README.md|gem install stateset_embedded -v ${workspace_version}"
  "README.md|npm install @stateset/embedded@${workspace_version}"
  "README.md|npm install -g @stateset/cli@${workspace_version}"
  "README.md|## What's New in v${workspace_version}"
  "README.md|stateset-embedded = \"${workspace_version}\""
  "README.md|<artifactId>${java_maven_artifact}</artifactId>"
  "README.md|<version>${workspace_version}</version>"
  "README.md|implementation 'com.stateset:${java_maven_artifact}:${workspace_version}'"
  "README.md|implementation(\"com.stateset:${kotlin_maven_artifact}:${workspace_version}\")"
  "README.md|.package(url: \"https://github.com/stateset/stateset-swift.git\", from: \"${workspace_version}\")"
  "README.md|pod 'StateSet', '~> ${workspace_version}'"
  "README.md|dotnet add package StateSet.Embedded --version ${workspace_version}"
  "README.md|<PackageReference Include=\"StateSet.Embedded\" Version=\"${workspace_version}\" />"
  "README.md|go get github.com/stateset/stateset-icommerce/bindings/go/stateset@v${workspace_version}"
  "docs/src/index.md|Current release: **${workspace_version}**"
  "docs/src/trust-foundation.md|- Current workspace release: \`${workspace_version}\`"
  "docs/src/getting-started.md|cargo add stateset-sdk --features full"
  "docs/src/getting-started.md|npm install @stateset/embedded@${workspace_version}"
  "docs/src/getting-started.md|pip install stateset-embedded==${workspace_version}"
  "docs/src/getting-started.md|npm install @stateset/cli@${workspace_version} @stateset/embedded@${workspace_version}"
  "docs/src/ai-agents.md|npm install @stateset/cli@${workspace_version} @stateset/embedded@${workspace_version}"
  "docs/src/api/java.md|<artifactId>${java_maven_artifact}</artifactId>"
  "docs/src/api/java.md|<version>${workspace_version}</version>"
  "docs/src/api/java.md|implementation 'com.stateset:${java_maven_artifact}:${workspace_version}'"
  "docs/src/api/go.md|go get github.com/stateset/stateset-icommerce/bindings/go/stateset@v${workspace_version}"
  "docs/src/api/go.md|\"github.com/stateset/stateset-icommerce/bindings/go/stateset\""
  "docs/src/api/rust.md|stateset-embedded = \"${workspace_version}\""
  "docs/src/api/rust.md|stateset-embedded = { version = \"${workspace_version}\", features = [\"postgres\"] }"
  "docs/src/api/kotlin.md|implementation(\"com.stateset:${kotlin_maven_artifact}:${workspace_version}\")"
  "docs/src/api/kotlin.md|implementation 'com.stateset:${kotlin_maven_artifact}:${workspace_version}'"
  "docs/src/api/ruby.md|gem install stateset_embedded"
  "docs/src/api/ruby.md|gem 'stateset_embedded'"
  "docs/src/api/php.md|composer require stateset/embedded"
  "docs/src/api/php.md|extension=stateset_embedded"
  "docs/src/guides/async-vs-sync.md|stateset-embedded = \"${workspace_version}\""
  "docs/src/guides/async-vs-sync.md|stateset-embedded = { version = \"${workspace_version}\", features = [\"postgres\"] }"
  "docs/src/guides/async-vs-sync.md|stateset-embedded = { version = \"${workspace_version}\", features = [\"sqlite\", \"postgres\"] }"
  "docs/src/api/swift.md|from: \"${workspace_version}\""
  "docs/src/advanced/deployment.md|image: stateset/icommerce:${workspace_version}"
  "TRUST_FOUNDATION.md|The current workspace release line is \`${workspace_version}\`."
  "bindings/java/README.md|<version>${workspace_version}</version>"
  "examples/README.md|javac -d . -cp path/to/${java_jar_basename}-${workspace_version}.jar BasicUsage.java"
  "examples/README.md|java -cp .:path/to/${java_jar_basename}-${workspace_version}.jar com.stateset.examples.BasicUsage"
  "examples/java/BasicUsage.java|* Compile with: javac -d . -cp ${java_jar_basename}-${workspace_version}.jar BasicUsage.java"
  "examples/java/BasicUsage.java|* Run with: java -cp .:${java_jar_basename}-${workspace_version}.jar com.stateset.examples.BasicUsage"
  "examples/go/basic_usage.go|// Build the Rust library first: cargo build --release -p stateset-go"
  "examples/go/basic_usage.go|// Then run with: go run basic_usage.go"
  "examples/kotlin/BasicUsage.kt|* Run with: ./gradlew run"
  "examples/kotlin/BasicUsage.kt|* Or build a jar: ./gradlew jar && java -jar build/libs/kotlin-${workspace_version}.jar"
  "examples/swift/BasicUsage.swift|* Run with: swift run"
  "bindings/swift/README.md|from: \"${workspace_version}\""
  "bindings/ruby/spec/commerce_spec.rb|expect(StateSet::VERSION).to eq('${workspace_version}')"
  "examples/README.md|java -jar build/libs/kotlin-${workspace_version}.jar"
  "examples/ruby/Gemfile|gem 'stateset_embedded', '~> ${workspace_version}'"
  "examples/kotlin/build.gradle.kts|version = \"${workspace_version}\""
  "examples/kotlin/build.gradle.kts|implementation(\"com.stateset:embedded-kotlin:${workspace_version}\")"
  "examples/dotnet/BasicUsage.csproj|<PackageReference Include=\"StateSet.Embedded\" Version=\"${workspace_version}\" />"
  "examples/node/package.json|\"version\": \"${workspace_version}\""
  "examples/node/package.json|\"@stateset/embedded\": \"^${workspace_version}\""
  "packages/create-stateset-app/templates/storefront/package.json|\"@stateset/embedded\": \"^${workspace_version}\""
  "packages/create-stateset-app/package.json|\"version\": \"${workspace_version}\""
)

for entry in "${required_version_snippets[@]}"; do
  file="${entry%%|*}"
  snippet="${entry#*|}"
  if ! grep -Fq -- "$snippet" "$file"; then
    echo "::error file=${file}::Missing version-synced snippet: $snippet"
    fail=1
  fi
done

disallowed_snippets=(
  "examples/README.md|swiftc -I ../bindings/swift/Sources -L ../target/release -lstateset_swift BasicUsage.swift -o basic_usage"
  "examples/swift/BasicUsage.swift|#!/usr/bin/env swift"
  "examples/swift/BasicUsage.swift|* Run with: swift BasicUsage.swift"
  "examples/kotlin/BasicUsage.kt|* Run with: kotlinc BasicUsage.kt -include-runtime -d BasicUsage.jar && java -jar BasicUsage.jar"
)

for entry in "${disallowed_snippets[@]}"; do
  file="${entry%%|*}"
  snippet="${entry#*|}"
  if grep -Fq -- "$snippet" "$file"; then
    echo "::error file=${file}::Found stale or unsupported snippet: $snippet"
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
  if ! grep -Fq -- "$snippet" "$file"; then
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
