/**
 * Receiving (inbound receipts) tests for @stateset/embedded Node.js bindings.
 *
 * A receipt is the dock-side record of goods arriving:
 * expected -> in_progress -> received, cancellable until goods are received.
 * Wrong-state transitions are refused with `VALIDATION` and the message names
 * the state the engine wanted. `createReceiptFromPo` copies the purchase
 * order's lines onto the receipt as expected quantities.
 */

'use strict';

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

const UNKNOWN_ID = '3f2504e0-4f89-41d3-9a0c-0305e82c3301';

async function warehouse(commerce) {
  return commerce.warehouse.createWarehouse({ code: 'WH-R', name: 'Receiving dock' });
}

async function purchaseOrder(commerce, items) {
  const supplier = await commerce.purchaseOrders.createSupplier({ name: 'Acme Supply' });
  return commerce.purchaseOrders.create({ supplierId: supplier.id, items });
}

test('createReceipt -> getReceipt -> getReceiptByNumber -> listReceipts', async () => {
  const commerce = new Commerce(':memory:');
  const wh = await warehouse(commerce);

  const receipt = await commerce.receiving.createReceipt({
    receiptType: 'purchase_order',
    warehouseId: wh.id,
    carrier: 'UPS',
    trackingNumber: '1Z999AA10123456784',
  });
  assert.ok(receipt.id);
  assert.match(receipt.receiptNumber, /^RCV-/);
  assert.equal(receipt.receiptType, 'PurchaseOrder');
  assert.equal(receipt.warehouseId, wh.id);
  assert.equal(receipt.status, 'Expected');
  assert.equal(receipt.carrier, 'UPS');
  assert.equal(receipt.trackingNumber, '1Z999AA10123456784');
  assert.ok(!Number.isNaN(Date.parse(receipt.createdAt)));

  assert.deepEqual(await commerce.receiving.getReceipt(receipt.id), receipt);
  assert.deepEqual(await commerce.receiving.getReceiptByNumber(receipt.receiptNumber), receipt);

  const listed = await commerce.receiving.listReceipts();
  assert.equal(listed.length, 1);
  assert.equal(listed[0].id, receipt.id);
  assert.equal(await commerce.receiving.countReceipts(), 1);
});

test('unknown receipts read back as null', async () => {
  const commerce = new Commerce(':memory:');
  assert.equal(await commerce.receiving.getReceipt(UNKNOWN_ID), null);
  assert.equal(await commerce.receiving.getReceiptByNumber('RCV-NOPE'), null);
});

test('every receipt type maps to its engine variant', async () => {
  const commerce = new Commerce(':memory:');
  const wh = await warehouse(commerce);
  const cases = [
    ['purchase_order', 'PurchaseOrder'],
    ['po', 'PurchaseOrder'],
    ['return', 'Return'],
    ['customer_return', 'Return'],
    ['transfer', 'Transfer'],
    ['adjustment', 'Adjustment'],
  ];
  for (const [input, expected] of cases) {
    const r = await commerce.receiving.createReceipt({ receiptType: input, warehouseId: wh.id });
    assert.equal(r.receiptType, expected, `receiptType '${input}'`);
  }
  assert.equal(await commerce.receiving.countReceipts(), cases.length);
});

test('startReceiving then completeReceiving walks expected -> in_progress -> received', async () => {
  const commerce = new Commerce(':memory:');
  const wh = await warehouse(commerce);
  const receipt = await commerce.receiving.createReceipt({ receiptType: 'po', warehouseId: wh.id });

  const started = await commerce.receiving.startReceiving(receipt.id);
  assert.equal(started.status, 'InProgress');
  assert.equal(started.id, receipt.id);
  assert.equal(started.receiptNumber, receipt.receiptNumber);

  const done = await commerce.receiving.completeReceiving(receipt.id);
  assert.equal(done.status, 'Received');
  assert.equal((await commerce.receiving.getReceipt(receipt.id)).status, 'Received');
});

test('completeReceiving on an expected receipt is refused with VALIDATION and leaves it expected', async () => {
  const commerce = new Commerce(':memory:');
  const wh = await warehouse(commerce);
  const receipt = await commerce.receiving.createReceipt({ receiptType: 'po', warehouseId: wh.id });

  await assert.rejects(
    commerce.receiving.completeReceiving(receipt.id),
    (err) => err.code === 'VALIDATION' && /in_progress/.test(err.message),
  );
  assert.equal((await commerce.receiving.getReceipt(receipt.id)).status, 'Expected');
});

test('startReceiving twice is refused with VALIDATION', async () => {
  const commerce = new Commerce(':memory:');
  const wh = await warehouse(commerce);
  const receipt = await commerce.receiving.createReceipt({ receiptType: 'po', warehouseId: wh.id });
  await commerce.receiving.startReceiving(receipt.id);

  await assert.rejects(
    commerce.receiving.startReceiving(receipt.id),
    (err) => err.code === 'VALIDATION' && /expected/.test(err.message),
  );
  assert.equal((await commerce.receiving.getReceipt(receipt.id)).status, 'InProgress');
});

test('cancelReceipt works from expected and in_progress, never once goods are received', async () => {
  const commerce = new Commerce(':memory:');
  const wh = await warehouse(commerce);

  const expected = await commerce.receiving.createReceipt({ receiptType: 'po', warehouseId: wh.id });
  assert.equal((await commerce.receiving.cancelReceipt(expected.id)).status, 'Cancelled');

  const inProgress = await commerce.receiving.createReceipt({ receiptType: 'po', warehouseId: wh.id });
  await commerce.receiving.startReceiving(inProgress.id);
  assert.equal((await commerce.receiving.cancelReceipt(inProgress.id)).status, 'Cancelled');

  const received = await commerce.receiving.createReceipt({ receiptType: 'po', warehouseId: wh.id });
  await commerce.receiving.startReceiving(received.id);
  await commerce.receiving.completeReceiving(received.id);
  await assert.rejects(
    commerce.receiving.cancelReceipt(received.id),
    (err) => err.code === 'VALIDATION' && /already received/.test(err.message),
  );
  assert.equal((await commerce.receiving.getReceipt(received.id)).status, 'Received');

  // A cancelled receipt cannot be restarted.
  await assert.rejects(
    commerce.receiving.startReceiving(expected.id),
    (err) => err.code === 'VALIDATION',
  );

  const statuses = (await commerce.receiving.listReceipts()).map((r) => r.status).sort();
  assert.deepEqual(statuses, ['Cancelled', 'Cancelled', 'Received']);
});

test('receipt transitions on an unknown id are NOT_FOUND', async () => {
  const commerce = new Commerce(':memory:');
  for (const op of ['startReceiving', 'completeReceiving', 'cancelReceipt']) {
    await assert.rejects(
      commerce.receiving[op](UNKNOWN_ID),
      (err) => err.code === 'NOT_FOUND',
      `${op} should be NOT_FOUND`,
    );
  }
});

test('createReceiptFromPo builds an expected receipt against an exact-money purchase order', async () => {
  const commerce = new Commerce(':memory:');
  const wh = await warehouse(commerce);
  const po = await purchaseOrder(commerce, [
    { sku: 'RAW-1', name: 'Raw one', quantity: 4, unitCost: 2.25 },
    { sku: 'RAW-2', name: 'Raw two', quantity: 3, unitCost: 10.1 },
  ]);
  // 4 x 2.25 + 3 x 10.10 = 9.00 + 30.30 = 39.30, exact through the PO.
  assert.equal(po.totalExact, '39.30');

  const receipt = await commerce.receiving.createReceiptFromPo(po.id, wh.id);
  assert.match(receipt.receiptNumber, /^RCV-/);
  assert.equal(receipt.receiptType, 'PurchaseOrder');
  assert.equal(receipt.warehouseId, wh.id);
  assert.equal(receipt.status, 'Expected');
  assert.equal(receipt.carrier, undefined);
  assert.equal(receipt.trackingNumber, undefined);

  // The PO-backed receipt walks the same state machine.
  await commerce.receiving.startReceiving(receipt.id);
  const done = await commerce.receiving.completeReceiving(receipt.id);
  assert.equal(done.status, 'Received');
  assert.equal(await commerce.receiving.countReceipts(), 1);
});

test('createReceipt can reference a purchase order explicitly', async () => {
  const commerce = new Commerce(':memory:');
  const wh = await warehouse(commerce);
  const po = await purchaseOrder(commerce, [{ sku: 'RAW-1', name: 'Raw', quantity: 1, unitCost: 1 }]);
  const receipt = await commerce.receiving.createReceipt({
    receiptType: 'po',
    warehouseId: wh.id,
    purchaseOrderId: po.id,
  });
  assert.equal(receipt.status, 'Expected');
  assert.equal(receipt.receiptType, 'PurchaseOrder');
});

test(
  'createReceiptFromPo refuses a purchase order the store does not have',
  async () => {
    const commerce = new Commerce(':memory:');
    const wh = await warehouse(commerce);
    await assert.rejects(
      commerce.receiving.createReceiptFromPo(UNKNOWN_ID, wh.id),
      (err) => err.code === 'NOT_FOUND',
    );
    assert.equal(await commerce.receiving.countReceipts(), 0);
  },
);

test('createReceipt with an unknown warehouse is refused with VALIDATION', async () => {
  const commerce = new Commerce(':memory:');
  await assert.rejects(
    commerce.receiving.createReceipt({ receiptType: 'po', warehouseId: 9999 }),
    (err) => err.code === 'VALIDATION',
  );
  assert.equal(await commerce.receiving.countReceipts(), 0);
});

test('malformed UUIDs are refused with VALIDATION before touching the engine', async () => {
  const commerce = new Commerce(':memory:');
  const wh = await warehouse(commerce);

  await assert.rejects(
    commerce.receiving.createReceiptFromPo('not-a-uuid', wh.id),
    (err) => err.code === 'VALIDATION' && /Invalid PO UUID/.test(err.message),
  );
  await assert.rejects(
    commerce.receiving.createReceipt({ receiptType: 'po', warehouseId: wh.id, purchaseOrderId: 'not-a-uuid' }),
    (err) => err.code === 'VALIDATION' && /purchase order UUID 'not-a-uuid'/.test(err.message),
  );
  for (const op of ['getReceipt', 'startReceiving', 'completeReceiving', 'cancelReceipt']) {
    await assert.rejects(
      commerce.receiving[op]('nope'),
      (err) => err.code === 'VALIDATION' && /Invalid UUID/.test(err.message),
      `${op} should reject a malformed UUID`,
    );
  }
  assert.equal(await commerce.receiving.countReceipts(), 0);
});
