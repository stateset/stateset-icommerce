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

docker run --rm -i --network none --mount "$mount" "$operator_image" node --input-type=module <<'JS'
import { Commerce } from '@stateset/embedded';
const commerce = new Commerce(process.env.STATESET_DB_PATH);
try {
  for (let index = 0; index < 101; index++) {
    const product = await commerce.products.create({ name: 'A catalog item ' + index });
    await commerce.products.activate(product.id);
  }
  const late = await commerce.products.create({
    name: 'Z catalog item', slug: 'autumn-tea', category: 'Seasonal', description: 'Late-page jasmine',
    variants: [{ sku: 'JASMINE', priceExact: '12.10', isDefault: true }],
  });
  await commerce.products.activate(late.id);
  await commerce.products.create({
    name: 'Private staging marker', slug: 'staged-only', description: 'unpublished inventory',
    variants: [{ sku: 'STAGED', priceExact: '9.00', isDefault: true }],
  });
} finally {
  await commerce.close();
}
JS

web=$(docker run --rm -d --network none --mount "$mount" "$web_image")
docker exec -i "$web" node --input-type=module <<'JS'
import assert from 'node:assert/strict';
import { setTimeout as wait } from 'node:timers/promises';
assert.notEqual(process.getuid(), 0);
for (let attempt = 0; ; attempt++) {
  let response;
  try {
    response = await fetch('http://127.0.0.1:3000/api/products?search=Classic%20T-Shirt&limit=1', { signal: AbortSignal.timeout(3000) });
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
const origin = 'http://127.0.0.1:3000';
for (const params of ['search=JASMINE', 'category=seasonal']) {
  const response = await fetch(origin + '/api/products?limit=1&' + params);
  assert.equal(response.status, 200);
  assert.deepEqual((await response.json()).products.map((p) => p.slug), ['autumn-tea']);
}
const drafts = await fetch(origin + '/api/products?search=staging');
assert.deepEqual((await drafts.json()).products, []);
const detail = await fetch(origin + '/products/staged-only');
assert.ok(!(await detail.text()).includes('Private staging marker'));
const legacy = await fetch(origin + '/products/z-catalog-item');
assert.equal(legacy.status, 200);
assert.ok((await legacy.text()).includes('Late-page jasmine'));
const cart = await fetch(origin + '/api/cart', {
  method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ sku: 'STAGED' }),
});
assert.equal(cart.status, 404);
console.log('Unprivileged storefront serves the operator catalog, searches across pages, and excludes drafts from purchase.');
JS
