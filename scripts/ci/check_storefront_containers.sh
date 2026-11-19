#!/usr/bin/env bash
set -euo pipefail

operator_image="${1:?Pass an operator image}"
web_image="${2:?Pass a web image}"
volume=$(docker volume create)
web=""
cleanup() {
  if [[ -n "$web" ]]; then docker stop "$web" >/dev/null || true; fi
  docker volume rm "$volume" >/dev/null
}
trap cleanup EXIT
mount="type=volume,source=$volume,target=/var/lib/stateset"

docker run --rm --network none --mount "$mount" "$operator_image" npm run setup -- --apply >/dev/null
docker run --rm --network none --mount "$mount" "$operator_image" npm run seed >/dev/null
docker run --rm --network none --mount "$mount" "$operator_image" node scripts/merchant-worker.js >/dev/null

web=$(docker run --rm -d --network none --mount "$mount" "$web_image")
docker exec -i "$web" node --input-type=module <<'JS'
import assert from 'node:assert/strict';
import { setTimeout as wait } from 'node:timers/promises';
assert.notEqual(process.getuid(), 0);
for (let attempt = 0; ; attempt++) {
  let response;
  try {
    response = await fetch('http://127.0.0.1:3000/api/products?limit=30', { signal: AbortSignal.timeout(3000) });
  } catch (error) {
    if (attempt === 29) throw error;
    await wait(500);
    continue;
  }
  assert.equal(response.status, 200);
  const { products } = await response.json();
  assert.ok(products.some((product) => product.slug === 'classic-t-shirt'), 'Web storefront should serve the catalog created by the operator');
  break;
}
console.log('Unprivileged storefront serves the persisted operator catalog.');
JS
