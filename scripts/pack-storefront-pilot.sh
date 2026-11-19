#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 && $# -ne 3 ]] || [[ $# -eq 3 && $2 != --bundle ]]; then
  echo "Usage: bash scripts/pack-storefront-pilot.sh NEW_OUTPUT_DIRECTORY [--bundle VERIFIED_SDK_TAR]" >&2
  exit 2
fi
output=$(realpath -m -- "$1")
if [[ -e "$output" || -L "$output" ]]; then
  echo "Choose a new output directory." >&2
  exit 1
fi
shift
repo=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
revision=$(git -C "$repo" rev-parse HEAD)
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
mkdir "$work/source"
# Generate from committed sources in an empty directory so a merchant's local
# configuration, store data, and npm configuration cannot enter the archive.
git -C "$repo" archive "$revision" packages/create-stateset-app scripts/create-storefront-preview.mjs \
  scripts/storefront-pilot scripts/storefront-native-notices/linux-x64-gnu-notices.tar.gz \
  LICENSE LICENSE-MIT LICENSE-APACHE | tar -x -C "$work/source"
project="$work/stateset-merchant-pilot"
node "$work/source/scripts/create-storefront-preview.mjs" "$project" --lock-only "$@"
cp "$work/source/scripts/storefront-pilot/"{README.md,compose.yaml} "$project/"
cp "$work/source/"{LICENSE,LICENSE-MIT,LICENSE-APACHE} "$project/"
cp "$work/source/packages/create-stateset-app/README.md" "$project/MERCHANT-GUIDE.md"
printf '%s\n' "$revision" > "$project/STOREFRONT-REVISION"

name="stateset-merchant-pilot-linux-x64-gnu-${revision:0:12}.tar.gz"
# Ship only tracked template files and the known outputs of this packaging run.
{
  git -C "$repo" ls-tree -r --name-only -z "$revision:packages/create-stateset-app/templates/storefront" |
    while IFS= read -r -d '' entry; do
      if [[ "$entry" == gitignore ]]; then entry=.gitignore; fi
      printf 'stateset-merchant-pilot/%s\0' "$entry"
    done
  for entry in .env.example package-lock.json README.md MERCHANT-GUIDE.md LICENSE LICENSE-MIT \
    LICENSE-APACHE STOREFRONT-REVISION compose.yaml; do
    printf 'stateset-merchant-pilot/%s\0' "$entry"
  done
  (cd "$work" && find stateset-merchant-pilot/vendor/stateset -type f -print0)
} | LC_ALL=C sort -zu | tar -C "$work" --mtime=@0 --owner=0 --group=0 --numeric-owner \
  --null -czf "$work/$name" -T -
(cd "$work" && sha256sum "$name" > SHA256SUMS)
mkdir -- "$output"
mv -- "$work/$name" "$work/SHA256SUMS" "$output/"
echo "Pilot archive and checksum are in $output"
