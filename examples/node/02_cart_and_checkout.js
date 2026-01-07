#!/usr/bin/env node
/**
 * StateSet iCommerce - Shopping Cart & Checkout Example
 *
 * This example demonstrates the Agentic Commerce Protocol (ACP):
 * - Creating and managing shopping carts
 * - Adding/updating/removing items
 * - Setting shipping and billing addresses
 * - Applying discounts
 * - Completing checkout
 * - Inventory reservation
 *
 * Run with: node 02_cart_and_checkout.js
 */

const { Commerce } = require('@stateset/embedded');

async function main() {
  console.log('=== StateSet iCommerce - Cart & Checkout ===\n');

  const commerce = new Commerce(':memory:');
  console.log('[Setup] Commerce engine initialized\n');

  // ============================================
  // Setup: Create products and inventory
  // ============================================

  console.log('[Setup] Creating products and inventory...');

  const laptop = await commerce.products.create({
    name: 'Pro Laptop 15"',
    description: 'High-performance laptop for professionals',
    variants: [
      { sku: 'LAPTOP-BASE', name: 'Base Model', price: 1299.99 },
      { sku: 'LAPTOP-PRO', name: 'Pro Model', price: 1799.99 }
    ]
  });

  const mouse = await commerce.products.create({
    name: 'Wireless Mouse',
    description: 'Ergonomic wireless mouse',
    variants: [{ sku: 'MOUSE-001', name: 'Standard', price: 49.99 }]
  });

  const keyboard = await commerce.products.create({
    name: 'Mechanical Keyboard',
    description: 'RGB mechanical keyboard',
    variants: [{ sku: 'KB-001', name: 'Standard', price: 129.99 }]
  });

  // Create inventory
  await commerce.inventory.createItem({ sku: 'LAPTOP-BASE', name: 'Laptop Base', initialQuantity: 20 });
  await commerce.inventory.createItem({ sku: 'LAPTOP-PRO', name: 'Laptop Pro', initialQuantity: 10 });
  await commerce.inventory.createItem({ sku: 'MOUSE-001', name: 'Mouse', initialQuantity: 100 });
  await commerce.inventory.createItem({ sku: 'KB-001', name: 'Keyboard', initialQuantity: 50 });

  // Create customer
  const customer = await commerce.customers.create({
    email: 'jane@example.com',
    firstName: 'Jane',
    lastName: 'Doe',
    phone: '+1-555-0789'
  });

  console.log('    Products and inventory created\n');

  // ============================================
  // 1. Create a Shopping Cart
  // ============================================

  console.log('[1] Creating shopping cart...');

  const cart = await commerce.carts.create({
    customerId: customer.id,
    customerEmail: customer.email,
    customerName: `${customer.firstName} ${customer.lastName}`,
    currency: 'USD',
    expiresInMinutes: 60 // Cart expires in 1 hour
  });

  console.log(`    Cart created: ${cart.cartNumber}`);
  console.log(`    Status: ${cart.status}`);
  console.log(`    Currency: ${cart.currency}\n`);

  // ============================================
  // 2. Add Items to Cart
  // ============================================

  console.log('[2] Adding items to cart...');

  const item1 = await commerce.carts.addItem(cart.id, {
    productId: laptop.id,
    sku: 'LAPTOP-PRO',
    name: 'Pro Laptop 15" - Pro Model',
    description: 'High-performance laptop',
    quantity: 1,
    unitPrice: 1799.99,
    requiresShipping: true
  });
  console.log(`    Added: ${item1.name} x${item1.quantity} = $${item1.total}`);

  const item2 = await commerce.carts.addItem(cart.id, {
    productId: mouse.id,
    sku: 'MOUSE-001',
    name: 'Wireless Mouse',
    quantity: 2,
    unitPrice: 49.99,
    requiresShipping: true
  });
  console.log(`    Added: ${item2.name} x${item2.quantity} = $${item2.total}`);

  const item3 = await commerce.carts.addItem(cart.id, {
    productId: keyboard.id,
    sku: 'KB-001',
    name: 'Mechanical Keyboard',
    quantity: 1,
    unitPrice: 129.99,
    requiresShipping: true
  });
  console.log(`    Added: ${item3.name} x${item3.quantity} = $${item3.total}`);

  // Get cart items
  let items = await commerce.carts.getItems(cart.id);
  console.log(`    Total items in cart: ${items.length}\n`);

  // ============================================
  // 3. Update Cart Items
  // ============================================

  console.log('[3] Updating cart items...');

  // Update mouse quantity
  const updatedItem = await commerce.carts.updateItem(item2.id, {
    quantity: 1 // Changed from 2 to 1
  });
  console.log(`    Updated: ${updatedItem.name} quantity to ${updatedItem.quantity}`);

  // Remove keyboard
  await commerce.carts.removeItem(item3.id);
  console.log(`    Removed: Mechanical Keyboard`);

  // Check updated cart
  let updatedCart = await commerce.carts.get(cart.id);
  console.log(`    Updated subtotal: $${updatedCart.subtotal}\n`);

  // ============================================
  // 4. Set Shipping Address
  // ============================================

  console.log('[4] Setting shipping address...');

  updatedCart = await commerce.carts.setShipping(cart.id, {
    shippingAddress: {
      firstName: 'Jane',
      lastName: 'Doe',
      company: 'Tech Corp',
      line1: '123 Main Street',
      line2: 'Suite 400',
      city: 'San Francisco',
      state: 'CA',
      postalCode: '94105',
      country: 'US',
      phone: '+1-555-0789',
      email: 'jane@example.com'
    },
    shippingMethod: 'express',
    shippingCarrier: 'ups',
    shippingAmount: 25.00
  });

  console.log(`    Shipping address set: ${updatedCart.shippingAddress.city}, ${updatedCart.shippingAddress.state}`);
  console.log(`    Shipping method: ${updatedCart.shippingMethod} (${updatedCart.shippingCarrier})`);
  console.log(`    Shipping cost: $${updatedCart.shippingAmount}\n`);

  // ============================================
  // 5. Set Billing Address
  // ============================================

  console.log('[5] Setting billing address...');

  updatedCart = await commerce.carts.setBillingAddress(cart.id, {
    firstName: 'Jane',
    lastName: 'Doe',
    line1: '123 Main Street',
    line2: 'Suite 400',
    city: 'San Francisco',
    state: 'CA',
    postalCode: '94105',
    country: 'US'
  });

  console.log(`    Billing address set: ${updatedCart.billingAddress.line1}\n`);

  // ============================================
  // 6. Reserve Inventory
  // ============================================

  console.log('[6] Reserving inventory for cart...');

  // Check stock before reservation
  let laptopStock = await commerce.inventory.getStock('LAPTOP-PRO');
  console.log(`    LAPTOP-PRO before: ${laptopStock.totalAvailable} available`);

  // Reserve inventory
  updatedCart = await commerce.carts.reserveInventory(cart.id);
  console.log(`    Inventory reserved: ${updatedCart.inventoryReserved}`);

  // Check stock after reservation
  laptopStock = await commerce.inventory.getStock('LAPTOP-PRO');
  console.log(`    LAPTOP-PRO after: ${laptopStock.totalAvailable} available, ${laptopStock.totalAllocated} allocated\n`);

  // ============================================
  // 7. Set Payment Method
  // ============================================

  console.log('[7] Setting payment method...');

  updatedCart = await commerce.carts.setPayment(cart.id, {
    paymentMethod: 'credit_card',
    paymentToken: 'tok_visa_4242' // Mock token
  });

  console.log(`    Payment method: ${updatedCart.paymentMethod}\n`);

  // ============================================
  // 8. Review Cart Before Checkout
  // ============================================

  console.log('[8] Reviewing cart before checkout...');

  // Recalculate totals
  updatedCart = await commerce.carts.recalculate(cart.id);

  console.log(`    Cart Number: ${updatedCart.cartNumber}`);
  console.log(`    Customer: ${updatedCart.customerName} (${updatedCart.customerEmail})`);
  console.log(`    Items: ${updatedCart.itemCount}`);
  console.log(`    Subtotal: $${updatedCart.subtotal}`);
  console.log(`    Shipping: $${updatedCart.shippingAmount}`);
  console.log(`    Tax: $${updatedCart.taxAmount}`);
  console.log(`    Discount: $${updatedCart.discountAmount}`);
  console.log(`    Grand Total: $${updatedCart.grandTotal}`);
  console.log(`    Inventory Reserved: ${updatedCart.inventoryReserved}\n`);

  // ============================================
  // 9. Complete Checkout
  // ============================================

  console.log('[9] Completing checkout...');

  // Begin checkout process
  await commerce.carts.beginCheckout(cart.id);
  console.log('    Checkout initiated');

  // Mark ready for payment
  await commerce.carts.markReadyForPayment(cart.id);
  console.log('    Cart ready for payment');

  // Complete checkout
  const result = await commerce.carts.complete(cart.id);

  console.log('\n    === CHECKOUT COMPLETE ===');
  console.log(`    Order ID: ${result.orderId}`);
  console.log(`    Order Number: ${result.orderNumber}`);
  console.log(`    Total Charged: $${result.totalCharged} ${result.currency}`);
  if (result.paymentId) {
    console.log(`    Payment ID: ${result.paymentId}`);
  }

  // ============================================
  // 10. Verify Order Created
  // ============================================

  console.log('\n[10] Verifying order...');

  const order = await commerce.orders.get(result.orderId);
  console.log(`    Order Status: ${order.status}`);
  console.log(`    Payment Status: ${order.paymentStatus}`);
  console.log(`    Fulfillment Status: ${order.fulfillmentStatus}`);
  console.log(`    Items: ${order.items.length}`);

  // Check final inventory
  laptopStock = await commerce.inventory.getStock('LAPTOP-PRO');
  const mouseStock = await commerce.inventory.getStock('MOUSE-001');
  console.log(`    Final LAPTOP-PRO stock: ${laptopStock.totalAvailable}`);
  console.log(`    Final MOUSE-001 stock: ${mouseStock.totalAvailable}`);

  // ============================================
  // Demo: Abandoned Cart Flow
  // ============================================

  console.log('\n[Demo] Abandoned cart flow...');

  const abandonedCart = await commerce.carts.create({
    customerEmail: 'abandoned@example.com',
    currency: 'USD'
  });

  await commerce.carts.addItem(abandonedCart.id, {
    sku: 'MOUSE-001',
    name: 'Wireless Mouse',
    quantity: 1,
    unitPrice: 49.99
  });

  // Mark as abandoned
  await commerce.carts.abandon(abandonedCart.id);

  // Query abandoned carts
  const abandonedCarts = await commerce.carts.getAbandoned();
  console.log(`    Abandoned carts: ${abandonedCarts.length}`);

  // Demo: Expired cart
  const expiredCart = await commerce.carts.create({
    customerEmail: 'expired@example.com',
    currency: 'USD'
  });
  await commerce.carts.expire(expiredCart.id);

  const expiredCarts = await commerce.carts.getExpired();
  console.log(`    Expired carts: ${expiredCarts.length}`);

  // Cart count
  const totalCarts = await commerce.carts.count();
  console.log(`    Total carts: ${totalCarts}`);

  console.log('\n=== Cart & Checkout Example Complete ===');
}

main().catch(console.error);
