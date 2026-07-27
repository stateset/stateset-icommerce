#!/usr/bin/env bash
# Fresh-consumer smoke against the LIVE crates.io index.
#
# Scaffolds a brand-new cargo project outside the workspace, adds the published
# stateset-sdk at the requested version with --features full, and runs a real
# commerce operation. This is the test that would have caught the defect where
# every published version up to 1.23.0 failed `cargo add` (pre-release pqc
# dependencies drifting under cargo's unlocked semver resolution — masked
# locally and in CI by the workspace Cargo.lock).
#
# Usage: registry_smoke.sh <version>   e.g. registry_smoke.sh 1.23.2
set -euo pipefail

VERSION="${1:?usage: registry_smoke.sh <version>}"

WORKDIR="$(mktemp -d)"
trap 'rm -rf "${WORKDIR}"' EXIT
cd "${WORKDIR}"

cargo init -q --name registry_smoke

# The index can lag the publish by a couple of minutes.
for attempt in $(seq 1 10); do
  if cargo add "stateset-sdk@${VERSION}" --features full -q 2>/dev/null; then
    break
  fi
  if [[ "${attempt}" == "10" ]]; then
    echo "stateset-sdk ${VERSION} never became resolvable on crates.io" >&2
    exit 1
  fi
  echo "index not ready (attempt ${attempt}/10); retrying in 30s"
  sleep 30
done

cat > src/main.rs <<'EOF'
use stateset_sdk::prelude::*;

fn main() -> Result<()> {
    let commerce = Commerce::new(":memory:")?;
    let customer = commerce.customers().create(CreateCustomer {
        email: "registry-smoke@stateset.dev".into(),
        first_name: "Registry".into(),
        last_name: "Smoke".into(),
        ..Default::default()
    })?;
    println!("registry smoke OK: customer {}", customer.id);
    Ok(())
}
EOF

cargo run -q
echo "Fresh-consumer smoke passed for stateset-sdk ${VERSION}."
