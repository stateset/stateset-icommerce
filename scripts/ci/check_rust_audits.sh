#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

audit_tmp="$(mktemp -d)"
trap 'rm -rf "$audit_tmp"' EXIT
# Materialize discovery so a failed git command cannot become an empty,
# successful loop through process substitution.
git ls-files -z -- Cargo.lock '**/Cargo.lock' > "$audit_tmp/locks"
if [[ ! -s "$audit_tmp/locks" ]]; then
  echo '::error::No tracked Rust lockfiles were discovered.' >&2
  exit 1
fi

# These optional dependencies have no applicable compiled usage. Every
# manifest's complete normal/build graph is checked below before accepting
# the exceptions; standalone bindings must satisfy the same rule.
# rsa: RUSTSEC-2023-0071; rkyv 0.7: RUSTSEC-2026-0235.
status=0
fetch_args=()
while IFS= read -r -d '' lockfile; do
  manifest="${lockfile%Cargo.lock}Cargo.toml"
  if [[ ! -f "$lockfile" || ! -f "$manifest" ]]; then
    echo "::error::Missing lockfile or manifest: $lockfile / $manifest" >&2
    status=1
    continue
  fi
  echo "Auditing $lockfile"
  if ! cargo audit --file "$lockfile" --deny warnings \
      --ignore RUSTSEC-2023-0071 --ignore RUSTSEC-2026-0235 "${fetch_args[@]}"; then
    status=1
  fi
  # All lockfiles in this run use the same fetched advisory database. A
  # failed initial fetch already makes the overall check fail.
  fetch_args=(--no-fetch)
  if ! cargo tree --manifest-path "$manifest" --locked --workspace --all-features \
      --target all --edges normal,build --prefix none --format '{p}' > "$audit_tmp/graph"; then
    echo "::error file=$manifest::Unable to verify advisory exceptions in the build graph." >&2
    status=1
  elif grep -E '^(rsa|rkyv) ' "$audit_tmp/graph"; then
    echo "::error file=$manifest::An advisory-ignored package is compiled; remove its exception and remediate it." >&2
    status=1
  fi
done < "$audit_tmp/locks"
exit "$status"
