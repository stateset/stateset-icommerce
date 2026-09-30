import { test } from 'node:test';
import assert from 'node:assert/strict';
import { planPartialShipment } from '../../src/tools/fulfillment-recovery.js';

function fixture(overrides = {}) {
  const order = {
    id: 'order-1',
    version: 4,
    status: 'partially_shipped',
    items: [
      { id: 'line-1', sku: 'W-1', name: 'Widget', quantity: 5, shippedQuantity: 2 },
      { id: 'line-2', sku: 'W-2', name: 'Other', quantity: 1, shippedQuantity: 1 },
    ],
    ...overrides,
  };
  return {
    orders: { get: async () => structuredClone(order) },
    shipments: {
      get: async (id) => ({ id, orderId: 'order-1', version: 2, status: 'shipped' }),
      create: async () => assert.fail('Planning must never create shipments'),
    },
  };
}

test('derives remaining units from persisted fulfillment and records observed versions', async () => {
  const plan = await planPartialShipment(fixture(), {
    orderId: 'order-1',
    shipmentId: 'shipment-1',
  });
  assert.equal(plan.items.length, 1);
  assert.equal(plan.items[0].quantity, 3);
  assert.equal(plan.items[0].shippedQuantity, 2);
  assert.equal(plan.orderVersion, 4);
  assert.equal(plan.parentShipment.version, 2);
  assert.equal(plan.snapshotOnly, true);
  assert.equal(plan.executable, false);
});

test('aggregates duplicate requested lines without exceeding remaining quantity', async () => {
  const plan = await planPartialShipment(fixture(), {
    orderId: 'order-1',
    remainingItems: [
      { sku: 'W-1', quantity: 1 },
      { orderItemId: 'line-1', quantity: 2 },
    ],
  });
  assert.equal(plan.items.length, 1);
  assert.equal(plan.items[0].quantity, 3);
});

for (const [name, items] of [
  ['excess', [{ sku: 'W-1', quantity: 4 }]],
  [
    'duplicate excess',
    [
      { sku: 'W-1', quantity: 2 },
      { sku: 'W-1', quantity: 2 },
    ],
  ],
  ['fulfilled line', [{ sku: 'W-2', quantity: 1 }]],
  ['missing line', [{ orderItemId: 'other-order-line', quantity: 1 }]],
  ['mismatched SKU', [{ orderItemId: 'line-1', sku: 'W-2', quantity: 1 }]],
  ['zero', [{ sku: 'W-1', quantity: 0 }]],
  ['negative', [{ sku: 'W-1', quantity: -1 }]],
  ['fraction', [{ sku: 'W-1', quantity: 0.5 }]],
  ['unsafe integer', [{ sku: 'W-1', quantity: Number.MAX_SAFE_INTEGER + 1 }]],
  ['numeric string', [{ sku: 'W-1', quantity: '1' }]],
  ['empty selection', []],
]) {
  test(`rejects ${name} requests`, async () => {
    await assert.rejects(
      planPartialShipment(fixture(), { orderId: 'order-1', remainingItems: items }),
    );
  });
}

test('requires item identity when an order repeats a SKU', async () => {
  const commerce = fixture({
    items: [
      { id: 'a', sku: 'SAME', quantity: 2, shippedQuantity: 1 },
      { id: 'b', sku: 'SAME', quantity: 3, shippedQuantity: 0 },
    ],
  });
  await assert.rejects(
    planPartialShipment(commerce, {
      orderId: 'order-1',
      remainingItems: [{ sku: 'SAME', quantity: 1 }],
    }),
    /ambiguous/,
  );
  const plan = await planPartialShipment(commerce, {
    orderId: 'order-1',
    remainingItems: [{ orderItemId: 'a', quantity: 1 }],
  });
  assert.equal(plan.items[0].orderItemId, 'a');
});

for (const shippedQuantity of [undefined, -1, 6, 0.5]) {
  test(`refuses untrustworthy shipped quantity ${shippedQuantity}`, async () => {
    const commerce = fixture({
      items: [{ id: 'line-1', sku: 'W-1', quantity: 5, shippedQuantity }],
    });
    await assert.rejects(planPartialShipment(commerce, { orderId: 'order-1' }), /reconcile first/);
  });
}

test('does not infer a replacement shipment for an already fulfilled order', async () => {
  const plan = await planPartialShipment(
    fixture({
      status: 'shipped',
      items: [{ id: 'line-1', sku: 'W-1', quantity: 5, shippedQuantity: 5 }],
    }),
    { orderId: 'order-1' },
  );
  assert.deepEqual(plan.items, []);
  assert.equal(plan.status, 'nothing_to_fulfill');
});

test('rejects missing orders, closed orders, missing shipments and cross-order shipments', async () => {
  const commerce = fixture();
  commerce.orders.get = async () => null;
  await assert.rejects(planPartialShipment(commerce, { orderId: 'order-1' }), /Order not found/);
  for (const status of ['cancelled', 'refunded']) {
    await assert.rejects(
      planPartialShipment(fixture({ status }), { orderId: 'order-1' }),
      /Cannot plan/,
    );
  }
  const missing = fixture();
  missing.shipments.get = async () => null;
  await assert.rejects(
    planPartialShipment(missing, { orderId: 'order-1', shipmentId: 's' }),
    /Shipment not found/,
  );
  const foreign = fixture();
  foreign.shipments.get = async () => ({ id: 's', orderId: 'other-order' });
  await assert.rejects(
    planPartialShipment(foreign, { orderId: 'order-1', shipmentId: 's' }),
    /does not belong/,
  );
});
