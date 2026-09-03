#!/usr/bin/env bash
# Version-bump every synced release surface, in the right order, with the
# exclusions that hand-run seds kept getting wrong.
#
#   scripts/release-bump.sh 1.30.0 1.30.0
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
#     dependencies (@stateset/embedded, @stateset/cli, platform packages)
#     can only regenerate AFTER those versions are published to npm — before
#     that, npm errors with ETARGET. So the initial bump updates metadata only;
#     publish, then run `release-bump.sh --sync-locks` for full resolution.
#   * The untracked-at-the-time bindings/node/npm/ platform dirs were missed
#     by `git grep`; this script bumps them explicitly.
#
# After running: update CHANGELOG.md and the README "What's New" section by
# hand (content, not mechanics), then run the hygiene gate (done here last).
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

sync_locks() {
  cd "${ROOT_DIR}"
  echo "==> Regenerating npm lockfiles against the published registry versions"
  for d in bindings/node bindings/wasm cli admin examples/node; do
    (cd "$d" && npm install --package-lock-only)
    echo "    $d"
  done
  echo "==> Verifying bindings/node lock/manifest sync"
  (cd bindings/node && npm ci --dry-run >/dev/null)
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

# README "What's New" anchor tracks the version.
old_anchor="whats-new-in-v${FROM//./}"
new_anchor="whats-new-in-v${TO//./}"
sed -i "s/(#${old_anchor})/(#${new_anchor})/" README.md

echo "==> Regenerating Cargo.lock"
cargo metadata --format-version 1 >/dev/null

echo "==> Regenerating inventories"
node ./scripts/ci/generate_workspace_inventory.mjs >/dev/null
node ./scripts/ci/generate_binding_api_inventory.mjs >/dev/null

cat <<EOF

Mechanical bump complete. Release flow from here:
  1. Add the ${TO} entry to CHANGELOG.md
  2. Update the "What's New in v${TO}" section content in README.md
  3. bash ./scripts/ci/check_release_hygiene.sh
  4. Commit, tag (v${TO}, cli-v${TO}, py-v${TO}), push tags, wait for publishes
  5. bash scripts/release-bump.sh --sync-locks   # after npm publish lands
  6. Commit the lockfile sync
EOF
