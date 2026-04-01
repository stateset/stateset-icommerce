#!/usr/bin/env bash
set -euo pipefail

cargo test -p stateset-core --all-features --quiet
cargo test -p stateset-primitives --all-features --quiet
cargo test -p stateset-observability --all-features --quiet
STATESET_ALLOW_POSTGRES_SKIP=1 cargo test -p stateset-db --all-features --quiet
STATESET_ALLOW_POSTGRES_SKIP=1 cargo test -p stateset-embedded --all-features --quiet
cargo test -p stateset-sdk --all-features --quiet
cargo test -p stateset-http --quiet
cargo test -p stateset-policy --all-features --quiet
cargo test -p stateset-sync --quiet
cargo test -p stateset-authz --quiet
cargo test -p stateset-crypto --quiet
cargo test -p stateset-pricing --quiet
cargo test -p stateset-protocol --quiet
cargo test -p stateset-a2a --quiet
cargo test -p stateset-ffi --all-features --quiet
cargo test -p stateset-jobs --quiet
cargo test -p stateset-macros --quiet

cargo check -p stateset-test-utils --quiet
cargo check -p stateset-migrations --quiet
cargo check -p stateset-benches --benches --quiet

cargo check -p stateset-embedded-node --quiet
cargo check -p stateset-embedded-python --quiet
cargo check -p stateset-embedded-wasm --quiet
cargo check -p stateset-go --quiet
cargo check -p stateset-java --quiet
cargo check -p stateset-kotlin --quiet
cargo check -p stateset-swift --quiet
cargo check -p stateset-dotnet --quiet

# These bindings are intentionally excluded from workspace membership because
# their runtime features depend on host headers/runtimes. The default build is a
# stub, so a standalone manifest-path compile still gives us useful coverage.
cargo check --manifest-path bindings/php/Cargo.toml --quiet
cargo check --manifest-path bindings/ruby/Cargo.toml --quiet
