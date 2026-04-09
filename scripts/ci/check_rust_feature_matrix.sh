#!/usr/bin/env bash
set -euo pipefail

bash "$(dirname "$0")/cargo_ci.sh" test -p stateset-core --all-features --quiet
bash "$(dirname "$0")/cargo_ci.sh" test -p stateset-primitives --all-features --quiet
bash "$(dirname "$0")/cargo_ci.sh" test -p stateset-observability --all-features --quiet
STATESET_ALLOW_POSTGRES_SKIP=1 bash "$(dirname "$0")/cargo_ci.sh" test -p stateset-db --all-features --quiet
STATESET_ALLOW_POSTGRES_SKIP=1 bash "$(dirname "$0")/cargo_ci.sh" test -p stateset-embedded --all-features --quiet
bash "$(dirname "$0")/cargo_ci.sh" test -p stateset-sdk --all-features --quiet
bash "$(dirname "$0")/cargo_ci.sh" test -p stateset-http --quiet
bash "$(dirname "$0")/cargo_ci.sh" test -p stateset-policy --all-features --quiet
bash "$(dirname "$0")/cargo_ci.sh" test -p stateset-sync --quiet
bash "$(dirname "$0")/cargo_ci.sh" test -p stateset-authz --quiet
bash "$(dirname "$0")/cargo_ci.sh" test -p stateset-crypto --quiet
bash "$(dirname "$0")/cargo_ci.sh" test -p stateset-pricing --quiet
bash "$(dirname "$0")/cargo_ci.sh" test -p stateset-protocol --quiet
bash "$(dirname "$0")/cargo_ci.sh" test -p stateset-a2a --quiet
bash "$(dirname "$0")/cargo_ci.sh" test -p stateset-ffi --all-features --quiet
bash "$(dirname "$0")/cargo_ci.sh" test -p stateset-jobs --quiet
bash "$(dirname "$0")/cargo_ci.sh" test -p stateset-macros --quiet

bash "$(dirname "$0")/cargo_ci.sh" check -p stateset-test-utils --quiet
bash "$(dirname "$0")/cargo_ci.sh" check -p stateset-migrations --quiet
bash "$(dirname "$0")/cargo_ci.sh" check -p stateset-benches --benches --quiet

bash "$(dirname "$0")/cargo_ci.sh" check -p stateset-embedded-node --quiet
bash "$(dirname "$0")/cargo_ci.sh" check -p stateset-embedded-python --quiet
bash "$(dirname "$0")/cargo_ci.sh" check -p stateset-embedded-wasm --quiet
bash "$(dirname "$0")/cargo_ci.sh" check -p stateset-go --quiet
bash "$(dirname "$0")/cargo_ci.sh" check -p stateset-java --quiet
bash "$(dirname "$0")/cargo_ci.sh" check -p stateset-kotlin --quiet
bash "$(dirname "$0")/cargo_ci.sh" check -p stateset-swift --quiet
bash "$(dirname "$0")/cargo_ci.sh" check -p stateset-dotnet --quiet

# These bindings are intentionally excluded from workspace membership because
# their runtime features depend on host headers/runtimes. The default build is a
# stub, so a standalone manifest-path compile still gives us useful coverage.
bash "$(dirname "$0")/cargo_ci.sh" check --manifest-path bindings/php/Cargo.toml --quiet
bash "$(dirname "$0")/cargo_ci.sh" check --manifest-path bindings/ruby/Cargo.toml --quiet
