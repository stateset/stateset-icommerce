/**
 * Lifecycle tests for the procurement / pricing / logistics domains of the
 * @stateset/embedded Node.js bindings: prepayments, vendor credits, price
 * schedules, price levels, transfer orders, production batches, supplier
 * SKUs, and inbound shipments.
 *
 * Money and quantities cross as exact decimal strings; timestamps as RFC 3339
 * strings; enums as snake_case strings.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { randomUUID } = require('node:crypto');
const { test } = require('node:test');

test('Prepayments: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');
  const supplierId = randomUUID();

  assert.ok(commerce.prepayments, 'prepayments API should exist');
  assert.equal(await commerce.prepayments.isSupported(), true);

  const prepayment = await commerce.prepayments.create({
    supplierId,
    amount: '1000.00',
    method: 'wire',
    reference: 'WIRE-42',
    memo: 'advance for Q3',
  });
  assert.ok(prepayment.id);
  assert.equal(prepayment.supplierId, supplierId);
  assert.equal(prepayment.amount, '1000.00');
  assert.equal(prepayment.remaining, '1000.00');
  assert.equal(prepayment.status, 'open');
  assert.equal(prepayment.method, 'wire');

  const found = await commerce.prepayments.get(prepayment.id);
  assert.equal(found.id, prepayment.id);
  const listed = await commerce.prepayments.list({ supplierId, status: 'open' });
  assert.ok(listed.some((p) => p.id === prepayment.id));

  const targetId = randomUUID();
  const applied = await commerce.prepayments.apply(prepayment.id, {
    targetType: 'bill',
    targetId,
    amount: '400.00',
  });
  assert.equal(applied.remaining, '600.00');

  const applications = await commerce.prepayments.listApplications(prepayment.id);
  assert.equal(applications.length, 1);
  assert.equal(applications[0].targetType, 'bill');
  assert.equal(applications[0].targetId, targetId);
  assert.equal(applications[0].amount, '400.00');
  assert.equal(applications[0].reversed, false);

  const reversed = await commerce.prepayments.reverseApplication(
    prepayment.id,
    applications[0].id,
  );
  assert.equal(reversed.remaining, '1000.00');

  const refunded = await commerce.prepayments.refund(prepayment.id);
  assert.equal(refunded.status, 'refunded');
  assert.equal(refunded.remaining, '0');
});

test('VendorCredits: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');
  const supplierId = randomUUID();

  assert.ok(commerce.vendorCredits, 'vendorCredits API should exist');
  assert.equal(await commerce.vendorCredits.isSupported(), true);

  const credit = await commerce.vendorCredits.create({
    supplierId,
    amount: '250.00',
    memo: 'pricing adjustment',
  });
  assert.ok(credit.id);
  assert.equal(credit.amount, '250.00');
  assert.equal(credit.remaining, '250.00');
  assert.equal(credit.status, 'open');

  const found = await commerce.vendorCredits.get(credit.id);
  assert.equal(found.id, credit.id);
  const listed = await commerce.vendorCredits.list({ supplierId });
  assert.ok(listed.some((c) => c.id === credit.id));

  const applied = await commerce.vendorCredits.apply(credit.id, {
    targetType: 'bill',
    targetId: randomUUID(),
    amount: '100.00',
  });
  assert.equal(applied.remaining, '150.00');

  const applications = await commerce.vendorCredits.listApplications(credit.id);
  assert.equal(applications.length, 1);
  assert.equal(applications[0].amount, '100.00');

  const reversed = await commerce.vendorCredits.reverseApplication(
    credit.id,
    applications[0].id,
  );
  assert.equal(reversed.remaining, '250.00');

  const cancelled = await commerce.vendorCredits.cancel(credit.id);
  assert.equal(cancelled.status, 'cancelled');
});

test('PriceSchedules: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');
  const productId = randomUUID();

  assert.ok(commerce.priceSchedules, 'priceSchedules API should exist');
  assert.equal(await commerce.priceSchedules.isSupported(), true);

  const schedule = await commerce.priceSchedules.create({
    name: 'Black Friday',
    code: 'BF-2026',
    startsAt: '2026-11-27T00:00:00Z',
    endsAt: '2026-11-30T23:59:59Z',
    priority: 10,
  });
  assert.ok(schedule.id);
  assert.equal(schedule.name, 'Black Friday');
  assert.equal(schedule.isActive, true);
  assert.equal(schedule.priority, 10);

  const found = await commerce.priceSchedules.get(schedule.id);
  assert.equal(found.id, schedule.id);
  const listed = await commerce.priceSchedules.list({ isActive: true });
  assert.ok(listed.some((s) => s.id === schedule.id));

  const updated = await commerce.priceSchedules.update(schedule.id, {
    name: 'Black Friday Sale',
  });
  assert.equal(updated.name, 'Black Friday Sale');

  const entry = await commerce.priceSchedules.setEntry(schedule.id, productId, '19.99');
  assert.equal(entry.price, '19.99');
  assert.equal(entry.productId, productId);

  const entries = await commerce.priceSchedules.listEntries(schedule.id);
  assert.equal(entries.length, 1);

  const inWindow = await commerce.priceSchedules.resolvePrice(
    productId,
    '2026-11-28T12:00:00Z',
  );
  assert.equal(inWindow, '19.99');
  const outOfWindow = await commerce.priceSchedules.resolvePrice(
    productId,
    '2026-12-25T12:00:00Z',
  );
  assert.equal(outOfWindow, null);

  await commerce.priceSchedules.deleteEntry(schedule.id, productId);
  assert.equal((await commerce.priceSchedules.listEntries(schedule.id)).length, 0);

  await commerce.priceSchedules.delete(schedule.id);
  assert.equal(await commerce.priceSchedules.get(schedule.id), null);
});

test('PriceLevels: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');
  const productId = randomUUID();

  assert.ok(commerce.priceLevels, 'priceLevels API should exist');
  assert.equal(await commerce.priceLevels.isSupported(), true);

  const level = await commerce.priceLevels.create({
    name: 'Wholesale',
    code: 'WHOLESALE',
    adjustmentType: 'percentage_discount',
    adjustmentValue: '10',
  });
  assert.ok(level.id);
  assert.equal(level.code, 'WHOLESALE');
  assert.equal(level.adjustmentType, 'percentage_discount');
  assert.equal(level.adjustmentValue, '10');
  assert.equal(level.isActive, true);

  const found = await commerce.priceLevels.get(level.id);
  assert.equal(found.id, level.id);
  const listed = await commerce.priceLevels.list({ isActive: true });
  assert.ok(listed.some((l) => l.id === level.id));

  const updated = await commerce.priceLevels.update(level.id, {
    adjustmentValue: '15',
  });
  assert.equal(updated.adjustmentValue, '15');

  const entry = await commerce.priceLevels.setEntry(level.id, productId, '42.00');
  assert.equal(entry.price, '42.00');

  const entries = await commerce.priceLevels.listEntries(level.id);
  assert.equal(entries.length, 1);
  assert.equal(entries[0].productId, productId);

  await commerce.priceLevels.deleteEntry(level.id, productId);
  assert.equal((await commerce.priceLevels.listEntries(level.id)).length, 0);

  await commerce.priceLevels.delete(level.id);
  assert.equal(await commerce.priceLevels.get(level.id), null);
});

test('TransferOrders: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');
  const sourceWarehouseId = randomUUID();
  const destinationWarehouseId = randomUUID();
  const productId = randomUUID();

  assert.ok(commerce.transferOrders, 'transferOrders API should exist');
  assert.equal(await commerce.transferOrders.isSupported(), true);

  const order = await commerce.transferOrders.create({
    sourceWarehouseId,
    destinationWarehouseId,
    items: [{ productId, quantity: '10' }],
    notes: 'restock east coast',
  });
  assert.ok(order.id);
  assert.ok(order.number.startsWith('TO-'));
  assert.equal(order.status, 'draft');
  assert.equal(order.items.length, 1);
  assert.equal(order.items[0].quantity, '10');

  const found = await commerce.transferOrders.get(order.id);
  assert.equal(found.id, order.id);
  const listed = await commerce.transferOrders.list({ sourceWarehouseId });
  assert.ok(listed.some((o) => o.id === order.id));

  const shipped = await commerce.transferOrders.ship(order.id);
  assert.equal(shipped.status, 'in_transit');
  assert.equal(shipped.items[0].quantityShipped, '10');
  assert.ok(shipped.shippedAt);

  const partial = await commerce.transferOrders.receiveLine(
    order.id,
    shipped.items[0].id,
    '4',
  );
  assert.equal(partial.status, 'partially_received');
  assert.equal(partial.items[0].quantityReceived, '4');

  const full = await commerce.transferOrders.receiveLine(order.id, shipped.items[0].id, '6');
  assert.equal(full.status, 'received');
  assert.ok(full.receivedAt);

  const other = await commerce.transferOrders.create({
    sourceWarehouseId,
    destinationWarehouseId,
    items: [{ productId, quantity: '5' }],
  });
  const cancelled = await commerce.transferOrders.cancel(other.id);
  assert.equal(cancelled.status, 'cancelled');
});

test('ProductionBatches: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');
  const workOrderA = randomUUID();
  const workOrderB = randomUUID();

  assert.ok(commerce.productionBatches, 'productionBatches API should exist');
  assert.equal(await commerce.productionBatches.isSupported(), true);

  const batch = await commerce.productionBatches.create({
    name: 'July widgets',
    workOrderIds: [workOrderA],
    notes: 'first run',
  });
  assert.ok(batch.id);
  assert.equal(batch.status, 'planned');
  assert.deepEqual(batch.workOrderIds, [workOrderA]);

  const found = await commerce.productionBatches.get(batch.id);
  assert.equal(found.id, batch.id);
  const listed = await commerce.productionBatches.list({ status: 'planned' });
  assert.ok(listed.some((b) => b.id === batch.id));

  const updated = await commerce.productionBatches.update(batch.id, {
    name: 'July widgets v2',
    status: 'in_progress',
  });
  assert.equal(updated.name, 'July widgets v2');
  assert.equal(updated.status, 'in_progress');

  const added = await commerce.productionBatches.addWorkOrders(batch.id, [workOrderB]);
  assert.equal(added.workOrderIds.length, 2);

  const removed = await commerce.productionBatches.removeWorkOrder(batch.id, workOrderA);
  assert.deepEqual(removed.workOrderIds, [workOrderB]);

  await commerce.productionBatches.delete(batch.id);
  assert.equal(await commerce.productionBatches.get(batch.id), null);
});

test('SupplierSkus: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');
  const supplierId = randomUUID();
  const productId = randomUUID();

  assert.ok(commerce.supplierSkus, 'supplierSkus API should exist');
  assert.equal(await commerce.supplierSkus.isSupported(), true);

  const record = await commerce.supplierSkus.create({
    productId,
    supplierId,
    sku: 'ACME-001',
    unitCost: '12.50',
    minOrderQty: '100',
    leadTimeDays: 14,
  });
  assert.ok(record.id);
  assert.equal(record.sku, 'ACME-001');
  assert.equal(record.unitCost, '12.50');
  assert.equal(record.minOrderQty, '100');
  assert.equal(record.leadTimeDays, 14);
  assert.equal(record.isPreferred, false);

  const found = await commerce.supplierSkus.get(record.id);
  assert.equal(found.id, record.id);
  const listed = await commerce.supplierSkus.list({ supplierId });
  assert.ok(listed.some((r) => r.id === record.id));

  const updated = await commerce.supplierSkus.update(record.id, {
    unitCost: '11.75',
    isPreferred: true,
  });
  assert.equal(updated.unitCost, '11.75');
  assert.equal(updated.isPreferred, true);

  const count = await commerce.supplierSkus.bulkUpsert(supplierId, [
    { productId, sku: 'ACME-001-B', unitCost: '11.00' },
    { productId: randomUUID(), sku: 'ACME-002', unitCost: '3.25' },
  ]);
  assert.equal(count, 2);

  await commerce.supplierSkus.delete(record.id);
  assert.equal(await commerce.supplierSkus.get(record.id), null);
});

test('InboundShipments: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');
  const supplierId = randomUUID();
  const warehouseId = randomUUID();
  const productId = randomUUID();

  assert.ok(commerce.inboundShipments, 'inboundShipments API should exist');
  assert.equal(await commerce.inboundShipments.isSupported(), true);

  const shipment = await commerce.inboundShipments.create({
    supplierId,
    warehouseId,
    carrier: 'DHL',
    trackingNumber: '1Z999',
    items: [{ productId, sku: 'SKU-1', quantityExpected: '10' }],
  });
  assert.ok(shipment.id);
  assert.equal(shipment.status, 'pending');
  assert.equal(shipment.carrier, 'DHL');
  assert.equal(shipment.items.length, 1);
  assert.equal(shipment.items[0].quantityExpected, '10');

  const found = await commerce.inboundShipments.get(shipment.id);
  assert.equal(found.id, shipment.id);
  const listed = await commerce.inboundShipments.list({ supplierId, status: 'pending' });
  assert.ok(listed.some((s) => s.id === shipment.id));

  const inTransit = await commerce.inboundShipments.markInTransit(shipment.id);
  assert.equal(inTransit.status, 'in_transit');

  const arrived = await commerce.inboundShipments.markArrived(shipment.id);
  assert.equal(arrived.status, 'arrived');

  const partial = await commerce.inboundShipments.receiveLine(
    shipment.id,
    shipment.items[0].id,
    '4',
  );
  assert.equal(partial.status, 'partially_received');
  assert.equal(partial.items[0].quantityReceived, '4');

  const full = await commerce.inboundShipments.receiveLine(
    shipment.id,
    shipment.items[0].id,
    '6',
  );
  assert.equal(full.status, 'received');
  assert.ok(full.receivedAt);

  const other = await commerce.inboundShipments.create({
    supplierId,
    items: [{ productId, sku: 'SKU-2', quantityExpected: '5' }],
  });
  const cancelled = await commerce.inboundShipments.cancel(other.id);
  assert.equal(cancelled.status, 'cancelled');
});
