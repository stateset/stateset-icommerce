#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUTPUT_DIR="${GOLDEN_PATH_EVIDENCE_DIR:-${REPO_ROOT}/artifacts/storefront-golden-path}"
SKIP_DEPENDENCIES="${GOLDEN_PATH_SKIP_DEPENDENCIES:-0}"
# Where the generated storefront gets @stateset/embedded from:
#   local-tarball (default) - build this checkout's binding and install the
#     packed result, so the gate proves THIS commit scaffolds and runs. Before
#     this, the golden path installed whatever the registry served, which meant
#     a PR could pass against a binding that no longer matched its own source.
#   registry - the post-publish verification path, kept for the dispatch-only
#     `registry-install` job in storefront-golden-path.yml.
EMBEDDED_SOURCE="${GOLDEN_PATH_EMBEDDED_SOURCE:-local-tarball}"
# Local escape hatch for a machine without a Rust toolchain: reuse an already
# built bindings/node/*.node instead of rebuilding it. CI always builds.
SKIP_EMBEDDED_BUILD="${GOLDEN_PATH_SKIP_EMBEDDED_BUILD:-0}"
WORK_DIR="$(mktemp -d)"
PROJECT_DIR="${WORK_DIR}/golden-store"
LOG_PATH="${OUTPUT_DIR}/golden-path.log"
mkdir -p "${OUTPUT_DIR}"
trap 'rm -rf "${WORK_DIR}"' EXIT

run_logged() {
  echo "+ $*" | tee -a "${LOG_PATH}"
  "$@" 2>&1 | tee -a "${LOG_PATH}"
}

# `npm audit` exits non-zero both when it finds vulnerabilities and when the
# registry's audit endpoint is unreachable, so a registry outage used to read
# as "the storefront has a high-severity vulnerability" and fail the gate. Tell
# the two apart: a real report is JSON we can parse, a service error is not.
# Retry the transport, then fail only on findings we can actually see.
run_audit_with_retries() {
  local attempt output rc
  for attempt in 1 2 3; do
    echo "+ npm audit --audit-level=high --json (attempt ${attempt})" | tee -a "${LOG_PATH}"
    set +e
    output="$(npm audit --audit-level=high --json 2>&1)"
    rc=$?
    set -e
    printf '%s\n' "${output}" >> "${LOG_PATH}"

    # A parseable report means the endpoint answered; rc then reflects findings.
    if printf '%s' "${output}" | node -e '
      let raw = "";
      process.stdin.on("data", (c) => { raw += c; });
      process.stdin.on("end", () => {
        try {
          const parsed = JSON.parse(raw);
          process.exit(parsed && parsed.metadata ? 0 : 1);
        } catch {
          process.exit(1);
        }
      });
    ' 2>/dev/null; then
      if [[ ${rc} -eq 0 ]]; then
        return 0
      fi
      echo "error: npm audit reported high-severity vulnerabilities" >&2
      return 1
    fi

    echo "warning: npm audit endpoint did not return a report (attempt ${attempt})" | tee -a "${LOG_PATH}"
    [[ ${attempt} -lt 3 ]] && sleep $((attempt * 5))
  done

  echo "error: npm audit endpoint unreachable after 3 attempts -- registry issue, not a finding" >&2
  return 1
}

cd "${REPO_ROOT}"
: > "${LOG_PATH}"
run_logged node ./scripts/check-node.mjs 20.20.0

EMBEDDED_TARBALL=""
EMBEDDED_TARBALL_SHA256=""
if [[ "${EMBEDDED_SOURCE}" == "local-tarball" ]]; then
  if [[ "${SKIP_EMBEDDED_BUILD}" != "1" ]]; then
    run_logged npm --prefix bindings/node ci
    run_logged npm --prefix bindings/node run build:debug
  fi

  HOST_BINDING="$(find bindings/node -maxdepth 1 -name 'stateset-embedded.*.node' -print -quit)"
  if [[ -z "${HOST_BINDING}" ]]; then
    echo "error: no built native binding in bindings/node (run npm run build:debug)" >&2
    exit 1
  fi

  # The published @stateset/embedded ships no binaries: each platform's .node
  # lives in an optional per-platform package. Nothing like that exists for an
  # unpublished commit, so stage the canonical packed contents plus this host's
  # freshly built .node -- which index.js prefers over the platform package --
  # and repack. The repository manifest is never modified.
  EMBEDDED_STAGE="${WORK_DIR}/embedded-stage"
  mkdir -p "${EMBEDDED_STAGE}"
  run_logged npm pack ./bindings/node --pack-destination "${EMBEDDED_STAGE}"
  EMBEDDED_BASE_TARBALL="$(find "${EMBEDDED_STAGE}" -maxdepth 1 -name 'stateset-embedded-*.tgz' -print -quit)"
  if [[ -z "${EMBEDDED_BASE_TARBALL}" ]]; then
    echo "error: packed embedded binding was not produced" >&2
    exit 1
  fi
  run_logged tar -xzf "${EMBEDDED_BASE_TARBALL}" -C "${EMBEDDED_STAGE}"
  cp "${HOST_BINDING}" "${EMBEDDED_STAGE}/package/"
  node --input-type=module - "${EMBEDDED_STAGE}/package/package.json" <<'NODE'
import { readFileSync, writeFileSync } from 'node:fs';

const manifestPath = process.argv[2];
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
manifest.files = [...new Set([...(manifest.files ?? []), '*.node'])];
// The per-platform packages are exact pins that do not exist on the registry
// until the release publishes. This staged copy carries the host binary
// itself, so drop them: without this, the golden path would fail with ETARGET
// on exactly the commits that bump the version.
delete manifest.optionalDependencies;
writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
NODE
  run_logged npm pack "${EMBEDDED_STAGE}/package" --pack-destination "${WORK_DIR}"
  EMBEDDED_TARBALL="$(find "${WORK_DIR}" -maxdepth 1 -name 'stateset-embedded-*.tgz' -print -quit)"
  if [[ -z "${EMBEDDED_TARBALL}" ]]; then
    echo "error: repacked embedded binding was not produced" >&2
    exit 1
  fi
  EMBEDDED_TARBALL_SHA256="$(sha256sum "${EMBEDDED_TARBALL}" | cut -d ' ' -f 1)"
fi

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
  if [[ -n "${EMBEDDED_TARBALL}" ]]; then
    # Optional deps omitted: the tarball carries this host's .node, and the
    # per-platform packages for an unpublished version do not exist yet.
    run_logged npm install --no-fund --no-audit --omit=optional "${EMBEDDED_TARBALL}"
  fi
  run_logged npm install --no-fund --no-audit
  cp package-lock.json "${OUTPUT_DIR}/resolved-package-lock.json"
  RESOLVED_LOCK_SHA256="$(sha256sum package-lock.json | cut -d ' ' -f 1)"
  run_audit_with_retries
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
export EMBEDDED_SOURCE EMBEDDED_TARBALL_SHA256

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
  embedded_source: process.env.EMBEDDED_SOURCE,
  embedded_tarball_sha256: process.env.EMBEDDED_TARBALL_SHA256 || null,
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
          process.env.EMBEDDED_SOURCE === 'local-tarball'
            ? 'locally built @stateset/embedded installed from a packed tarball'
            : 'published @stateset/embedded installed from the registry',
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
