#!/usr/bin/env node
/**
 * StateSet iCommerce - Full E-Commerce Workflow Example
 *
 * This example demonstrates a complete end-to-end e-commerce workflow:
 * 1. Store setup (products, inventory, tax, currency)
 * 2. Customer registration
 * 3. Shopping cart and checkout with ACP
 * 4. Payment processing
 * 5. Order fulfillment and shipping
 * 6. Returns and refunds
 * 7. Analytics and reporting
 *
 * Run with: node 09_full_workflow.js
 */

const { Commerce } = require('@stateset/embedded');

async function main() {
  console.log('╔══════════════════════════════════════════════════════════════╗');
  console.log('║        StateSet iCommerce - Full E-Commerce Workflow         ║');
  console.log('╚══════════════════════════════════════════════════════════════╝\n');

  const commerce = new Commerce(':memory:');

  // ============================================
  // PHASE 1: Store Setup
  // ============================================

  console.log('┌─────────────────────────────────────────────────────────────┐');
  console.log('│ PHASE 1: Store Setup                                        │');
  console.log('└─────────────────────────────────────────────────────────────┘\n');

  // Configure currency
  console.log('[1.1] Setting up currencies...');
  await commerce.currency.updateSettings({
    baseCurrency: 'USD',
    enabledCurrencies: ['USD', 'EUR', 'GBP', 'CAD'],
    autoConvert: true
  });
  await commerce.currency.setRates([
    { baseCurrency: 'USD', quoteCurrency: 'EUR', rate: 0.92, source: 'manual' },
    { baseCurrency: 'USD', quoteCurrency: 'GBP', rate: 0.79, source: 'manual' },
    { baseCurrency: 'USD', quoteCurrency: 'CAD', rate: 1.36, source: 'manual' }
  ]);
  console.log('    Currencies configured: USD (base), EUR, GBP, CAD\n');

  // Configure tax
  console.log('[1.2] Setting up tax...');
  await commerce.tax.updateSettings({
    enabled: true,
    taxShipping: true,
    roundingMode: 'half_up'
  });

  const taxJurisdiction = await commerce.tax.createJurisdiction({
    name: 'California',
    code: 'US-CA',
    level: 'state',
    countryCode: 'US',
    stateCode: 'CA'
  });

  await commerce.tax.createRate({
    jurisdictionId: taxJurisdiction.id,
    taxType: 'sales_tax',
    rate: 0.0875,
    name: 'California Sales Tax',
    effectiveFrom: '2024-01-01'
  });
  console.log('    Tax configured: 8.75% California sales tax\n');

  // Create products
  console.log('[1.3] Creating product catalog...');
  const products = {};

  products.laptop = await commerce.products.create({
    name: 'UltraBook Pro 15"',
    description: 'Lightweight professional laptop with stunning display',
    variants: [
      { sku: 'UB-PRO-8GB', name: '8GB RAM', price: 1299.99 },
      { sku: 'UB-PRO-16GB', name: '16GB RAM', price: 1499.99 },
      { sku: 'UB-PRO-32GB', name: '32GB RAM', price: 1899.99 }
    ]
  });
  console.log(`    Created: ${products.laptop.name}`);

  products.mouse = await commerce.products.create({
    name: 'ErgoMouse Wireless',
    description: 'Ergonomic wireless mouse with precision tracking',
    variants: [{ sku: 'ERGO-MOUSE', name: 'Standard', price: 79.99 }]
  });
  console.log(`    Created: ${products.mouse.name}`);

  products.keyboard = await commerce.products.create({
    name: 'MechBoard Pro',
    description: 'Mechanical keyboard with customizable RGB',
    variants: [
      { sku: 'MECH-KB-BLK', name: 'Black', price: 149.99 },
      { sku: 'MECH-KB-WHT', name: 'White', price: 149.99 }
    ]
  });
  console.log(`    Created: ${products.keyboard.name}`);

  products.headphones = await commerce.products.create({
    name: 'StudioPods Pro',
    description: 'Active noise-cancelling wireless headphones',
    variants: [{ sku: 'STUDIO-PODS', name: 'Standard', price: 349.99 }]
  });
  console.log(`    Created: ${products.headphones.name}\n`);

  // Set up inventory
  console.log('[1.4] Setting up inventory...');
  const inventoryItems = [
    { sku: 'UB-PRO-8GB', name: 'UltraBook 8GB', initialQuantity: 25, reorderPoint: 5 },
    { sku: 'UB-PRO-16GB', name: 'UltraBook 16GB', initialQuantity: 50, reorderPoint: 10 },
    { sku: 'UB-PRO-32GB', name: 'UltraBook 32GB', initialQuantity: 15, reorderPoint: 3 },
    { sku: 'ERGO-MOUSE', name: 'ErgoMouse', initialQuantity: 200, reorderPoint: 40 },
    { sku: 'MECH-KB-BLK', name: 'MechBoard Black', initialQuantity: 75, reorderPoint: 15 },
    { sku: 'MECH-KB-WHT', name: 'MechBoard White', initialQuantity: 50, reorderPoint: 10 },
    { sku: 'STUDIO-PODS', name: 'StudioPods', initialQuantity: 100, reorderPoint: 20 }
  ];

  for (const item of inventoryItems) {
    await commerce.inventory.createItem(item);
  }
  console.log(`    Created inventory for ${inventoryItems.length} SKUs\n`);

  // Create promotion
  console.log('[1.5] Setting up promotions...');
  const promotion = await commerce.promotions.create({
    name: 'Welcome 10% Off',
    description: 'Get 10% off your first order',
    promotionType: 'percentage_off',
    trigger: 'coupon_code',
    target: 'order',
    percentageOff: 0.10,
    maxDiscountAmount: 150,
    totalUsageLimit: 1000,
    perCustomerLimit: 1
  });

  await commerce.promotions.createCoupon({
    promotionId: promotion.id,
    code: 'WELCOME10'
  });
  console.log('    Created promotion: WELCOME10 (10% off, max $150)\n');

  // ============================================
  // PHASE 2: Customer Journey
  // ============================================

  console.log('┌─────────────────────────────────────────────────────────────┐');
  console.log('│ PHASE 2: Customer Journey                                   │');
  console.log('└─────────────────────────────────────────────────────────────┘\n');

  // Create customer
  console.log('[2.1] Customer registration...');
  const customer = await commerce.customers.create({
    email: 'sarah.tech@example.com',
    firstName: 'Sarah',
    lastName: 'Tech',
    phone: '+1-555-0199',
    acceptsMarketing: true
  });
  console.log(`    Registered: ${customer.firstName} ${customer.lastName} (${customer.email})`);
  console.log(`    Customer ID: ${customer.id}\n`);

  // Create shopping cart
  console.log('[2.2] Creating shopping cart...');
  const cart = await commerce.carts.create({
    customerId: customer.id,
    customerEmail: customer.email,
    customerName: `${customer.firstName} ${customer.lastName}`,
    currency: 'USD',
    expiresInMinutes: 60
  });
  console.log(`    Cart: ${cart.cartNumber}`);
  console.log(`    Status: ${cart.status}\n`);

  // Add items to cart
  console.log('[2.3] Adding items to cart...');

  const cartItem1 = await commerce.carts.addItem(cart.id, {
    productId: products.laptop.id,
    sku: 'UB-PRO-16GB',
    name: 'UltraBook Pro 15" - 16GB RAM',
    quantity: 1,
    unitPrice: 1499.99,
    requiresShipping: true
  });
  console.log(`    Added: ${cartItem1.name} - $${cartItem1.unitPrice}`);

  const cartItem2 = await commerce.carts.addItem(cart.id, {
    productId: products.mouse.id,
    sku: 'ERGO-MOUSE',
    name: 'ErgoMouse Wireless',
    quantity: 1,
    unitPrice: 79.99,
    requiresShipping: true
  });
  console.log(`    Added: ${cartItem2.name} - $${cartItem2.unitPrice}`);

  const cartItem3 = await commerce.carts.addItem(cart.id, {
    productId: products.headphones.id,
    sku: 'STUDIO-PODS',
    name: 'StudioPods Pro',
    quantity: 1,
    unitPrice: 349.99,
    requiresShipping: true
  });
  console.log(`    Added: ${cartItem3.name} - $${cartItem3.unitPrice}\n`);

  // Apply coupon
  console.log('[2.4] Applying coupon code...');
  let updatedCart = await commerce.carts.applyDiscount(cart.id, 'WELCOME10');
  console.log(`    Applied coupon: ${updatedCart.couponCode}\n`);

  // Set shipping address
  console.log('[2.5] Setting shipping information...');
  const shippingAddress = {
    firstName: 'Sarah',
    lastName: 'Tech',
    company: 'TechStart Inc.',
    line1: '456 Innovation Blvd',
    line2: 'Suite 300',
    city: 'San Jose',
    state: 'CA',
    postalCode: '95110',
    country: 'US',
    phone: '+1-555-0199',
    email: 'sarah.tech@example.com'
  };

  updatedCart = await commerce.carts.setShipping(cart.id, {
    shippingAddress: shippingAddress,
    shippingMethod: 'express',
    shippingCarrier: 'fedex',
    shippingAmount: 24.99
  });
  console.log(`    Shipping: ${updatedCart.shippingMethod} via ${updatedCart.shippingCarrier}`);
  console.log(`    Address: ${shippingAddress.city}, ${shippingAddress.state}\n`);

  // Set billing address
  console.log('[2.6] Setting billing address...');
  updatedCart = await commerce.carts.setBillingAddress(cart.id, shippingAddress);
  console.log('    Billing address set (same as shipping)\n');

  // Reserve inventory
  console.log('[2.7] Reserving inventory...');
  updatedCart = await commerce.carts.reserveInventory(cart.id);
  console.log(`    Inventory reserved: ${updatedCart.inventoryReserved}\n`);

  // Calculate tax
  console.log('[2.8] Calculating tax...');
  const taxCalc = await commerce.tax.calculate({
    lineItems: [
      { id: cartItem1.id, quantity: 1, unitPrice: 1499.99 },
      { id: cartItem2.id, quantity: 1, unitPrice: 79.99 },
      { id: cartItem3.id, quantity: 1, unitPrice: 349.99 }
    ],
    shippingAddress: { state: 'CA', country: 'US' },
    shippingAmount: 24.99
  });
  console.log(`    Tax calculated: $${taxCalc.totalTax.toFixed(2)}\n`);

  // Set tax and payment
  updatedCart = await commerce.carts.setTax(cart.id, taxCalc.totalTax);
  updatedCart = await commerce.carts.setPayment(cart.id, {
    paymentMethod: 'credit_card',
    paymentToken: 'tok_visa_4242'
  });

  // Recalculate and review
  console.log('[2.9] Cart summary before checkout...');
  updatedCart = await commerce.carts.recalculate(cart.id);
  console.log('    ┌────────────────────────────────────┐');
  console.log(`    │ Cart: ${updatedCart.cartNumber}              │`);
  console.log('    ├────────────────────────────────────┤');
  console.log(`    │ Items:          ${updatedCart.itemCount}                  │`);
  console.log(`    │ Subtotal:       $${updatedCart.subtotal.toFixed(2).padStart(10)}     │`);
  console.log(`    │ Discount:      -$${updatedCart.discountAmount.toFixed(2).padStart(10)}     │`);
  console.log(`    │ Shipping:       $${updatedCart.shippingAmount.toFixed(2).padStart(10)}     │`);
  console.log(`    │ Tax:            $${updatedCart.taxAmount.toFixed(2).padStart(10)}     │`);
  console.log('    ├────────────────────────────────────┤');
  console.log(`    │ TOTAL:          $${updatedCart.grandTotal.toFixed(2).padStart(10)}     │`);
  console.log('    └────────────────────────────────────┘\n');

  // ============================================
  // PHASE 3: Checkout & Payment
  // ============================================

  console.log('┌─────────────────────────────────────────────────────────────┐');
  console.log('│ PHASE 3: Checkout & Payment                                 │');
  console.log('└─────────────────────────────────────────────────────────────┘\n');

  console.log('[3.1] Beginning checkout...');
  await commerce.carts.beginCheckout(cart.id);
  await commerce.carts.markReadyForPayment(cart.id);
  console.log('    Cart ready for payment\n');

  console.log('[3.2] Completing checkout...');
  const checkoutResult = await commerce.carts.complete(cart.id);

  console.log('    ╔════════════════════════════════════════╗');
  console.log('    ║         CHECKOUT COMPLETE!             ║');
  console.log('    ╠════════════════════════════════════════╣');
  console.log(`    ║ Order #:    ${checkoutResult.orderNumber.padEnd(22)}    ║`);
  console.log(`    ║ Amount:     $${checkoutResult.totalCharged.toFixed(2).padStart(10)} ${checkoutResult.currency.padEnd(10)} ║`);
  console.log('    ╚════════════════════════════════════════╝\n');

  // Create payment record
  console.log('[3.3] Processing payment...');
  const payment = await commerce.payments.create({
    orderId: checkoutResult.orderId,
    customerId: customer.id,
    amount: checkoutResult.totalCharged,
    currency: 'USD',
    paymentMethod: 'credit_card'
  });
  await commerce.payments.markCompleted(payment.id);
  console.log(`    Payment ${payment.paymentNumber}: COMPLETED\n`);

  // ============================================
  // PHASE 4: Order Fulfillment
  // ============================================

  console.log('┌─────────────────────────────────────────────────────────────┐');
  console.log('│ PHASE 4: Order Fulfillment                                  │');
  console.log('└─────────────────────────────────────────────────────────────┘\n');

  // Get order
  const order = await commerce.orders.get(checkoutResult.orderId);

  console.log('[4.1] Confirming order...');
  await commerce.orders.updateStatus(order.id, 'confirmed');
  console.log(`    Order ${order.orderNumber} confirmed\n`);

  console.log('[4.2] Creating shipment...');
  const shipment = await commerce.shipments.create({
    orderId: order.id,
    recipientName: `${customer.firstName} ${customer.lastName}`,
    shippingAddress: `${shippingAddress.line1}, ${shippingAddress.city}, ${shippingAddress.state} ${shippingAddress.postalCode}`,
    carrier: 'fedex',
    shippingMethod: 'express',
    trackingNumber: 'FDX123456789',
    recipientEmail: customer.email,
    recipientPhone: customer.phone
  });
  console.log(`    Shipment ${shipment.shipmentNumber} created`);
  console.log(`    Carrier: ${shipment.carrier} (${shipment.shippingMethod})\n`);

  console.log('[4.3] Shipping order...');
  await commerce.shipments.ship(shipment.id);
  await commerce.orders.ship(order.id, 'FDX123456789');
  console.log(`    Tracking: ${shipment.trackingNumber}`);
  console.log('    Status: SHIPPED\n');

  console.log('[4.4] Marking as delivered...');
  await commerce.shipments.deliver(shipment.id);
  console.log('    Status: DELIVERED\n');

  // Adjust inventory for fulfilled items
  console.log('[4.5] Updating inventory...');
  await commerce.inventory.adjust('UB-PRO-16GB', -1, 'Order fulfilled');
  await commerce.inventory.adjust('ERGO-MOUSE', -1, 'Order fulfilled');
  await commerce.inventory.adjust('STUDIO-PODS', -1, 'Order fulfilled');
  console.log('    Inventory adjusted for fulfilled items\n');

  // ============================================
  // PHASE 5: Post-Purchase (Warranty & Return)
  // ============================================

  console.log('┌─────────────────────────────────────────────────────────────┐');
  console.log('│ PHASE 5: Post-Purchase                                      │');
  console.log('└─────────────────────────────────────────────────────────────┘\n');

  // Create warranty
  console.log('[5.1] Registering warranty...');
  const warranty = await commerce.warranties.create({
    customerId: customer.id,
    productId: products.laptop.id,
    orderId: order.id,
    warrantyType: 'standard',
    durationMonths: 24,
    serialNumber: 'UB-2024-001234'
  });
  console.log(`    Warranty ${warranty.warrantyNumber} registered`);
  console.log(`    Valid until: ${warranty.endDate}`);
  console.log(`    Serial: ${warranty.serialNumber}\n`);

  // Create a return for the headphones
  console.log('[5.2] Processing return request...');
  const orderItems = order.items;
  const headphonesItem = orderItems.find(i => i.sku === 'STUDIO-PODS');

  const returnRequest = await commerce.returns.create({
    orderId: order.id,
    reason: 'not_as_described',
    reasonDetails: 'Customer expected different sound profile',
    items: [{ orderItemId: headphonesItem.id, quantity: 1 }]
  });
  console.log(`    Return ${returnRequest.id.slice(0, 8)}... created`);
  console.log(`    Reason: ${returnRequest.reason}\n`);

  console.log('[5.3] Approving return...');
  await commerce.returns.approve(returnRequest.id);
  console.log('    Return approved\n');

  console.log('[5.4] Processing refund...');
  const refund = await commerce.payments.createRefund({
    paymentId: payment.id,
    amount: 349.99,
    reason: 'Return - product not as described'
  });
  console.log(`    Refund ${refund.refundNumber}: $${refund.amount}`);
  console.log(`    Status: ${refund.status}\n`);

  // Restore inventory
  await commerce.inventory.adjust('STUDIO-PODS', 1, 'Return restocked');
  console.log('    Inventory restored for returned item\n');

  // ============================================
  // PHASE 6: Analytics & Reporting
  // ============================================

  console.log('┌─────────────────────────────────────────────────────────────┐');
  console.log('│ PHASE 6: Analytics & Reporting                              │');
  console.log('└─────────────────────────────────────────────────────────────┘\n');

  console.log('[6.1] Sales summary...');
  const salesSummary = await commerce.analytics.salesSummary({ period: 'today' });
  console.log(`    Revenue: $${salesSummary.totalRevenue.toFixed(2)}`);
  console.log(`    Orders: ${salesSummary.orderCount}`);
  console.log(`    AOV: $${salesSummary.averageOrderValue.toFixed(2)}`);
  console.log(`    Items sold: ${salesSummary.itemsSold}\n`);

  console.log('[6.2] Customer metrics...');
  const customerMetrics = await commerce.analytics.customerMetrics();
  console.log(`    Total customers: ${customerMetrics.totalCustomers}`);
  console.log(`    New customers: ${customerMetrics.newCustomers}\n`);

  console.log('[6.3] Inventory health...');
  const invHealth = await commerce.analytics.inventoryHealth();
  console.log(`    Total SKUs: ${invHealth.totalSkus}`);
  console.log(`    In stock: ${invHealth.inStockSkus}`);
  console.log(`    Low stock: ${invHealth.lowStockSkus}`);
  console.log(`    Out of stock: ${invHealth.outOfStockSkus}\n`);

  console.log('[6.4] Return metrics...');
  const returnMetrics = await commerce.analytics.returnMetrics();
  console.log(`    Total returns: ${returnMetrics.totalReturns}`);
  console.log(`    Return rate: ${returnMetrics.returnRatePercent.toFixed(2)}%`);
  console.log(`    Total refunded: $${returnMetrics.totalRefunded.toFixed(2)}\n`);

  console.log('[6.5] Order status breakdown...');
  const statusBreakdown = await commerce.analytics.orderStatusBreakdown();
  console.log(`    Shipped: ${statusBreakdown.shipped}`);
  console.log(`    Delivered: ${statusBreakdown.delivered}\n`);

  // ============================================
  // Summary
  // ============================================

  console.log('┌─────────────────────────────────────────────────────────────┐');
  console.log('│ WORKFLOW COMPLETE                                           │');
  console.log('└─────────────────────────────────────────────────────────────┘\n');

  console.log('Summary:');
  console.log(`  - Customer: ${customer.firstName} ${customer.lastName}`);
  console.log(`  - Order: ${order.orderNumber}`);
  console.log(`  - Original total: $${checkoutResult.totalCharged.toFixed(2)}`);
  console.log(`  - Refund: $${refund.amount.toFixed(2)}`);
  console.log(`  - Net revenue: $${(checkoutResult.totalCharged - refund.amount).toFixed(2)}`);
  console.log(`  - Warranty registered: ${warranty.warrantyNumber}`);
  console.log(`  - Shipping: ${shipment.trackingNumber} (DELIVERED)`);

  console.log('\n╔══════════════════════════════════════════════════════════════╗');
  console.log('║        Full E-Commerce Workflow Example Complete!            ║');
  console.log('╚══════════════════════════════════════════════════════════════╝\n');
}

main().catch(console.error);
