#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUTPUT_DIR="${GOLDEN_PATH_EVIDENCE_DIR:-${REPO_ROOT}/artifacts/storefront-golden-path}"
SKIP_DEPENDENCIES="${GOLDEN_PATH_SKIP_DEPENDENCIES:-0}"
WORK_DIR="$(mktemp -d)"
PROJECT_DIR="${WORK_DIR}/golden-store"
LOG_PATH="${OUTPUT_DIR}/golden-path.log"
mkdir -p "${OUTPUT_DIR}"
trap 'rm -rf "${WORK_DIR}"' EXIT

run_logged() {
  echo "+ $*" | tee -a "${LOG_PATH}"
  "$@" 2>&1 | tee -a "${LOG_PATH}"
}

cd "${REPO_ROOT}"
: > "${LOG_PATH}"
run_logged node ./scripts/check-node.mjs 20.20.0

run_logged npm pack ./packages/create-stateset-app --pack-destination "${WORK_DIR}"
GENERATOR_TARBALL="$(find "${WORK_DIR}" -maxdepth 1 -name 'create-stateset-app-*.tgz' -print -quit)"
if [[ -z "${GENERATOR_TARBALL}" ]]; then
  echo "error: packed generator was not produced" >&2
  exit 1
fi
GENERATOR_TARBALL_SHA256="$(sha256sum "${GENERATOR_TARBALL}" | cut -d ' ' -f 1)"

# Exercise the package's public bin exactly as an operator does. --yes makes
# the command deterministic while --skip-install lets this script own and log
# each subsequent golden-path stage.
run_logged npm exec --yes --package="${GENERATOR_TARBALL}" -- \
  create-stateset-app "${PROJECT_DIR}" --yes --skip-install

test -f "${PROJECT_DIR}/package.json"
test -f "${PROJECT_DIR}/scripts/seed.js"
if grep -R '{{STORE_NAME}}\|{{PACKAGE_NAME}}' "${PROJECT_DIR}" --exclude='package-lock.json'; then
  echo "error: generated storefront contains unresolved placeholders" >&2
  exit 1
fi

LEVEL="scaffold"
RESOLVED_LOCK_SHA256=""
if [[ "${SKIP_DEPENDENCIES}" != "1" ]]; then
  cd "${PROJECT_DIR}"
  run_logged npm install --no-fund --no-audit
  cp package-lock.json "${OUTPUT_DIR}/resolved-package-lock.json"
  RESOLVED_LOCK_SHA256="$(sha256sum package-lock.json | cut -d ' ' -f 1)"
  run_logged npm audit --audit-level=high
  run_logged npm run seed
  # The single-quoted JavaScript intentionally contains JS template literals.
  # shellcheck disable=SC2016
  run_logged node --input-type=module -e '
    import { Commerce } from "@stateset/embedded";
    const commerce = new Commerce(process.env.STATESET_DB_PATH || "./store.db");
    const products = await commerce.products.list();
    const stock = await commerce.inventory.getStock("CLASSIC-T-SHIRT");
    if (products.length !== 10) throw new Error(`expected 10 products, got ${products.length}`);
    if (stock.totalAvailable !== "100" || stock.totalOnHand !== "100") {
      throw new Error(`expected stock 100, got ${JSON.stringify(stock)}`);
    }
    console.log(JSON.stringify({ products: products.length, sku: "CLASSIC-T-SHIRT", stock: "100" }));
  '
  run_logged npm run typecheck
  run_logged npm run build
  LEVEL="install-seed-query-typecheck-build"
fi

cd "${REPO_ROOT}"
COMMIT_SHA="${GITHUB_SHA:-$(git rev-parse HEAD)}"
CREATED_AT="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
LOG_SHA256="$(sha256sum "${LOG_PATH}" | cut -d ' ' -f 1)"
GENERATOR_VERSION="$(node -p "require('./packages/create-stateset-app/package.json').version")"
EMBEDDED_VERSION="$(node -p "require('./bindings/node/package.json').version")"
export LEVEL CREATED_AT COMMIT_SHA GENERATOR_VERSION EMBEDDED_VERSION LOG_SHA256
export GENERATOR_TARBALL_SHA256 RESOLVED_LOCK_SHA256

node --input-type=module - "${OUTPUT_DIR}/evidence.json" <<'NODE'
import fs from 'node:fs';

const output = process.argv[2];
const evidence = {
  schema_version: 1,
  result: 'passed',
  level: process.env.LEVEL,
  created_at: process.env.CREATED_AT,
  commit_sha: process.env.COMMIT_SHA,
  github_run_id: process.env.GITHUB_RUN_ID || 'local',
  github_run_attempt: process.env.GITHUB_RUN_ATTEMPT || 'local',
  generator_version: process.env.GENERATOR_VERSION,
  embedded_version: process.env.EMBEDDED_VERSION,
  generator_tarball_sha256: process.env.GENERATOR_TARBALL_SHA256,
  resolved_package_lock: process.env.RESOLVED_LOCK_SHA256
    ? {
        path: 'resolved-package-lock.json',
        sha256: process.env.RESOLVED_LOCK_SHA256,
      }
    : null,
  assertions:
    process.env.LEVEL === 'scaffold'
      ? ['packed public CLI executed', 'template complete', 'no unresolved placeholders']
      : [
          'packed public CLI executed',
          'dependencies installed',
          'high-severity audit passed',
          '10 products seeded and queried',
          'inventory stock verified',
          'TypeScript passed',
          'production build passed',
        ],
  log: 'golden-path.log',
  log_sha256: process.env.LOG_SHA256,
};
fs.writeFileSync(output, `${JSON.stringify(evidence, null, 2)}\n`);
NODE

echo "Storefront golden-path evidence written to ${OUTPUT_DIR}"
