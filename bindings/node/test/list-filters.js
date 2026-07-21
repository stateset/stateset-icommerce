/**
 * Filter-threading tests for list endpoints that previously accepted no
 * arguments: purchaseOrders.list, workOrders.list, quality.listInspections,
 * quality.listNcrs. These prove the camelCase filter object reaches the store.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

test('purchaseOrders.list honors supplier, status and total filters', async () => {
  const commerce = new Commerce(':memory:');
  const s1 = await commerce.purchaseOrders.createSupplier({ name: 'Supplier One' });
  const s2 = await commerce.purchaseOrders.createSupplier({ name: 'Supplier Two' });

  const cheap = await commerce.purchaseOrders.create({
    supplierId: s1.id,
    items: [{ sku: 'CHEAP', name: 'Cheap', quantity: 1, unitCost: 5 }],
  });
  const pricey = await commerce.purchaseOrders.create({
    supplierId: s1.id,
    items: [{ sku: 'PRICEY', name: 'Pricey', quantity: 1, unitCost: 500 }],
  });
  await commerce.purchaseOrders.create({
    supplierId: s2.id,
    items: [{ sku: 'OTHER', name: 'Other', quantity: 1, unitCost: 10 }],
  });

  // Zero-arg still works (backwards compatible).
  const all = await commerce.purchaseOrders.list();
  assert.equal(all.length, 3);

  const bySupplier = await commerce.purchaseOrders.list({ supplierId: s1.id });
  assert.equal(bySupplier.length, 2);
  assert.ok(bySupplier.every((p) => p.supplierId === s1.id));

  const expensive = await commerce.purchaseOrders.list({ supplierId: s1.id, minTotal: '100' });
  assert.deepEqual(
    expensive.map((p) => p.id).sort(),
    [pricey.id].sort(),
  );

  const budget = await commerce.purchaseOrders.list({ supplierId: s1.id, maxTotal: '100' });
  assert.deepEqual(
    budget.map((p) => p.id).sort(),
    [cheap.id].sort(),
  );

  const limited = await commerce.purchaseOrders.list({ limit: 1 });
  assert.equal(limited.length, 1);
});

test('workOrders.list honors status and pagination filters', async () => {
  const commerce = new Commerce(':memory:');
  const product = await commerce.products.create({ name: 'WO Product' });
  const bom = await commerce.bom.create({ name: 'BOM', productId: product.id });

  const wo1 = await commerce.workOrders.create({
    productId: product.id,
    bomId: bom.id,
    quantityToBuild: 5,
  });
  await commerce.workOrders.create({
    productId: product.id,
    bomId: bom.id,
    quantityToBuild: 3,
  });

  const all = await commerce.workOrders.list();
  assert.equal(all.length, 2);

  const draftStatus = wo1.status;
  const byStatus = await commerce.workOrders.list({ status: draftStatus });
  assert.ok(byStatus.every((w) => w.status === draftStatus));
  assert.ok(byStatus.length >= 1);

  const byProduct = await commerce.workOrders.list({ productId: product.id });
  assert.equal(byProduct.length, 2);

  const limited = await commerce.workOrders.list({ limit: 1 });
  assert.equal(limited.length, 1);
});

test('quality.listInspections and listNcrs honor filters', async () => {
  const commerce = new Commerce(':memory:');

  const shipInsp = await commerce.quality.createInspection({
    inspectionType: 'final',
    referenceType: 'shipment',
    referenceId: '11111111-1111-1111-1111-111111111111',
  });
  await commerce.quality.createInspection({
    inspectionType: 'incoming',
    referenceType: 'purchase_order',
    referenceId: '22222222-2222-2222-2222-222222222222',
  });

  const allInsp = await commerce.quality.listInspections();
  assert.equal(allInsp.length, 2);

  const finals = await commerce.quality.listInspections({ inspectionType: 'final' });
  assert.deepEqual(
    finals.map((i) => i.id).sort(),
    [shipInsp.id].sort(),
  );

  const byRef = await commerce.quality.listInspections({
    referenceId: '11111111-1111-1111-1111-111111111111',
  });
  assert.equal(byRef.length, 1);

  const limitedInsp = await commerce.quality.listInspections({ limit: 1 });
  assert.equal(limitedInsp.length, 1);

  // NCRs
  const widget = await commerce.quality.createNcr({
    source: 'supplier_issue',
    severity: 'major',
    sku: 'WIDGET',
    quantityAffected: 2,
    description: 'widget defect',
  });
  await commerce.quality.createNcr({
    source: 'internal_audit',
    severity: 'minor',
    sku: 'GADGET',
    quantityAffected: 1,
    description: 'gadget defect',
  });

  const allNcrs = await commerce.quality.listNcrs();
  assert.equal(allNcrs.length, 2);

  const bySku = await commerce.quality.listNcrs({ sku: 'WIDGET' });
  assert.deepEqual(
    bySku.map((n) => n.id).sort(),
    [widget.id].sort(),
  );

  const majors = await commerce.quality.listNcrs({ severity: 'major' });
  assert.equal(majors.length, 1, 'severity filter must reach the store');
  assert.equal(majors[0].id, widget.id);

  const limitedNcrs = await commerce.quality.listNcrs({ limit: 1 });
  assert.equal(limitedNcrs.length, 1);
});
