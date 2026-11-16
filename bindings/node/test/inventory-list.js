/**
 * `inventory.list`: every inventory item in SKU order with its stock summed
 * over locations, filtered by SKU text, active flag and the engine's reorder
 * rule, and paged by `limit`/`offset` (a low-stock page counts matching items
 * only).
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

async function store() {
  const commerce = new Commerce(':memory:');
  // B-2 sits below its reorder point; A-1 and C-3 have no reorder pressure.
  await commerce.inventory.createItem({
    sku: 'B-2',
    name: 'Bracket',
    initialQuantity: 2,
    reorderPoint: 5,
  });
  await commerce.inventory.createItem({ sku: 'A-1', name: 'Anchor', initialQuantity: 10 });
  await commerce.inventory.createItem({ sku: 'C-3', name: 'Clamp' });
  await commerce.inventory.reserve('A-1', 3, 'order', 'order-1');
  return commerce;
}

const skus = (items) => items.map((item) => item.sku);

test('inventory.list lists items with exact stock in SKU order', async () => {
  const commerce = await store();
  const items = await commerce.inventory.list();
  assert.deepEqual(skus(items), ['A-1', 'B-2', 'C-3']);
  const [anchor, bracket, clamp] = items;
  assert.equal(anchor.name, 'Anchor');
  assert.equal(anchor.isActive, true);
  assert.equal(typeof anchor.id, 'number');
  for (const field of ['onHand', 'allocated', 'available']) {
    assert.equal(typeof anchor[field], 'string', `${field} is an exact decimal string`);
  }
  assert.equal(Number(anchor.onHand), 10);
  assert.equal(Number(anchor.allocated), 3);
  assert.equal(Number(anchor.available), 7);
  assert.equal(Number(bracket.available), 2);
  assert.equal(Number(clamp.onHand), 0);
  assert.deepEqual(
    items.map((item) => item.lowStock),
    [false, true, false],
  );
});

test('inventory.list filters by SKU text, active flag and low stock', async () => {
  const commerce = await store();
  assert.deepEqual(skus(await commerce.inventory.list({ sku: 'b-' })), ['B-2']);
  assert.deepEqual(skus(await commerce.inventory.list({ isActive: true })), ['A-1', 'B-2', 'C-3']);
  assert.deepEqual(await commerce.inventory.list({ isActive: false }), []);
  assert.deepEqual(skus(await commerce.inventory.list({ lowStock: true })), ['B-2']);
  assert.deepEqual(skus(await commerce.inventory.list({ lowStock: false })), ['A-1', 'C-3']);
});

test('inventory.list pages, and a low-stock page counts matching items only', async () => {
  const commerce = await store();
  assert.deepEqual(skus(await commerce.inventory.list({ limit: 2 })), ['A-1', 'B-2']);
  assert.deepEqual(skus(await commerce.inventory.list({ limit: 2, offset: 2 })), ['C-3']);
  assert.deepEqual(skus(await commerce.inventory.list({ lowStock: false, limit: 1, offset: 1 })), [
    'C-3',
  ]);
  assert.deepEqual(skus(await commerce.inventory.list({ limit: 5000 })), ['A-1', 'B-2', 'C-3']);
});
