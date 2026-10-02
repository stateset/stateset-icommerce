/**
 * Backorder tests for @stateset/embedded Node.js bindings (`commerce.backorder`).
 *
 * An order placed under `stockPolicy: 'allow_backorder'` reserves what is on
 * the shelf and books the shortfall as a backorder in the same transaction.
 * Backorders can also be raised by hand. A pending backorder can be
 * cancelled (idempotently); a fulfilled one cannot. Quantities cross the
 * boundary as numbers but are exact decimals in the engine.
 */

'use strict';

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

const UNKNOWN_ID = '3f2504e0-4f89-41d3-9a0c-0305e82c3301';

async function setup(commerce, { sku = 'BO-SKU', onHand = 3 } = {}) {
  const customer = await commerce.customers.create({
    email: 'bo@example.com',
    firstName: 'B',
    lastName: 'O',
  });
  await commerce.inventory.createItem({ sku, name: 'Backorderable', initialQuantity: onHand });
  return { customer, sku };
}

async function orderFor(commerce, customer, sku, quantity, stockPolicy = 'allow_backorder') {
  return commerce.orders.create({
    customerId: customer.id,
    stockPolicy,
    items: [{ sku, name: 'Backorderable', quantity, unitPriceExact: '4.00' }],
  });
}

test('an order exceeding stock under allow_backorder books the shortfall as a backorder', async () => {
  const commerce = new Commerce(':memory:');
  const { customer, sku } = await setup(commerce, { onHand: 3 });

  const order = await orderFor(commerce, customer, sku, 5);
  assert.equal(order.status, 'pending');
  assert.equal(order.totalAmountExact, '20.00');

  // The 3 on hand are reserved for the order; 2 are short.
  const stock = await commerce.inventory.getStock(sku);
  assert.equal(stock.totalOnHand, '3');
  assert.equal(stock.totalAllocated, '3');
  assert.equal(stock.totalAvailable, '0');

  const backorders = await commerce.backorder.getBackordersForOrder(order.id);
  assert.equal(backorders.length, 1);
  const [bo] = backorders;
  assert.match(bo.backorderNumber, /^BO-/);
  assert.equal(bo.orderId, order.id);
  assert.equal(bo.customerId, customer.id);
  assert.equal(bo.sku, sku);
  assert.equal(bo.quantityOrdered, 2);
  assert.equal(bo.quantityFulfilled, 0);
  assert.equal(bo.quantityRemaining, 2);
  assert.equal(bo.status, 'Pending');
  assert.equal(bo.priority, 'Normal');

  assert.equal(await commerce.backorder.countPending(), 1);
  const summary = await commerce.backorder.getSummary();
  assert.equal(summary.totalBackorders, 1);
  assert.equal(summary.totalValue, 2); // units on backorder, not money
  assert.equal(summary.criticalCount, 0);
  assert.equal(summary.overdueCount, 0);
});

test('an order that fits on-hand stock raises no backorder', async () => {
  const commerce = new Commerce(':memory:');
  const { customer, sku } = await setup(commerce, { onHand: 3 });
  const order = await orderFor(commerce, customer, sku, 3);
  assert.deepEqual(await commerce.backorder.getBackordersForOrder(order.id), []);
  assert.equal(await commerce.backorder.countPending(), 0);
  assert.equal((await commerce.inventory.getStock(sku)).totalAvailable, '0');
});

test('reject_if_insufficient refuses the order with INSUFFICIENT_STOCK and books nothing', async () => {
  const commerce = new Commerce(':memory:');
  const { customer, sku } = await setup(commerce, { onHand: 3 });
  await assert.rejects(
    orderFor(commerce, customer, sku, 5, 'reject_if_insufficient'),
    (err) =>
      err.code === 'INSUFFICIENT_STOCK' &&
      err.details.sku === sku &&
      err.details.requested === '5' &&
      err.details.available === '3',
  );
  assert.deepEqual(await commerce.backorder.listBackorders(), []);
  assert.equal((await commerce.inventory.getStock(sku)).totalAllocated, '0');
});

test('createBackorder -> getBackorder -> getBackorderByNumber -> listBackorders', async () => {
  const commerce = new Commerce(':memory:');
  const { customer, sku } = await setup(commerce);
  const order = await orderFor(commerce, customer, sku, 1);

  const bo = await commerce.backorder.createBackorder({
    orderId: order.id,
    customerId: customer.id,
    sku,
    quantity: 2.5,
    priority: 'critical',
    notes: 'customer escalated',
  });
  assert.ok(bo.id);
  assert.match(bo.backorderNumber, /^BO-/);
  assert.equal(bo.quantityOrdered, 2.5);
  assert.equal(bo.quantityFulfilled, 0);
  assert.equal(bo.quantityRemaining, 2.5);
  assert.equal(bo.status, 'Pending');
  assert.equal(bo.priority, 'Critical');
  assert.ok(!Number.isNaN(Date.parse(bo.createdAt)));

  assert.deepEqual(await commerce.backorder.getBackorder(bo.id), bo);
  assert.deepEqual(await commerce.backorder.getBackorderByNumber(bo.backorderNumber), bo);
  assert.equal(await commerce.backorder.getBackorder(UNKNOWN_ID), null);
  assert.equal(await commerce.backorder.getBackorderByNumber('BO-NOPE'), null);

  const listed = await commerce.backorder.listBackorders();
  assert.equal(listed.length, 1);
  assert.equal(listed[0].id, bo.id);
});

test('priority strings map onto the engine enum, absent is Normal, unknown is refused', async () => {
  const commerce = new Commerce(':memory:');
  const { customer, sku } = await setup(commerce);
  const order = await orderFor(commerce, customer, sku, 1);
  const cases = [
    ['low', 'Low'],
    ['normal', 'Normal'],
    ['high', 'High'],
    ['critical', 'Critical'],
    ['CRITICAL', 'Critical'],
    [undefined, 'Normal'],
  ];
  for (const [priority, expected] of cases) {
    const bo = await commerce.backorder.createBackorder({
      orderId: order.id,
      customerId: customer.id,
      sku,
      quantity: 1,
      priority,
    });
    assert.equal(bo.priority, expected, `priority '${priority}'`);
  }
  await assert.rejects(
    commerce.backorder.createBackorder({ orderId: order.id, customerId: customer.id, sku, quantity: 1, priority: 'whenever' }),
    (err) => err.code === 'VALIDATION' && /backorder priority 'whenever'/.test(err.message),
  );
  assert.equal((await commerce.backorder.listBackorders()).length, cases.length);
});

test('getBackordersForSku and getSummary aggregate across orders', async () => {
  const commerce = new Commerce(':memory:');
  const { customer, sku } = await setup(commerce, { onHand: 0 });
  await commerce.inventory.createItem({ sku: 'OTHER', name: 'Other', initialQuantity: 0 });

  const o1 = await orderFor(commerce, customer, sku, 4);
  const o2 = await orderFor(commerce, customer, sku, 2);
  const o3 = await orderFor(commerce, customer, 'OTHER', 7);
  await commerce.backorder.createBackorder({
    orderId: o1.id,
    customerId: customer.id,
    sku,
    quantity: 0.5,
    priority: 'critical',
  });

  const forSku = await commerce.backorder.getBackordersForSku(sku);
  assert.equal(forSku.length, 3);
  assert.deepEqual(
    forSku.map((b) => b.quantityRemaining).sort((a, b) => a - b),
    [0.5, 2, 4],
  );
  assert.deepEqual(await commerce.backorder.getBackordersForSku('NOPE'), []);
  assert.equal((await commerce.backorder.getBackordersForOrder(o2.id)).length, 1);
  assert.equal((await commerce.backorder.getBackordersForOrder(o3.id))[0].quantityRemaining, 7);

  const summary = await commerce.backorder.getSummary();
  assert.equal(summary.totalBackorders, 4);
  assert.equal(summary.totalValue, 13.5); // 4 + 2 + 7 + 0.5 units
  assert.equal(summary.criticalCount, 1);
  assert.equal(summary.overdueCount, 0);
  assert.equal(await commerce.backorder.countPending(), 4);
});

test('cancelBackorder flips pending -> cancelled, is idempotent, and drops it from the pending count', async () => {
  const commerce = new Commerce(':memory:');
  const { customer, sku } = await setup(commerce, { onHand: 0 });
  const order = await orderFor(commerce, customer, sku, 3);
  const keep = await orderFor(commerce, customer, sku, 1); // stays open
  const [bo] = await commerce.backorder.getBackordersForOrder(order.id);
  assert.equal(bo.status, 'Pending');
  assert.equal(await commerce.backorder.countPending(), 2);

  const cancelled = await commerce.backorder.cancelBackorder(bo.id);
  assert.equal(cancelled.status, 'Cancelled');
  assert.equal(cancelled.quantityRemaining, 3); // quantity is not rewritten by a cancel

  const again = await commerce.backorder.cancelBackorder(bo.id);
  assert.equal(again.status, 'Cancelled');

  assert.equal(await commerce.backorder.countPending(), 1);
  const summary = await commerce.backorder.getSummary();
  assert.equal(summary.totalBackorders, 1);
  assert.equal(summary.totalValue, 1);
  // Still visible in the order's history.
  assert.equal((await commerce.backorder.getBackordersForOrder(order.id))[0].status, 'Cancelled');
  assert.equal((await commerce.backorder.getBackordersForOrder(keep.id))[0].status, 'Pending');

  await assert.rejects(commerce.backorder.cancelBackorder(UNKNOWN_ID), (err) => err.code === 'NOT_FOUND');
});

test(
  'getSummary reports zeros when no backorder is open',
  async () => {
    const commerce = new Commerce(':memory:');
    const empty = await commerce.backorder.getSummary();
    assert.deepEqual(empty, { totalBackorders: 0, criticalCount: 0, overdueCount: 0, totalValue: 0 });

    const { customer, sku } = await setup(commerce, { onHand: 0 });
    const order = await orderFor(commerce, customer, sku, 3);
    const [bo] = await commerce.backorder.getBackordersForOrder(order.id);
    await commerce.backorder.cancelBackorder(bo.id);
    assert.equal(await commerce.backorder.countPending(), 0);
    const afterCancel = await commerce.backorder.getSummary();
    assert.deepEqual(afterCancel, { totalBackorders: 0, criticalCount: 0, overdueCount: 0, totalValue: 0 });
  },
);

test('getOverdueBackorders is empty when no backorder carries an expected date', async () => {
  const commerce = new Commerce(':memory:');
  const { customer, sku } = await setup(commerce, { onHand: 0 });
  await orderFor(commerce, customer, sku, 1);
  assert.deepEqual(await commerce.backorder.getOverdueBackorders(), []);
  assert.equal((await commerce.backorder.getSummary()).overdueCount, 0);
});

test(
  'a later stock receipt fulfils the pending backorder',
  { todo: 'engine: receiving stock (inventory.adjust / completeReceiving) never allocates or fulfils pending backorders. The capability is now reachable -- call backorder.autoAllocateInventory(sku) after a receipt -- but nothing triggers it automatically, and whether a receipt should is a design decision' },
  async () => {
    const commerce = new Commerce(':memory:');
    const { customer, sku } = await setup(commerce, { onHand: 3 });
    const order = await orderFor(commerce, customer, sku, 5);
    const [before] = await commerce.backorder.getBackordersForOrder(order.id);
    assert.equal(before.quantityRemaining, 2);

    await commerce.inventory.adjust(sku, 10, 'Supplier receipt');
    assert.equal((await commerce.inventory.getStock(sku)).totalOnHand, '13');

    const [after] = await commerce.backorder.getBackordersForOrder(order.id);
    assert.equal(after.quantityFulfilled, 2);
    assert.equal(after.quantityRemaining, 0);
    assert.equal(after.status, 'Fulfilled');
    assert.equal(await commerce.backorder.countPending(), 0);
    // The freed units are now held for the order, not available to others.
    assert.equal((await commerce.inventory.getStock(sku)).totalAllocated, '5');
  },
);

test(
  'createBackorder refuses a non-positive quantity',
  async () => {
    const commerce = new Commerce(':memory:');
    const { customer, sku } = await setup(commerce);
    const order = await orderFor(commerce, customer, sku, 1);
    for (const quantity of [0, -1]) {
      await assert.rejects(
        commerce.backorder.createBackorder({ orderId: order.id, customerId: customer.id, sku, quantity }),
        (err) => err.code === 'VALIDATION',
        `quantity ${quantity} should be refused`,
      );
    }
    assert.deepEqual(await commerce.backorder.listBackorders(), []);
  },
);

test('malformed UUIDs and a NaN quantity are refused with VALIDATION', async () => {
  const commerce = new Commerce(':memory:');
  const { customer, sku } = await setup(commerce);
  const order = await orderFor(commerce, customer, sku, 1);

  await assert.rejects(
    commerce.backorder.createBackorder({ orderId: 'not-a-uuid', customerId: customer.id, sku, quantity: 1 }),
    (err) => err.code === 'VALIDATION' && /order UUID/i.test(err.message),
  );
  await assert.rejects(
    commerce.backorder.createBackorder({ orderId: order.id, customerId: 'not-a-uuid', sku, quantity: 1 }),
    (err) => err.code === 'VALIDATION' && /customer UUID/i.test(err.message),
  );
  await assert.rejects(
    commerce.backorder.createBackorder({ orderId: order.id, customerId: customer.id, sku, quantity: NaN }),
    (err) => err.code === 'VALIDATION' && /quantity/i.test(err.message),
  );
  for (const op of ['getBackorder', 'cancelBackorder', 'getBackordersForOrder']) {
    await assert.rejects(
      commerce.backorder[op]('nope'),
      (err) => err.code === 'VALIDATION' && /Invalid UUID/.test(err.message),
      `${op} should reject a malformed UUID`,
    );
  }
  assert.deepEqual(await commerce.backorder.listBackorders(), []);
});

test('autoAllocateInventory fills an open backorder once stock arrives', async () => {
  const commerce = new Commerce(':memory:');
  const { customer, sku } = await setup(commerce, { onHand: 3 });

  const order = await orderFor(commerce, customer, sku, 5);
  const [backorder] = await commerce.backorder.getBackordersForOrder(order.id);
  assert.equal(backorder.quantityRemaining, 2);

  // Nothing is available yet, so there is nothing to allocate.
  assert.deepEqual(await commerce.backorder.autoAllocateInventory(sku), []);

  await commerce.inventory.adjust(sku, 5, 'receipt');

  const allocations = await commerce.backorder.autoAllocateInventory(sku);
  assert.equal(allocations.length, 1, 'the one open backorder is allocated');
  const [allocation] = allocations;
  assert.equal(allocation.backorderId, backorder.id);
  assert.equal(allocation.sku, sku);
  assert.equal(allocation.quantity, 2, 'allocated up to what the backorder still needs');
  assert.ok(
    ['Reserved', 'Confirmed'].includes(allocation.status),
    `unexpected allocation status ${allocation.status}`,
  );
  assert.ok(Date.parse(allocation.allocatedAt) > 0, 'allocatedAt is a timestamp');

  // A second sweep has nothing left to do.
  assert.deepEqual(await commerce.backorder.autoAllocateInventory(sku), []);
});

test('autoAllocateInventory on an unknown sku allocates nothing', async () => {
  const commerce = new Commerce(':memory:');
  assert.deepEqual(await commerce.backorder.autoAllocateInventory('NO-SUCH-SKU'), []);
});
