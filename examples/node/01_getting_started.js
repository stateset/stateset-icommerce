#!/usr/bin/env node
/**
 * StateSet iCommerce - Getting Started Example
 *
 * This example demonstrates the basic setup and core operations:
 * - Initializing the Commerce engine
 * - Creating customers, products, and orders
 * - Basic inventory management
 *
 * Run with: node 01_getting_started.js
 */

const { Commerce } = require('@stateset/embedded');

async function main() {
  console.log('=== StateSet iCommerce - Getting Started ===\n');

  // ============================================
  // 1. Initialize Commerce Engine
  // ============================================

  // Use ':memory:' for in-memory database (great for testing)
  // Use './store.db' for persistent storage
  const commerce = new Commerce(':memory:');
  console.log('[1] Commerce engine initialized with in-memory database\n');

  // ============================================
  // 2. Create Customers
  // ============================================

  console.log('[2] Creating customers...');

  const customer1 = await commerce.customers.create({
    email: 'alice@example.com',
    firstName: 'Alice',
    lastName: 'Smith',
    phone: '+1-555-0123',
    acceptsMarketing: true
  });
  console.log(`    Created: ${customer1.firstName} ${customer1.lastName} (ID: ${customer1.id})`);

  const customer2 = await commerce.customers.create({
    email: 'bob@example.com',
    firstName: 'Bob',
    lastName: 'Johnson',
    phone: '+1-555-0456',
    acceptsMarketing: false
  });
  console.log(`    Created: ${customer2.firstName} ${customer2.lastName} (ID: ${customer2.id})`);

  // Retrieve customer by email
  const foundCustomer = await commerce.customers.getByEmail('alice@example.com');
  console.log(`    Found by email: ${foundCustomer.email}`);

  // Count customers
  const customerCount = await commerce.customers.count();
  console.log(`    Total customers: ${customerCount}\n`);

  // ============================================
  // 3. Create Products with Variants
  // ============================================

  console.log('[3] Creating products with variants...');

  // Product with multiple variants
  const tshirt = await commerce.products.create({
    name: 'Classic T-Shirt',
    description: 'Comfortable cotton t-shirt available in multiple sizes',
    variants: [
      { sku: 'TSHIRT-S', name: 'Small', price: 24.99 },
      { sku: 'TSHIRT-M', name: 'Medium', price: 24.99 },
      { sku: 'TSHIRT-L', name: 'Large', price: 24.99 },
      { sku: 'TSHIRT-XL', name: 'Extra Large', price: 27.99, compareAtPrice: 29.99 }
    ]
  });
  console.log(`    Created: ${tshirt.name} (ID: ${tshirt.id}, Slug: ${tshirt.slug})`);

  // Simple product with single variant
  const mug = await commerce.products.create({
    name: 'Coffee Mug',
    description: 'Ceramic mug with StateSet logo',
    variants: [
      { sku: 'MUG-001', name: 'Standard', price: 14.99 }
    ]
  });
  console.log(`    Created: ${mug.name} (ID: ${mug.id})`);

  // Get variant by SKU
  const variant = await commerce.products.getVariantBySku('TSHIRT-M');
  console.log(`    Variant lookup: ${variant.sku} = $${variant.price}`);

  // List all products
  const products = await commerce.products.list();
  console.log(`    Total products: ${products.length}\n`);

  // ============================================
  // 4. Set Up Inventory
  // ============================================

  console.log('[4] Setting up inventory...');

  // Create inventory items for each SKU
  const inventoryItems = [
    { sku: 'TSHIRT-S', name: 'T-Shirt Small', initialQuantity: 50, reorderPoint: 10 },
    { sku: 'TSHIRT-M', name: 'T-Shirt Medium', initialQuantity: 75, reorderPoint: 15 },
    { sku: 'TSHIRT-L', name: 'T-Shirt Large', initialQuantity: 60, reorderPoint: 12 },
    { sku: 'TSHIRT-XL', name: 'T-Shirt XL', initialQuantity: 40, reorderPoint: 8 },
    { sku: 'MUG-001', name: 'Coffee Mug', initialQuantity: 100, reorderPoint: 20 }
  ];

  for (const item of inventoryItems) {
    await commerce.inventory.createItem(item);
    console.log(`    Created inventory: ${item.sku} (${item.initialQuantity} units)`);
  }

  // Check stock level
  const mugStock = await commerce.inventory.getStock('MUG-001');
  console.log(`    Stock check MUG-001: ${mugStock.totalOnHand} on hand, ${mugStock.totalAvailable} available\n`);

  // ============================================
  // 5. Create Orders
  // ============================================

  console.log('[5] Creating orders...');

  // Order 1: Alice buys t-shirts
  const order1 = await commerce.orders.create({
    customerId: customer1.id,
    items: [
      { sku: 'TSHIRT-M', name: 'T-Shirt Medium', quantity: 2, unitPrice: 24.99 },
      { sku: 'TSHIRT-L', name: 'T-Shirt Large', quantity: 1, unitPrice: 24.99 }
    ],
    currency: 'USD',
    notes: 'Gift wrapping requested'
  });
  console.log(`    Order ${order1.orderNumber}: $${order1.totalAmount} (${order1.status})`);

  // Order 2: Bob buys mugs
  const order2 = await commerce.orders.create({
    customerId: customer2.id,
    items: [
      { sku: 'MUG-001', name: 'Coffee Mug', quantity: 4, unitPrice: 14.99 }
    ],
    currency: 'USD'
  });
  console.log(`    Order ${order2.orderNumber}: $${order2.totalAmount} (${order2.status})\n`);

  // ============================================
  // 6. Process Orders
  // ============================================

  console.log('[6] Processing orders...');

  // Confirm order 1
  let updatedOrder1 = await commerce.orders.updateStatus(order1.id, 'confirmed');
  console.log(`    ${order1.orderNumber} status: ${updatedOrder1.status}`);

  // Adjust inventory for order 1
  await commerce.inventory.adjust('TSHIRT-M', -2, 'Order fulfillment');
  await commerce.inventory.adjust('TSHIRT-L', -1, 'Order fulfillment');
  console.log(`    Inventory adjusted for ${order1.orderNumber}`);

  // Ship order 1
  updatedOrder1 = await commerce.orders.ship(order1.id, 'UPS123456789');
  console.log(`    ${order1.orderNumber} shipped with tracking: ${updatedOrder1.trackingNumber}`);

  // Process order 2
  let updatedOrder2 = await commerce.orders.updateStatus(order2.id, 'confirmed');
  await commerce.inventory.adjust('MUG-001', -4, 'Order fulfillment');
  updatedOrder2 = await commerce.orders.ship(order2.id, 'FEDEX987654321');
  console.log(`    ${order2.orderNumber} shipped with tracking: ${updatedOrder2.trackingNumber}\n`);

  // ============================================
  // 7. Check Final State
  // ============================================

  console.log('[7] Final state summary...');

  // Get counts
  const finalCustomers = await commerce.customers.count();
  const finalProducts = await commerce.products.count();
  const finalOrders = await commerce.orders.count();

  console.log(`    Customers: ${finalCustomers}`);
  console.log(`    Products: ${finalProducts}`);
  console.log(`    Orders: ${finalOrders}`);

  // Check inventory after fulfillment
  const tshirtMStock = await commerce.inventory.getStock('TSHIRT-M');
  const mugFinalStock = await commerce.inventory.getStock('MUG-001');
  console.log(`    T-Shirt M inventory: ${tshirtMStock.totalAvailable} available (was 75)`);
  console.log(`    Mug inventory: ${mugFinalStock.totalAvailable} available (was 100)`);

  // List all orders
  const allOrders = await commerce.orders.list();
  console.log('\n    All Orders:');
  for (const order of allOrders) {
    console.log(`      - ${order.orderNumber}: $${order.totalAmount} (${order.status})`);
  }

  console.log('\n=== Getting Started Example Complete ===');
}

main().catch(console.error);
