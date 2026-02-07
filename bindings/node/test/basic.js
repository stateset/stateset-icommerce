/**
 * Basic test for @stateset/embedded Node.js bindings
 */

const { Commerce } = require('../index.js');
const assert = require('assert');
const { test } = require('node:test');

test('Commerce: basic operations', async (t) => {
  // Create an in-memory commerce instance
  const commerce = new Commerce(':memory:');
  let widgetSmall;
  let widgetLarge;

  await t.test('should have all APIs available', () => {
    assert.ok(commerce.customers, 'customers API should exist');
    assert.ok(commerce.orders, 'orders API should exist');
    assert.ok(commerce.products, 'products API should exist');
    assert.ok(commerce.inventory, 'inventory API should exist');
    assert.ok(commerce.returns, 'returns API should exist');
    assert.ok(commerce.carts, 'carts API should exist');
    assert.ok(commerce.payments, 'payments API should exist');
    assert.ok(commerce.shipments, 'shipments API should exist');
    assert.ok(commerce.warranties, 'warranties API should exist');
    assert.ok(commerce.purchaseOrders, 'purchaseOrders API should exist');
    assert.ok(commerce.invoices, 'invoices API should exist');
    assert.ok(commerce.bom, 'bom API should exist');
    assert.ok(commerce.workOrders, 'workOrders API should exist');
    assert.ok(commerce.analytics, 'analytics API should exist');
    assert.ok(commerce.currency, 'currency API should exist');
  });

  await t.test('should create and retrieve a customer', async () => {
    const customer = await commerce.customers.create({
      email: 'alice@example.com',
      firstName: 'Alice',
      lastName: 'Smith',
      phone: '+1-555-0123',
      acceptsMarketing: true
    });

    assert.ok(customer.id, 'customer should have an ID');
    assert.strictEqual(customer.email, 'alice@example.com');
    assert.strictEqual(customer.firstName, 'Alice');
    assert.strictEqual(customer.lastName, 'Smith');

    // Retrieve by ID
    const retrieved = await commerce.customers.get(customer.id);
    assert.strictEqual(retrieved.email, 'alice@example.com');

    // Retrieve by email
    const byEmail = await commerce.customers.getByEmail('alice@example.com');
    assert.strictEqual(byEmail.id, customer.id);

    // Count
    const count = await commerce.customers.count();
    assert.strictEqual(count, 1);
  });

  await t.test('should create and manage products', async () => {
    const product = await commerce.products.create({
      name: 'Premium Widget',
      description: 'A high-quality widget for all your widget needs',
      variants: [
        { sku: 'WIDGET-001', name: 'Small', price: 19.99 },
        { sku: 'WIDGET-002', name: 'Large', price: 29.99, compareAtPrice: 39.99 }
      ]
    });

    assert.ok(product.id, 'product should have an ID');
    assert.strictEqual(product.name, 'Premium Widget');

    // Get variant by SKU
    widgetSmall = await commerce.products.getVariantBySku('WIDGET-001');
    assert.ok(widgetSmall, 'variant should exist');
    assert.strictEqual(widgetSmall.sku, 'WIDGET-001');
    assert.ok(Math.abs(widgetSmall.price - 19.99) < 1e-6);

    widgetLarge = await commerce.products.getVariantBySku('WIDGET-002');
    assert.ok(widgetLarge, 'variant should exist');
    assert.strictEqual(widgetLarge.sku, 'WIDGET-002');
    assert.ok(Math.abs(widgetLarge.price - 29.99) < 1e-6);

    // Count products
    const count = await commerce.products.count();
    assert.strictEqual(count, 1);
  });

  await t.test('should manage inventory', async () => {
    // Create inventory item
    const item = await commerce.inventory.createItem({
      sku: 'INV-001',
      name: 'Test Item',
      description: 'A test inventory item',
      initialQuantity: 100,
      reorderPoint: 10
    });

    assert.ok(item.id, 'inventory item should have an ID');
    assert.strictEqual(item.sku, 'INV-001');

    // Get stock level
    let stock = await commerce.inventory.getStock('INV-001');
    assert.strictEqual(stock.sku, 'INV-001');
    assert.strictEqual(stock.totalOnHand, 100);
    assert.strictEqual(stock.totalAvailable, 100);

    // Adjust stock
    await commerce.inventory.adjust('INV-001', -10, 'Test adjustment');
    stock = await commerce.inventory.getStock('INV-001');
    assert.strictEqual(stock.totalOnHand, 90);

    // Reserve stock
    const reservation = await commerce.inventory.reserve(
      'INV-001',
      5,
      'order',
      'test-order-123',
      3600 // 1 hour expiry
    );
    assert.ok(reservation.id, 'reservation should have an ID');
    assert.strictEqual(reservation.quantity, 5);

    // Check available (should be reduced)
    stock = await commerce.inventory.getStock('INV-001');
    assert.strictEqual(stock.totalAvailable, 85); // 90 - 5 reserved

    // Confirm reservation
    await commerce.inventory.confirmReservation(reservation.id);
  });

  await t.test('should create and manage orders', async () => {
    // First create a customer for the order
    const customer = await commerce.customers.create({
      email: 'bob@example.com',
      firstName: 'Bob',
      lastName: 'Johnson'
    });

    if (!widgetSmall) widgetSmall = await commerce.products.getVariantBySku('WIDGET-001');
    if (!widgetLarge) widgetLarge = await commerce.products.getVariantBySku('WIDGET-002');
    assert.ok(widgetSmall, 'expected widgetSmall variant to exist');
    assert.ok(widgetLarge, 'expected widgetLarge variant to exist');

    // Create order
    const order = await commerce.orders.create({
      customerId: customer.id,
      items: [
        {
          sku: widgetSmall.sku,
          name: widgetSmall.name,
          quantity: 2,
          unitPrice: widgetSmall.price,
          productId: widgetSmall.productId,
          variantId: widgetSmall.id,
        },
        {
          sku: widgetLarge.sku,
          name: widgetLarge.name,
          quantity: 1,
          unitPrice: widgetLarge.price,
          productId: widgetLarge.productId,
          variantId: widgetLarge.id,
        },
      ],
      currency: 'USD',
      notes: 'Test order'
    });

    assert.ok(order.id, 'order should have an ID');
    assert.ok(order.orderNumber, 'order should have an order number');
    assert.strictEqual(order.customerId, customer.id);
    assert.strictEqual(order.status, 'pending');
    assert.strictEqual(order.items.length, 2);
    assert.ok(Math.abs(order.totalAmount - 69.97) < 1e-6); // 2*19.99 + 29.99

    // Update status
    let updated = await commerce.orders.updateStatus(order.id, 'confirmed');
    assert.strictEqual(updated.status, 'confirmed');

    // Ship the order
    updated = await commerce.orders.ship(order.id, 'TRACK123456');
    assert.strictEqual(updated.status, 'shipped');
    assert.strictEqual(updated.trackingNumber, 'TRACK123456');

    // Count orders
    const count = await commerce.orders.count();
    assert.strictEqual(count, 1);
  });

  await t.test('should handle returns', async () => {
    // Create customer and order first
    const customer = await commerce.customers.create({
      email: 'carol@example.com',
      firstName: 'Carol',
      lastName: 'Williams'
    });

    if (!widgetSmall) widgetSmall = await commerce.products.getVariantBySku('WIDGET-001');
    assert.ok(widgetSmall, 'expected widgetSmall variant to exist');

    const order = await commerce.orders.create({
      customerId: customer.id,
      items: [
        {
          sku: widgetSmall.sku,
          name: widgetSmall.name,
          quantity: 1,
          unitPrice: widgetSmall.price,
          productId: widgetSmall.productId,
          variantId: widgetSmall.id,
        },
      ]
    });

    // Get the order to access item IDs (we'll use a mock ID for now)
    const orderDetails = await commerce.orders.get(order.id);
    const orderItemId = orderDetails.items[0].id;

    // Create return
    const ret = await commerce.returns.create({
      orderId: order.id,
      reason: 'defective',
      reasonDetails: 'Item arrived broken',
      items: [
        { orderItemId: orderItemId, quantity: 1 }
      ]
    });

    assert.ok(ret.id, 'return should have an ID');
    assert.strictEqual(ret.orderId, order.id);
    assert.strictEqual(ret.status, 'requested');
    assert.strictEqual(ret.reason, 'defective');

    // Approve return
    const approved = await commerce.returns.approve(ret.id);
    assert.strictEqual(approved.status, 'approved');

    // Count returns
    const count = await commerce.returns.count();
    assert.strictEqual(count, 1);
  });

  await t.test('should run analytics and currency operations', async () => {
    const summary = await commerce.analytics.salesSummary({ period: 'last30days' });
    assert.ok(summary, 'sales summary should be returned');
    assert.ok(typeof summary.orderCount === 'number' && summary.orderCount >= 2);

    // Seed a rate and convert
    await commerce.currency.setRate({
      baseCurrency: 'USD',
      quoteCurrency: 'EUR',
      rate: 0.9,
      source: 'test'
    });

    const conversion = await commerce.currency.convert({ from: 'USD', to: 'EUR', amount: 100 });
    assert.ok(Math.abs(conversion.convertedAmount - 90) < 1e-6);
  });

  // Keep tests quiet; CI logs should be signal-heavy.
});
