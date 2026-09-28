/**
 * Read APIs that used to be write-only from Node, and lines that used to be
 * dropped on the floor:
 *
 * - refunds: `getRefund` / `getRefunds`, the pending -> completed | failed
 *   lifecycle (`completeRefund` / `failRefund`) and the payment's
 *   `amountRefunded` that only settled refunds advance;
 * - the promotion usage ledger (`promotions.listUsage`);
 * - `orders.getByNumber` and the order's tax / shipping / discount amounts;
 * - bill, inspection and receipt lines supplied at create time and read back.
 */

'use strict';

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

const UNKNOWN_ID = '3f2504e0-4f89-41d3-9a0c-0305e82c3301';

async function customer(commerce, tag) {
  return commerce.customers.create({
    email: `${tag}-${Date.now()}@example.com`,
    firstName: 'Read',
    lastName: 'Api',
  });
}

async function order(commerce, cust, items) {
  return commerce.orders.createExact({
    customerId: cust.id,
    items: items ?? [{ sku: 'RA-1', name: 'Read API widget', quantity: 2, unitPrice: '10.00' }],
  });
}

test('refunds are readable and follow pending -> completed | failed', async () => {
  const commerce = new Commerce(':memory:');
  const cust = await customer(commerce, 'refund');
  const o = await order(commerce, cust);
  const payment = await commerce.payments.createExact({ orderId: o.id, amount: '20.00' });
  await commerce.payments.markCompleted(payment.id);
  assert.equal(payment.amountRefundedExact, '0');

  const first = await commerce.payments.createRefundExact({
    paymentId: payment.id,
    amount: '5.25',
    reason: 'damaged',
  });
  assert.equal(first.status, 'pending');
  assert.equal(first.amountExact, '5.25');
  assert.equal(first.currency, 'USD');
  assert.equal(first.refundedAt ?? null, null, 'unset until the refund settles');

  // A pending refund is reserved but not yet refunded.
  assert.equal((await commerce.payments.get(payment.id)).amountRefundedExact, '0');
  assert.deepEqual(await commerce.payments.getRefund(first.id), first);
  assert.deepEqual(
    (await commerce.payments.getRefunds(payment.id)).map((r) => r.id),
    [first.id],
  );

  const completed = await commerce.payments.completeRefund(first.id);
  assert.equal(completed.status, 'completed');
  assert.ok(!Number.isNaN(Date.parse(completed.refundedAt)));
  const afterFirst = await commerce.payments.get(payment.id);
  assert.equal(afterFirst.amountRefundedExact, '5.25');
  assert.equal(afterFirst.status, 'partially_refunded');

  const second = await commerce.payments.createRefundExact({
    paymentId: payment.id,
    amount: '1.00',
  });
  const failed = await commerce.payments.failRefund(second.id, 'processor declined');
  assert.equal(failed.status, 'failed');
  assert.equal(failed.failureReason, 'processor declined');
  assert.equal((await commerce.payments.get(payment.id)).amountRefundedExact, '5.25');
  await assert.rejects(
    () => commerce.payments.failRefund(first.id, 'too late'),
    (err) => err.code === 'VALIDATION',
    'a completed refund cannot fail',
  );

  const all = await commerce.payments.getRefunds(payment.id);
  assert.deepEqual(all.map((r) => r.status).sort(), ['completed', 'failed']);
  assert.equal(await commerce.payments.getRefund(UNKNOWN_ID), null);
  assert.deepEqual(await commerce.payments.getRefunds(UNKNOWN_ID), []);
  await assert.rejects(
    () => commerce.payments.getRefunds('nope'),
    (err) => err.code === 'VALIDATION',
  );
});

test('promotions.listUsage reads the usage ledger by order, promotion and customer', async () => {
  const commerce = new Commerce(':memory:');
  const alice = await customer(commerce, 'alice');
  const bob = await customer(commerce, 'bob');
  const orderA = await order(commerce, alice);
  const orderB = await order(commerce, bob);
  const promo = await commerce.promotions.activate(
    (
      await commerce.promotions.create({
        name: 'Ten off',
        promotionType: 'percentage_off',
        percentageOff: 0.1,
      })
    ).id,
  );
  const other = await commerce.promotions.activate(
    (
      await commerce.promotions.create({
        name: 'Five off',
        promotionType: 'fixed_amount_off',
        fixedAmountOff: 5,
      })
    ).id,
  );

  await commerce.promotions.recordUsage(promo.id, null, alice.id, orderA.id, null, 2, 'USD');
  await commerce.promotions.recordUsage(other.id, null, alice.id, orderA.id, null, 5, 'USD');
  await commerce.promotions.recordUsage(promo.id, null, bob.id, orderB.id, null, 2, 'USD');

  const all = await commerce.promotions.listUsage();
  assert.equal(all.length, 3);

  const forA = await commerce.promotions.listUsage({ orderId: orderA.id });
  assert.deepEqual(forA.map((u) => u.promotionId).sort(), [promo.id, other.id].sort());
  assert.deepEqual(forA.map((u) => u.discountAmountExact).sort(), ['2', '5']);

  const promoUses = await commerce.promotions.listUsage({ promotionId: promo.id });
  assert.deepEqual(promoUses.map((u) => u.orderId).sort(), [orderA.id, orderB.id].sort());

  const bobPromo = await commerce.promotions.listUsage({
    promotionId: promo.id,
    customerId: bob.id,
  });
  assert.equal(bobPromo.length, 1);
  assert.equal(bobPromo[0].orderId, orderB.id);

  assert.equal((await commerce.promotions.listUsage({ limit: 2 })).length, 2);
  assert.equal((await commerce.promotions.listUsage({ offset: 2 })).length, 1);
  assert.deepEqual(await commerce.promotions.listUsage({ orderId: UNKNOWN_ID }), []);
  await assert.rejects(
    () => commerce.promotions.listUsage({ orderId: 'not-a-uuid' }),
    (err) => err.code === 'VALIDATION',
  );
});

test('orders.getByNumber and the order-level money breakdown', async () => {
  const commerce = new Commerce(':memory:');
  const cust = await customer(commerce, 'bynumber');
  const o = await order(commerce, cust, [
    { sku: 'TAXED', name: 'Taxed', quantity: 1, unitPrice: '10.00', taxAmount: '0.80' },
  ]);

  const found = await commerce.orders.getByNumber(o.orderNumber);
  assert.equal(found.id, o.id);
  assert.deepEqual(found, await commerce.orders.get(o.id));
  assert.equal(await commerce.orders.getByNumber('ORD-DOES-NOT-EXIST'), null);

  for (const field of ['taxAmount', 'shippingAmount', 'discountAmount']) {
    assert.equal(typeof found[field], 'number', field);
    assert.equal(typeof found[`${field}Exact`], 'string', `${field}Exact`);
  }
  assert.equal(found.shippingAmountExact, '0');
  assert.equal(found.discountAmountExact, '0');
  // total = lines + tax + shipping - discount, whatever the engine attributes
  // to order-level tax.
  const lines = found.items.reduce((sum, i) => sum + Number(i.totalExact), 0);
  assert.ok(Number(found.totalAmountExact) >= lines);
});

test('accountsPayable.createBill carries lines; getBillItems reads them back', async () => {
  const commerce = new Commerce(':memory:');
  const bill = await commerce.accountsPayable.createBill({
    supplierId: UNKNOWN_ID,
    dueDate: new Date('2026-12-01T00:00:00Z').toISOString(),
    items: [
      { description: 'Widgets', quantityExact: '3', unitPriceExact: '19.99', accountCode: '5000' },
      { description: 'Freight', quantity: 1, unitPrice: 12.5 },
    ],
  });
  assert.equal(bill.totalAmountExact, '72.47');

  const items = await commerce.accountsPayable.getBillItems(bill.id);
  assert.equal(items.length, 2);
  const widgets = items.find((i) => i.description === 'Widgets');
  assert.equal(widgets.quantityExact, '3');
  assert.equal(widgets.unitPriceExact, '19.99');
  assert.equal(widgets.amountExact, '59.97');
  assert.equal(widgets.accountCode, '5000');
  assert.equal(items.find((i) => i.description === 'Freight').amountExact, '12.5');

  // Header-only bills still work.
  const empty = await commerce.accountsPayable.createBill({
    supplierId: UNKNOWN_ID,
    dueDate: new Date('2026-12-01T00:00:00Z').toISOString(),
  });
  assert.deepEqual(await commerce.accountsPayable.getBillItems(empty.id), []);
  await assert.rejects(
    () =>
      commerce.accountsPayable.createBill({
        supplierId: UNKNOWN_ID,
        dueDate: new Date('2026-12-01T00:00:00Z').toISOString(),
        items: [{ description: 'No price', quantity: 1 }],
      }),
    (err) => err.code === 'VALIDATION',
  );
});

test('quality.createInspection carries lines; getInspectionItems reads them back', async () => {
  const commerce = new Commerce(':memory:');
  const inspection = await commerce.quality.createInspection({
    inspectionType: 'receiving',
    referenceType: 'receipt',
    referenceId: UNKNOWN_ID,
    items: [
      { sku: 'INSP-1', quantityToInspectExact: '12.5', lotNumber: 'LOT-7' },
      { sku: 'INSP-2', quantityToInspect: 4 },
    ],
  });
  const items = await commerce.quality.getInspectionItems(inspection.id);
  assert.deepEqual(items.map((i) => i.sku).sort(), ['INSP-1', 'INSP-2']);
  const first = items.find((i) => i.sku === 'INSP-1');
  assert.equal(first.quantityInspectedExact, '12.5');
  assert.equal(first.lotNumber, 'LOT-7');
  assert.equal(first.result, 'pending');

  const bare = await commerce.quality.createInspection({
    inspectionType: 'receiving',
    referenceType: 'receipt',
    referenceId: UNKNOWN_ID,
  });
  assert.deepEqual(await commerce.quality.getInspectionItems(bare.id), []);
});

test('receiving.createReceipt carries lines; getReceiptItems reads them back', async () => {
  const commerce = new Commerce(':memory:');
  const wh = await commerce.warehouse.createWarehouse({ code: 'WH-RA', name: 'Read API dock' });
  const receipt = await commerce.receiving.createReceipt({
    receiptType: 'purchase_order',
    warehouseId: wh.id,
    items: [
      { sku: 'RCV-1', expectedQuantityExact: '10', unitCostExact: '3.33', description: 'Cogs' },
      { sku: 'RCV-2', expectedQuantity: 2 },
    ],
  });
  const items = await commerce.receiving.getReceiptItems(receipt.id);
  assert.equal(items.length, 2);
  const cogs = items.find((i) => i.sku === 'RCV-1');
  assert.equal(cogs.expectedQuantityExact, '10');
  assert.equal(cogs.receivedQuantityExact, '0');
  assert.equal(cogs.unitCostExact, '3.33');
  assert.equal(cogs.description, 'Cogs');
  assert.equal(items.find((i) => i.sku === 'RCV-2').unitCostExact ?? null, null);

  const bare = await commerce.receiving.createReceipt({ receiptType: 'po', warehouseId: wh.id });
  assert.deepEqual(await commerce.receiving.getReceiptItems(bare.id), []);
});
