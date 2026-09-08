#!/usr/bin/env bash
# Version-bump every synced release surface, in the right order, with the
# exclusions that hand-run seds kept getting wrong.
#
#   scripts/release-bump.sh <from-version> <to-version>
#
# What this encodes (each learned the hard way during the 1.23.x line):
#   * Never touch ANY Cargo.lock with the sed — including the STANDALONE
#     bindings/php and bindings/ruby lockfiles, which once pinned the
#     third-party `uuid` crate at a version colliding with ours; a blanket
#     sed corrupted its checksum entries and broke both CI lanes.
#   * Never touch CHANGELOG.md (history keeps old versions), generated
#     artifacts/inventories (regenerate instead), supply-chain/config.toml
#     (cargo-vet exemptions for third-party crates), or the crypto fuzz
#     workspace lock.
#   * npm lockfile package/workspace metadata is safe to bump immediately,
#     and CI expects it to match the manifests. Registry-resolved INTERNAL
#     dependencies can only regenerate AFTER those versions are published to
#     npm — before that, npm errors with ETARGET. So the initial bump updates
#     metadata only; publish, then run `release-bump.sh --sync-locks`.
#     cli, admin and examples/node no longer have any: they depend on
#     `file:../bindings/node`, which is why a bump no longer breaks `npm ci`
#     on every branch. Only bindings/node still needs the post-publish sync,
#     for its exact per-platform optionalDependencies pins.
#   * The untracked-at-the-time bindings/node/npm/ platform dirs were missed
#     by `git grep`; this script bumps them explicitly.
#
# After running: update CHANGELOG.md by hand (content, not mechanics), then run
# the hygiene gate (done here last).
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

sync_locks() {
  cd "${ROOT_DIR}"
  echo "==> Regenerating npm lockfiles against the published registry versions"
  for d in bindings/node bindings/wasm cli admin examples/node; do
    (cd "$d" && npm install --package-lock-only)
    echo "    $d"
  done
  echo "==> Verifying every lock/manifest pair"
  # A stale lockfile is invisible until some unrelated branch's CI fails on
  # `npm ci`. Prove each one installs before calling the sync done.
  for d in bindings/node bindings/wasm cli admin examples/node; do
    (cd "$d" && npm ci --dry-run >/dev/null) || {
      echo "::error::npm ci would fail in ${d} after the lock sync" >&2
      exit 1
    }
    echo "    ${d}: npm ci is clean"
  done
  echo "Lockfiles synced. Commit them (chore: sync npm lockfiles for <version>)."
}

if [[ "${1:-}" == "--sync-locks" ]]; then
  sync_locks
  exit 0
fi

FROM="${1:?usage: release-bump.sh <from-version> <to-version> | --sync-locks}"
TO="${2:?usage: release-bump.sh <from-version> <to-version> | --sync-locks}"

cd "${ROOT_DIR}"

if ! [[ "${FROM}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ && "${TO}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "versions must be plain SemVer (got '${FROM}' -> '${TO}')" >&2
  exit 1
fi

FROM_RE="${FROM//./\\.}"
FROM_LINE="${FROM%.*}"
TO_LINE="${TO%.*}"

echo "==> Bumping tracked surfaces ${FROM} -> ${TO}"
mapfile -t files < <(git grep -l "${FROM_RE}" -- . \
  ':(exclude)CHANGELOG.md' \
  ':(exclude)Cargo.lock' \
  ':(exclude)*package-lock.json' \
  ':(exclude)artifacts/*' \
  ':(exclude)docs/src/appendix/*' \
  ':(exclude)supply-chain/*' \
  ':(exclude)bindings/php/Cargo.lock' \
  ':(exclude)bindings/ruby/Cargo.lock' \
  ':(exclude)crates/stateset-crypto/fuzz/Cargo.lock' \
  ':(exclude)cli/src/permissions.js')
for f in "${files[@]}"; do
  sed -i "s/${FROM_RE}/${TO}/g" "$f"
done
echo "    ${#files[@]} tracked files"

# Composer's branch alias follows the release line rather than the exact
# release. Keep it synchronized explicitly so the release gate cannot discover
# the mismatch only after the rest of the workspace has been bumped.
sed -i "s/${FROM_LINE//./\\.}\\.x-dev/${TO_LINE}.x-dev/g" bindings/php/composer.json

echo "==> Bumping npm platform package dirs (may be untracked on new platforms)"
for f in bindings/node/npm/*/package.json; do
  [ -f "$f" ] && sed -i "s/${FROM_RE}/${TO}/g" "$f"
done

echo "==> Bumping npm lockfile package/workspace metadata"
node --input-type=module - "$TO" <<'NODE'
import { readFileSync, writeFileSync } from 'node:fs';

const version = process.argv[2];
const lockfiles = [
  'bindings/node/package-lock.json',
  'bindings/wasm/package-lock.json',
  'cli/package-lock.json',
  'admin/package-lock.json',
  'examples/node/package-lock.json',
];

for (const path of lockfiles) {
  const lock = JSON.parse(readFileSync(path, 'utf8'));
  lock.version = version;
  for (const [packagePath, metadata] of Object.entries(lock.packages ?? {})) {
    if ((packagePath === '' || packagePath.startsWith('../')) && metadata.version) {
      metadata.version = version;
    }
  }
  writeFileSync(path, `${JSON.stringify(lock, null, 2)}\n`);
}
NODE

echo "==> Regenerating Cargo.lock"
cargo metadata --format-version 1 >/dev/null

echo "==> Regenerating inventories"
node ./scripts/ci/generate_workspace_inventory.mjs >/dev/null
node ./scripts/ci/generate_binding_api_inventory.mjs >/dev/null

cat <<EOF

Mechanical bump complete. Release flow from here:
  1. Add the ${TO} entry to CHANGELOG.md
  2. bash ./scripts/ci/check_release_hygiene.sh
  3. Open a PR and land it on master — a release is cut from master only
  4. npm run release:tag -- ${TO}
       Creates v${TO}, cli-v${TO} and py-v${TO} and pushes all three in ONE
       push, and only from a clean tree at origin/master whose required checks
       are green. Run it with --dry-run first to see the preconditions.
       (v1.31.0 and v1.32.0 shipped with only v${TO}-style tags, so npm and
       PyPI stayed two releases behind; v1.34.0 was tagged off a branch with a
       red check. This command is what makes both impossible.)
  5. Watch the three publish workflows, then:
       bash scripts/release-bump.sh --sync-locks   # after the npm publishes land
  6. Commit the lockfile sync (bindings/node only, in practice)
EOF
