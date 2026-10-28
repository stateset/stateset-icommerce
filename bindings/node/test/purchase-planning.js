'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { Commerce } = require('../index.js');

test('exact purchasing costs survive PO approval and bounded partial receipts', async () => {
  const commerce = new Commerce(':memory:');
  try {
    const warehouse = await commerce.warehouse.createWarehouse({
      code: 'SIM',
      name: 'Simulation warehouse',
    });
    const location = await commerce.warehouse.createLocation({
      warehouseId: warehouse.id,
      code: 'RECEIVING',
      locationType: 'receiving',
      isReceivable: true,
    });
    await commerce.currency.setRate({ baseCurrency: 'EUR', quoteCurrency: 'USD', rate: 1.1 });
    const supplier = await commerce.purchaseOrders.createSupplier({ name: 'Planning supplier' });
    let po = await commerce.purchaseOrders.create({
      supplierId: supplier.id,
      currency: 'EUR',
      items: [
        { sku: 'EXACT', name: 'Exact item', quantity: 3, unitCostExact: '0.100001', unitCost: 999 },
      ],
    });
    assert.equal(po.totalExact, '0.300003');
    assert.equal(po.currency, 'EUR');
    const [line] = await commerce.purchaseOrders.getItems(po.id);
    assert.equal(line.unitCostExact, '0.100001');
    assert.equal(line.lineTotalExact, '0.300003');
    assert.equal(line.quantityOrderedExact, '3');
    await assert.rejects(
      commerce.purchaseOrders.receive(
        po.id,
        [{ itemId: line.id, quantityReceivedExact: '1' }],
        warehouse.id,
        location.id,
      ),
    );
    await commerce.purchaseOrders.submit(po.id);
    await commerce.purchaseOrders.approve(po.id, 'test-planner');
    await commerce.purchaseOrders.send(po.id);
    po = await commerce.purchaseOrders.receive(
      po.id,
      [{ itemId: line.id, quantityReceivedExact: '1' }],
      warehouse.id,
      location.id,
    );
    assert.equal(po.status, 'partially_received');
    await assert.rejects(
      commerce.purchaseOrders.receive(
        po.id,
        [{ itemId: line.id, quantityReceivedExact: '3' }],
        warehouse.id,
        location.id,
      ),
    );
    assert.equal((await commerce.purchaseOrders.getItems(po.id))[0].quantityReceivedExact, '1');
    po = await commerce.purchaseOrders.receive(
      po.id,
      [{ itemId: line.id, quantityReceivedExact: '2' }],
      warehouse.id,
      location.id,
    );
    assert.equal(po.status, 'received');
    assert.equal((await commerce.purchaseOrders.getItems(po.id))[0].quantityReceivedExact, '3');
  } finally {
    commerce.close();
  }
});

test('purchasing rejects malformed exact money and unknown currency before writes', async () => {
  const commerce = new Commerce(':memory:');
  try {
    const supplier = await commerce.purchaseOrders.createSupplier({ name: 'Planning supplier' });
    for (const unitCostExact of ['no', 'NaN', 'Infinity', '-1']) {
      await assert.rejects(
        commerce.purchaseOrders.create({
          supplierId: supplier.id,
          items: [{ sku: 'BAD', name: 'Bad', quantity: 1, unitCostExact }],
        }),
      );
    }
    await assert.rejects(
      commerce.purchaseOrders.create({
        supplierId: supplier.id,
        currency: 'USDC',
        items: [{ sku: 'BAD', name: 'Bad', quantity: 1, unitCostExact: '1' }],
      }),
    );
    assert.equal(await commerce.purchaseOrders.count(), 0);
    const legacy = await commerce.purchaseOrders.create({
      supplierId: supplier.id,
      items: [{ sku: 'LEGACY', name: 'Legacy', quantity: 1, unitCost: 2.5 }],
    });
    assert.equal(legacy.totalExact, '2.5');
  } finally {
    commerce.close();
  }
});

test('physical shipment stock posts atomically and is safe to replay', async () => {
  const commerce = new Commerce(':memory:');
  try {
    const customer = await commerce.customers.create({
      email: 'stock@example.test',
      firstName: 'Stock',
      lastName: 'Posting',
    });
    await commerce.inventory.createItem({ sku: 'SHIP', name: 'Shipment', initialQuantity: 5 });
    const order = await commerce.orders.createExact({
      customerId: customer.id,
      currency: 'USD',
      stockPolicy: 'reject_if_insufficient',
      items: [{ sku: 'SHIP', name: 'Shipment', quantity: 2, unitPrice: '1.25' }],
    });
    await assert.rejects(commerce.inventory.fulfillOrderStock(order.id));
    await commerce.orders.ship(order.id);
    assert.equal((await commerce.inventory.getStock('SHIP')).totalAllocated, '2');
    await commerce.inventory.fulfillOrderStock(order.id);
    await commerce.inventory.fulfillOrderStock(order.id);
    const stock = await commerce.inventory.getStock('SHIP');
    assert.equal(stock.totalOnHand, '3');
    assert.equal(stock.totalAvailable, '3');
    assert.equal(stock.totalAllocated, '0');
  } finally {
    commerce.close();
  }
});
