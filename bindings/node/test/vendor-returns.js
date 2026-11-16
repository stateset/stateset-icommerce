/**
 * Vendor return API tests for @stateset/embedded Node.js bindings.
 *
 * Create, get, list, submit, process, cancel.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { randomUUID } = require('node:crypto');

test('VendorReturns: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');
  const supplierId = (await commerce.purchaseOrders.createSupplier({ name: 'Vendor returns' })).id;
  const warehouse = await commerce.warehouse.createWarehouse({ code: 'RETURNS', name: 'Returns' });
  const location = await commerce.warehouse.createLocation({ warehouseId: warehouse.id, locationType: 'bulk' });
  const products = [];
  for (const sku of ['RETURN-A', 'RETURN-B']) {
    products.push(await commerce.products.create({ name: sku, variants: [{ sku, priceExact: '10' }] }));
  }
  const po = await commerce.purchaseOrders.create({ supplierId, items: [
    { sku: 'RETURN-A', name: 'A', quantity: 5, unitCostExact: '12.50' },
    { sku: 'RETURN-B', name: 'B', quantity: 5, unitCostExact: '5.25' },
  ] });
  await commerce.purchaseOrders.submit(po.id);
  await commerce.purchaseOrders.approve(po.id, 'test');
  await commerce.purchaseOrders.send(po.id);
  const received = (await commerce.purchaseOrders.getItems(po.id)).map(item => ({ itemId: item.id, quantityReceivedExact: '5' }));
  await commerce.purchaseOrders.receive(po.id, received, warehouse.id, location.id);
  const source = index => ({ productId: products[index].id, sku: index === 0 ? 'RETURN-A' : 'RETURN-B', sourceLocationId: location.id });

  await t.test('API exists and is supported', async () => {
    assert.ok(commerce.vendorReturns, 'vendorReturns API should exist');
    assert.equal(await commerce.vendorReturns.isSupported(), true);
  });

  let vendorReturn;
  await t.test('create totals the item lines exactly', async () => {
    vendorReturn = await commerce.vendorReturns.create({
      supplierId,
      warehouseId: warehouse.id,
      currency: 'USD',
      notes: 'damaged pallet',
      items: [
        { ...source(0), quantity: '3', unitCost: '12.50', reason: 'defective' },
        { ...source(1), quantity: '1', unitCost: '5.25' },
      ],
    });
    assert.ok(vendorReturn.id);
    assert.ok(vendorReturn.number.startsWith('VR-'));
    assert.equal(vendorReturn.status, 'draft');
    assert.equal(vendorReturn.currency, 'USD');
    assert.equal(vendorReturn.items.length, 2);
    assert.equal(vendorReturn.totalCredit, '42.75');
    assert.equal(vendorReturn.creditGenerated, false);
    assert.equal(vendorReturn.processedAt, undefined);
  });

  await t.test('create rejects an unknown reason', async () => {
    await assert.rejects(
      () =>
        commerce.vendorReturns.create({
          supplierId,
      warehouseId: warehouse.id,
          items: [{ ...source(0), quantity: '1', unitCost: '1', reason: 'meh' }],
        }),
      /Invalid vendor return reason: meh/,
    );
  });

  await t.test('get returns the vendor return, and null when missing', async () => {
    const found = await commerce.vendorReturns.get(vendorReturn.id);
    assert.equal(found.id, vendorReturn.id);
    assert.equal(await commerce.vendorReturns.get(randomUUID()), null);
  });

  await t.test('submit moves a draft to pending', async () => {
    const submitted = await commerce.vendorReturns.submit(vendorReturn.id);
    assert.equal(submitted.status, 'pending');
  });

  await t.test('list filters by supplier and status', async () => {
    assert.ok((await commerce.vendorReturns.list()).length >= 1);
    assert.equal((await commerce.vendorReturns.list({ supplierId })).length, 1);
    assert.equal((await commerce.vendorReturns.list({ status: 'pending' })).length, 1);
    await assert.rejects(
      () => commerce.vendorReturns.list({ status: 'bogus' }),
      /Invalid vendor return status: bogus/,
    );
  });

  await t.test('process marks it processed with a credit', async () => {
    const processed = await commerce.vendorReturns.process(vendorReturn.id, true);
    assert.equal(processed.status, 'processed');
    assert.equal(processed.creditGenerated, true);
    assert.ok(processed.processedAt);
    const credits = await commerce.vendorCredits.list({ supplierId });
    assert.equal(credits.length, 1);
    assert.equal(credits[0].vendorReturnId, vendorReturn.id);
    assert.equal(credits[0].amount, '42.75');
    assert.equal(Number((await commerce.inventory.getStock('RETURN-A')).totalOnHand), 2);
    assert.equal(Number((await commerce.inventory.getStock('RETURN-B')).totalOnHand), 4);
  });

  await t.test('cancel rejects an already-processed return', async () => {
    await assert.rejects(() => commerce.vendorReturns.cancel(vendorReturn.id));
  });
});
