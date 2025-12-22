#!/usr/bin/env node
/**
 * StateSet iCommerce - Node.js Example
 *
 * Demonstrates the full commerce workflow:
 * - Customer creation
 * - Product catalog management
 * - Inventory tracking
 * - Order processing
 * - Analytics
 *
 * Run with: node basic_usage.js
 */

const { Commerce } = require('@stateset/embedded');

async function main() {
  console.log('=== StateSet iCommerce - Node.js Example ===\n');

  // Initialize commerce with in-memory database
  const commerce = new Commerce(':memory:');
  console.log('✓ Commerce initialized\n');

  // 1. Create a customer
  console.log('1. Creating customer...');
  const customer = await commerce.customers.create({
    email: 'alice@example.com',
    firstName: 'Alice',
    lastName: 'Smith',
    phone: '+1-555-0123'
  });
  console.log(`   Created customer: ${customer.firstName} ${customer.lastName} (${customer.email})`);

  // 2. Create products
  console.log('\n2. Creating products...');
  const widget = await commerce.products.create({
    name: 'Premium Widget',
    sku: 'WIDGET-001',
    price: 29.99,
    description: 'A high-quality widget for all your needs'
  });
  console.log(`   Created product: ${widget.name} (${widget.slug})`);

  const gadget = await commerce.products.create({
    name: 'Super Gadget',
    sku: 'GADGET-001',
    price: 49.99,
    description: 'An amazing gadget'
  });
  console.log(`   Created product: ${gadget.name} (${gadget.slug})`);

  // 3. Create inventory
  console.log('\n3. Setting up inventory...');
  await commerce.inventory.createItem({
    sku: 'WIDGET-001',
    name: 'Premium Widget',
    initialQuantity: 100
  });
  console.log('   Created inventory for WIDGET-001 (100 units)');

  await commerce.inventory.createItem({
    sku: 'GADGET-001',
    name: 'Super Gadget',
    initialQuantity: 50
  });
  console.log('   Created inventory for GADGET-001 (50 units)');

  // Check stock
  const widgetStock = await commerce.inventory.getStock('WIDGET-001');
  console.log(`   Stock check WIDGET-001: ${widgetStock.totalAvailable} available`);

  // 4. Create an order
  console.log('\n4. Creating order...');
  const order = await commerce.orders.create({
    customerId: customer.id,
    items: [
      {
        productId: widget.id,
        sku: 'WIDGET-001',
        name: 'Premium Widget',
        quantity: 2,
        unitPrice: 29.99
      },
      {
        productId: gadget.id,
        sku: 'GADGET-001',
        name: 'Super Gadget',
        quantity: 1,
        unitPrice: 49.99
      }
    ],
    currency: 'USD'
  });
  console.log(`   Created order ${order.orderNumber} (total: $${order.totalAmount})`);

  // 5. Process the order
  console.log('\n5. Processing order...');

  // Update order status
  let updatedOrder = await commerce.orders.updateStatus(order.id, 'confirmed');
  console.log(`   Order status: ${updatedOrder.status}`);

  // Adjust inventory (fulfill)
  await commerce.inventory.adjust('WIDGET-001', -2, 'Order fulfillment');
  await commerce.inventory.adjust('GADGET-001', -1, 'Order fulfillment');
  console.log('   Inventory adjusted');

  // Ship the order
  updatedOrder = await commerce.orders.ship(order.id, 'TRACK123456');
  console.log(`   Order shipped with tracking: ${updatedOrder.trackingNumber}`);

  // 6. Check final inventory
  console.log('\n6. Final inventory check...');
  const finalWidgetStock = await commerce.inventory.getStock('WIDGET-001');
  console.log(`   WIDGET-001: ${finalWidgetStock.totalAvailable} available (was 100)`);

  const finalGadgetStock = await commerce.inventory.getStock('GADGET-001');
  console.log(`   GADGET-001: ${finalGadgetStock.totalAvailable} available (was 50)`);

  // 7. Analytics
  console.log('\n7. Analytics...');
  const salesSummary = await commerce.analytics.getSalesSummary('today');
  console.log(`   Revenue: $${salesSummary.totalRevenue}`);
  console.log(`   Orders: ${salesSummary.orderCount}`);
  console.log(`   AOV: $${salesSummary.averageOrderValue}`);

  // 8. Summary
  console.log('\n=== Summary ===');
  const customers = await commerce.customers.list();
  const products = await commerce.products.list();
  const orders = await commerce.orders.list();
  console.log(`Customers: ${customers.length}`);
  console.log(`Products: ${products.length}`);
  console.log(`Orders: ${orders.length}`);

  console.log('\n✓ Example completed successfully!');
}

main().catch(console.error);
