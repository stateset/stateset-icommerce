/**
 * Fulfillment (waves and pick tasks) tests for @stateset/embedded Node.js
 * bindings.
 *
 * A wave is a batch of orders released to the floor together:
 * draft -> released -> completed, cancellable until completed. Wrong-state
 * transitions are refused with `CONFLICT` (the engine's `Conflict` variant,
 * "cannot <action> wave <id>: status is <status>"); a wave with open picks is
 * refused with `VALIDATION`. The binding exposes no pick-creation path, so
 * pick transitions are exercised against ids the engine does not know.
 */

'use strict';

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

const UNKNOWN_ID = '3f2504e0-4f89-41d3-9a0c-0305e82c3301';

async function setup(commerce, { orders = 1 } = {}) {
  const customer = await commerce.customers.create({
    email: 'wave@example.com',
    firstName: 'W',
    lastName: 'V',
  });
  const warehouse = await commerce.warehouse.createWarehouse({ code: 'WH-F', name: 'Fulfil' });
  const orderIds = [];
  for (let i = 0; i < orders; i += 1) {
    const order = await commerce.orders.create({
      customerId: customer.id,
      items: [{ sku: `WAVE-${i}`, name: `Wave item ${i}`, quantity: 1, unitPriceExact: '5.00' }],
    });
    orderIds.push(order.id);
  }
  return { customer, warehouse, orderIds };
}

test('createWave -> getWave -> listWaves round-trips a draft wave', async () => {
  const commerce = new Commerce(':memory:');
  const { warehouse, orderIds } = await setup(commerce, { orders: 2 });

  const wave = await commerce.fulfillment.createWave({
    warehouseId: warehouse.id,
    orderIds,
    priority: 3,
    notes: 'morning batch',
  });
  assert.ok(wave.id);
  assert.match(wave.waveNumber, /^WV-/);
  assert.equal(wave.warehouseId, warehouse.id);
  assert.equal(wave.orderCount, 2);
  assert.equal(wave.status, 'Draft');
  assert.ok(!Number.isNaN(Date.parse(wave.createdAt)));

  const fetched = await commerce.fulfillment.getWave(wave.id);
  assert.deepEqual(fetched, wave);

  const listed = await commerce.fulfillment.listWaves();
  assert.equal(listed.length, 1);
  assert.equal(listed[0].id, wave.id);
  assert.equal(await commerce.fulfillment.countWaves(), 1);
});

test('getWave returns null for an unknown id', async () => {
  const commerce = new Commerce(':memory:');
  assert.equal(await commerce.fulfillment.getWave(UNKNOWN_ID), null);
});

test('releaseWave then completeWave walks draft -> released -> completed', async () => {
  const commerce = new Commerce(':memory:');
  const { warehouse, orderIds } = await setup(commerce);
  const wave = await commerce.fulfillment.createWave({ warehouseId: warehouse.id, orderIds });

  const released = await commerce.fulfillment.releaseWave(wave.id);
  assert.equal(released.status, 'Released');
  assert.equal(released.id, wave.id);

  const completed = await commerce.fulfillment.completeWave(wave.id);
  assert.equal(completed.status, 'Completed');

  const fetched = await commerce.fulfillment.getWave(wave.id);
  assert.equal(fetched.status, 'Completed');
});

test('completeWave on a draft wave is refused with CONFLICT and leaves it draft', async () => {
  const commerce = new Commerce(':memory:');
  const { warehouse, orderIds } = await setup(commerce);
  const wave = await commerce.fulfillment.createWave({ warehouseId: warehouse.id, orderIds });

  await assert.rejects(
    commerce.fulfillment.completeWave(wave.id),
    (err) => err.code === 'CONFLICT' && /status is draft/.test(err.message),
  );
  assert.equal((await commerce.fulfillment.getWave(wave.id)).status, 'Draft');
});

test('releaseWave twice is refused with CONFLICT', async () => {
  const commerce = new Commerce(':memory:');
  const { warehouse, orderIds } = await setup(commerce);
  const wave = await commerce.fulfillment.createWave({ warehouseId: warehouse.id, orderIds });
  await commerce.fulfillment.releaseWave(wave.id);

  await assert.rejects(
    commerce.fulfillment.releaseWave(wave.id),
    (err) => err.code === 'CONFLICT' && /status is released/.test(err.message),
  );
});

test('cancelWave works from draft and from released, but not from completed', async () => {
  const commerce = new Commerce(':memory:');
  const { warehouse, orderIds } = await setup(commerce, { orders: 3 });

  const draft = await commerce.fulfillment.createWave({
    warehouseId: warehouse.id,
    orderIds: [orderIds[0]],
  });
  assert.equal((await commerce.fulfillment.cancelWave(draft.id)).status, 'Cancelled');

  const released = await commerce.fulfillment.createWave({
    warehouseId: warehouse.id,
    orderIds: [orderIds[1]],
  });
  await commerce.fulfillment.releaseWave(released.id);
  assert.equal((await commerce.fulfillment.cancelWave(released.id)).status, 'Cancelled');

  const done = await commerce.fulfillment.createWave({
    warehouseId: warehouse.id,
    orderIds: [orderIds[2]],
  });
  await commerce.fulfillment.releaseWave(done.id);
  await commerce.fulfillment.completeWave(done.id);
  await assert.rejects(
    commerce.fulfillment.cancelWave(done.id),
    (err) => err.code === 'CONFLICT' && /status is completed/.test(err.message),
  );

  // A cancelled wave cannot be released again.
  await assert.rejects(
    commerce.fulfillment.releaseWave(draft.id),
    (err) => err.code === 'CONFLICT' && /status is cancelled/.test(err.message),
  );

  const statuses = (await commerce.fulfillment.listWaves()).map((w) => w.status).sort();
  assert.deepEqual(statuses, ['Cancelled', 'Cancelled', 'Completed']);
  assert.equal(await commerce.fulfillment.countWaves(), 3);
});

test('wave transitions on an unknown wave id are NOT_FOUND', async () => {
  const commerce = new Commerce(':memory:');
  for (const op of ['releaseWave', 'completeWave', 'cancelWave']) {
    await assert.rejects(
      commerce.fulfillment[op](UNKNOWN_ID),
      (err) => err.code === 'NOT_FOUND',
      `${op} should be NOT_FOUND`,
    );
  }
});

test('pick tasks: none exist for a fresh wave and transitions on unknown picks are NOT_FOUND', async () => {
  const commerce = new Commerce(':memory:');
  const { warehouse, orderIds } = await setup(commerce);
  await commerce.fulfillment.createWave({ warehouseId: warehouse.id, orderIds });

  assert.deepEqual(await commerce.fulfillment.listPicks(), []);
  assert.equal(await commerce.fulfillment.getPick(UNKNOWN_ID), null);

  await assert.rejects(
    commerce.fulfillment.assignPick(UNKNOWN_ID, 'picker-1'),
    (err) => err.code === 'NOT_FOUND',
  );
  await assert.rejects(commerce.fulfillment.startPick(UNKNOWN_ID), (err) => err.code === 'NOT_FOUND');
  await assert.rejects(commerce.fulfillment.cancelPick(UNKNOWN_ID), (err) => err.code === 'NOT_FOUND');
});

test('isOrderReadyToPack is vacuously true with no picks; isOrderReadyToShip needs a completed pack', async () => {
  const commerce = new Commerce(':memory:');
  const { orderIds } = await setup(commerce);
  assert.equal(await commerce.fulfillment.isOrderReadyToPack(orderIds[0]), true);
  assert.equal(await commerce.fulfillment.isOrderReadyToShip(orderIds[0]), false);
});

test('createWave with an unknown warehouse is refused with VALIDATION', async () => {
  const commerce = new Commerce(':memory:');
  const { orderIds } = await setup(commerce);
  await assert.rejects(
    commerce.fulfillment.createWave({ warehouseId: 9999, orderIds }),
    (err) => err.code === 'VALIDATION',
  );
  assert.equal(await commerce.fulfillment.countWaves(), 0);
});

test(
  'createWave with an order the store does not have is refused',
  { todo: 'engine: create_wave inserts wave_orders rows for any UUID; an unknown order id is accepted and orderCount counts it' },
  async () => {
    const commerce = new Commerce(':memory:');
    const warehouse = await commerce.warehouse.createWarehouse({ code: 'WH-X', name: 'X' });
    await assert.rejects(
      commerce.fulfillment.createWave({ warehouseId: warehouse.id, orderIds: [UNKNOWN_ID] }),
      (err) => err.code === 'NOT_FOUND' || err.code === 'VALIDATION',
    );
    assert.equal(await commerce.fulfillment.countWaves(), 0);
  },
);

test('malformed UUIDs are refused with VALIDATION before touching the engine', async () => {
  const commerce = new Commerce(':memory:');
  const warehouse = await commerce.warehouse.createWarehouse({ code: 'WH-V', name: 'V' });

  await assert.rejects(
    commerce.fulfillment.createWave({ warehouseId: warehouse.id, orderIds: ['not-a-uuid'] }),
    (err) => err.code === 'VALIDATION' && /order UUID 'not-a-uuid'/.test(err.message),
  );
  for (const op of ['getWave', 'releaseWave', 'completeWave', 'cancelWave', 'getPick', 'startPick', 'cancelPick', 'isOrderReadyToPack', 'isOrderReadyToShip']) {
    await assert.rejects(
      commerce.fulfillment[op]('nope'),
      (err) => err.code === 'VALIDATION' && /Invalid UUID/.test(err.message),
      `${op} should reject a malformed UUID`,
    );
  }
  await assert.rejects(
    commerce.fulfillment.assignPick('nope', 'picker'),
    (err) => err.code === 'VALIDATION',
  );
  assert.equal(await commerce.fulfillment.countWaves(), 0);
});
