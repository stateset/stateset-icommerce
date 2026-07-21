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
  const supplierId = randomUUID();

  await t.test('API exists and is supported', async () => {
    assert.ok(commerce.vendorReturns, 'vendorReturns API should exist');
    assert.equal(await commerce.vendorReturns.isSupported(), true);
  });

  let vendorReturn;
  await t.test('create totals the item lines exactly', async () => {
    vendorReturn = await commerce.vendorReturns.create({
      supplierId,
      currency: 'USD',
      notes: 'damaged pallet',
      items: [
        { productId: randomUUID(), quantity: '3', unitCost: '12.50', reason: 'defective' },
        { productId: randomUUID(), quantity: '1', unitCost: '5.25' },
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
          items: [{ productId: randomUUID(), quantity: '1', unitCost: '1', reason: 'meh' }],
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
  });

  await t.test('cancel rejects an already-processed return', async () => {
    await assert.rejects(() => commerce.vendorReturns.cancel(vendorReturn.id));
  });
});
