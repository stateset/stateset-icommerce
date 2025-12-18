#!/usr/bin/env node
// Test script for WASM bindings

const { Commerce } = require('./pkg-node/stateset_embedded_wasm.js');

console.log('=== StateSet WASM Bindings Test ===\n');

try {
  // Create commerce instance
  const commerce = new Commerce();
  console.log('✓ Created Commerce instance');

  // Test Customers
  console.log('\n--- Customers ---');
  const customer = commerce.customers.create({
    email: 'alice@example.com',
    firstName: 'Alice',
    lastName: 'Smith',
    phone: '+1234567890',
  });
  console.log(`✓ Created customer: ${customer.id}`);
  console.log(`  Name: ${customer.fullName}`);
  console.log(`  Email: ${customer.email}`);
  console.log(`  Status: ${customer.status}`);

  const foundCustomer = commerce.customers.get(customer.id);
  console.log(`✓ Retrieved customer by ID: ${foundCustomer.email}`);

  const foundByEmail = commerce.customers.getByEmail('alice@example.com');
  console.log(`✓ Retrieved customer by email: ${foundByEmail.fullName}`);

  const customerCount = commerce.customers.count();
  console.log(`✓ Customer count: ${customerCount}`);

  // Test Products
  console.log('\n--- Products ---');
  const product = commerce.products.create({
    name: 'Premium Widget',
    description: 'A high-quality widget',
    variants: [
      { sku: 'WIDGET-SM', price: 19.99, name: 'Small' },
      { sku: 'WIDGET-LG', price: 29.99, name: 'Large' },
    ],
  });
  console.log(`✓ Created product: ${product.id}`);
  console.log(`  Name: ${product.name}`);
  console.log(`  Slug: ${product.slug}`);
  console.log(`  Status: ${product.status}`);

  const variant = commerce.products.getVariantBySku('WIDGET-LG');
  console.log(`✓ Found variant: ${variant.sku} - $${variant.price}`);

  // Test Inventory
  console.log('\n--- Inventory ---');
  const invItem = commerce.inventory.createItem({
    sku: 'WIDGET-001',
    name: 'Premium Widget',
    initialQuantity: 100,
  });
  console.log(`✓ Created inventory item: ${invItem.sku}`);

  const stock = commerce.inventory.getStock('WIDGET-001');
  console.log(`✓ Stock level for ${stock.sku}:`);
  console.log(`  On Hand: ${stock.totalOnHand}`);
  console.log(`  Allocated: ${stock.totalAllocated}`);
  console.log(`  Available: ${stock.totalAvailable}`);

  commerce.inventory.adjust('WIDGET-001', -10, 'Sold 10 units');
  const stockAfter = commerce.inventory.getStock('WIDGET-001');
  console.log(`✓ Adjusted stock: ${stockAfter.totalOnHand} on hand`);

  const reservation = commerce.inventory.reserve(
    'WIDGET-001',
    5,
    'order',
    'ord-123'
  );
  console.log(`✓ Created reservation: ${reservation.id}`);

  const stockWithRes = commerce.inventory.getStock('WIDGET-001');
  console.log(`  Available after reservation: ${stockWithRes.totalAvailable}`);

  commerce.inventory.releaseReservation(reservation.id);
  const stockReleased = commerce.inventory.getStock('WIDGET-001');
  console.log(`✓ Released reservation. Available: ${stockReleased.totalAvailable}`);

  // Test Orders
  console.log('\n--- Orders ---');
  const order = commerce.orders.create({
    customerId: customer.id,
    items: [
      { sku: 'WIDGET-001', name: 'Premium Widget', quantity: 2, unitPrice: 29.99 },
      { sku: 'WIDGET-002', name: 'Basic Widget', quantity: 1, unitPrice: 19.99 },
    ],
  });
  console.log(`✓ Created order: ${order.orderNumber}`);
  console.log(`  Customer: ${order.customerId}`);
  console.log(`  Status: ${order.status}`);
  console.log(`  Total: $${order.totalAmount}`);
  console.log(`  Items: ${order.items.length}`);

  const updatedOrder = commerce.orders.updateStatus(order.id, 'processing');
  console.log(`✓ Updated status: ${updatedOrder.status}`);

  const shippedOrder = commerce.orders.ship(order.id, '1Z123ABC');
  console.log(`✓ Shipped order: ${shippedOrder.trackingNumber}`);
  console.log(`  Status: ${shippedOrder.status}`);
  console.log(`  Fulfillment: ${shippedOrder.fulfillmentStatus}`);

  const orderCount = commerce.orders.count();
  console.log(`✓ Order count: ${orderCount}`);

  // Test Returns
  console.log('\n--- Returns ---');
  const ret = commerce.returns.create({
    orderId: order.id,
    reason: 'defective',
    items: [{ orderItemId: order.items[0].id, quantity: 1 }],
    reasonDetails: 'Product stopped working',
  });
  console.log(`✓ Created return: ${ret.id}`);
  console.log(`  Status: ${ret.status}`);
  console.log(`  Reason: ${ret.reason}`);

  const approvedReturn = commerce.returns.approve(ret.id);
  console.log(`✓ Approved return: ${approvedReturn.status}`);

  const returnCount = commerce.returns.count();
  console.log(`✓ Return count: ${returnCount}`);

  // Summary
  console.log('\n=== Test Summary ===');
  console.log(`Customers: ${commerce.customers.count()}`);
  console.log(`Products: ${commerce.products.count()}`);
  console.log(`Orders: ${commerce.orders.count()}`);
  console.log(`Returns: ${commerce.returns.count()}`);
  console.log('\n✅ All tests passed!');

  process.exit(0);
} catch (error) {
  console.error('\n❌ Test failed:', error);
  process.exit(1);
}
