'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { Commerce } = require('../index.js');

test('manufacturing reserves components and posts fractional finished goods once', async () => {
  const commerce = new Commerce(':memory:');
  try {
    const warehouse = await commerce.warehouse.createWarehouse({
      code: 'FACTORY',
      name: 'Factory',
    });
    const location = await commerce.warehouse.createLocation({
      warehouseId: warehouse.id,
      locationType: 'bulk',
    });
    const product = await commerce.products.create({
      name: 'Assembly',
      variants: [{ sku: 'ASSEMBLY', priceExact: '5' }],
    });
    const supplier = await commerce.purchaseOrders.createSupplier({ name: 'Components' });
    const po = await commerce.purchaseOrders.create({
      supplierId: supplier.id,
      items: [{ sku: 'RAW', name: 'Material', quantity: 10, unitCostExact: '1' }],
    });
    await commerce.purchaseOrders.submit(po.id);
    await commerce.purchaseOrders.approve(po.id, 'test');
    await commerce.purchaseOrders.send(po.id);
    const [line] = await commerce.purchaseOrders.getItems(po.id);
    await commerce.purchaseOrders.receive(
      po.id,
      [{ itemId: line.id, quantityReceivedExact: '10' }],
      warehouse.id,
      location.id,
    );
    const bom = await commerce.bom.create({ name: 'Assembly BOM', productId: product.id });
    await commerce.bom.addComponent(bom.id, { componentSku: 'RAW', name: 'Material', quantity: 2 });
    await commerce.bom.activate(bom.id);
    const wo = await commerce.workOrders.create({
      productId: product.id,
      bomId: bom.id,
      quantityToBuildExact: '1.5',
      stock: {
        sourceLocationId: location.id,
        outputLocationId: location.id,
        outputSku: 'ASSEMBLY',
      },
    });
    await assert.rejects(commerce.workOrders.completeExact(wo.id, '0.75'));
    await commerce.workOrders.start(wo.id);
    assert.match((await commerce.inventory.getStock('RAW')).totalAllocated, /^3(?:\.0+)?$/);
    const partial = await commerce.workOrders.completeExact(wo.id, '0.75');
    assert.equal(partial.quantityCompletedExact, '0.75');
    assert.match((await commerce.inventory.getStock('RAW')).totalOnHand, /^8\.50*$/);
    assert.equal((await commerce.inventory.getStock('ASSEMBLY')).totalOnHand, '0.75');
    const complete = await commerce.workOrders.completeExact(wo.id, '0.75');
    assert.equal(complete.status, 'completed');
    assert.match((await commerce.inventory.getStock('RAW')).totalOnHand, /^7(?:\.0+)?$/);
    assert.match((await commerce.inventory.getStock('RAW')).totalAllocated, /^0(?:\.0+)?$/);
    assert.equal((await commerce.inventory.getStock('ASSEMBLY')).totalOnHand, '1.50');
    await assert.rejects(commerce.workOrders.completeExact(wo.id, '0.75'));
    await assert.rejects(commerce.workOrders.start(wo.id));
  } finally {
    await commerce.close();
  }
});
