#!/usr/bin/env node
/**
 * StateSet iCommerce - Payments & Fulfillment Example
 *
 * This example demonstrates:
 * - Payment processing and refunds
 * - Shipment creation and tracking
 * - Returns and warranty management
 *
 * Run with: node 10_payments_and_fulfillment.js
 */

const { Commerce } = require('@stateset/embedded');

async function main() {
  console.log('=== StateSet iCommerce - Payments & Fulfillment ===\n');

  const commerce = new Commerce(':memory:');

  // ============================================
  // Setup: Create customer and order
  // ============================================

  console.log('[Setup] Creating test data...');

  const customer = await commerce.customers.create({
    email: 'buyer@example.com',
    firstName: 'Test',
    lastName: 'Buyer'
  });

  const product = await commerce.products.create({
    name: 'Premium Widget',
    variants: [{ sku: 'WIDGET-001', name: 'Standard', price: 99.99 }]
  });

  await commerce.inventory.createItem({
    sku: 'WIDGET-001',
    name: 'Widget',
    initialQuantity: 100
  });

  const order = await commerce.orders.create({
    customerId: customer.id,
    items: [
      { sku: 'WIDGET-001', name: 'Premium Widget', quantity: 3, unitPrice: 99.99 }
    ],
    currency: 'USD'
  });

  console.log(`    Customer: ${customer.firstName} ${customer.lastName}`);
  console.log(`    Order: ${order.orderNumber} ($${order.totalAmount})\n`);

  // ============================================
  // 1. Payment Processing
  // ============================================

  console.log('[1] Payment Processing...\n');

  // Create a payment
  console.log('    Creating payment...');
  const payment = await commerce.payments.create({
    orderId: order.id,
    customerId: customer.id,
    amount: order.totalAmount,
    currency: 'USD',
    paymentMethod: 'credit_card'
  });

  console.log(`    Payment: ${payment.paymentNumber}`);
  console.log(`      Amount: $${payment.amount} ${payment.currency}`);
  console.log(`      Status: ${payment.status}`);
  console.log(`      Method: ${payment.paymentMethod}`);

  // Mark payment as completed
  console.log('\n    Completing payment...');
  const completedPayment = await commerce.payments.markCompleted(payment.id);
  console.log(`      Status: ${completedPayment.status}`);

  // List payments
  const payments = await commerce.payments.list();
  console.log(`\n    Total payments: ${payments.length}`);

  // Payment count
  const paymentCount = await commerce.payments.count();
  console.log(`    Payment count: ${paymentCount}\n`);

  // ============================================
  // 2. Partial Refund
  // ============================================

  console.log('[2] Processing Partial Refund...\n');

  // Create partial refund (refund 1 item)
  const refund = await commerce.payments.createRefund({
    paymentId: payment.id,
    amount: 99.99,
    reason: 'Customer requested refund for 1 item'
  });

  console.log(`    Refund: ${refund.refundNumber}`);
  console.log(`      Amount: $${refund.amount}`);
  console.log(`      Status: ${refund.status}`);
  console.log(`      Reason: ${refund.reason}\n`);

  // ============================================
  // 3. Failed Payment Scenario
  // ============================================

  console.log('[3] Failed Payment Scenario...\n');

  // Create another payment that will fail
  const failedPayment = await commerce.payments.create({
    customerId: customer.id,
    amount: 500.00,
    currency: 'USD',
    paymentMethod: 'credit_card'
  });

  console.log(`    Payment: ${failedPayment.paymentNumber}`);

  // Mark as failed
  const markedFailed = await commerce.payments.markFailed(
    failedPayment.id,
    'Card declined - insufficient funds',
    'card_declined'
  );
  console.log(`      Status: ${markedFailed.status}`);

  // Cancel a payment
  const anotherPayment = await commerce.payments.create({
    customerId: customer.id,
    amount: 200.00,
    currency: 'USD'
  });
  const cancelledPayment = await commerce.payments.cancel(anotherPayment.id);
  console.log(`    Cancelled payment: ${cancelledPayment.paymentNumber} (${cancelledPayment.status})\n`);

  // ============================================
  // 4. Shipment Creation
  // ============================================

  console.log('[4] Creating Shipments...\n');

  // Confirm order first
  await commerce.orders.updateStatus(order.id, 'confirmed');

  // Create shipment
  const shipment = await commerce.shipments.create({
    orderId: order.id,
    recipientName: `${customer.firstName} ${customer.lastName}`,
    shippingAddress: '123 Main St, San Francisco, CA 94105',
    carrier: 'ups',
    shippingMethod: 'ground',
    trackingNumber: '1Z999AA10123456784',
    recipientEmail: customer.email,
    recipientPhone: '+1-555-0123'
  });

  console.log(`    Shipment: ${shipment.shipmentNumber}`);
  console.log(`      Order: ${shipment.orderId.slice(0, 8)}...`);
  console.log(`      Status: ${shipment.status}`);
  console.log(`      Carrier: ${shipment.carrier}`);
  console.log(`      Method: ${shipment.shippingMethod}`);
  console.log(`      Recipient: ${shipment.recipientName}`);
  console.log(`      Address: ${shipment.shippingAddress}\n`);

  // ============================================
  // 5. Shipment Lifecycle
  // ============================================

  console.log('[5] Shipment Lifecycle...\n');

  // Ship the shipment
  const shippedShipment = await commerce.shipments.ship(shipment.id);
  console.log(`    Shipped: ${shippedShipment.shipmentNumber}`);
  console.log(`      Status: ${shippedShipment.status}`);
  console.log(`      Tracking: ${shippedShipment.trackingNumber}`);

  // Update tracking number
  const updatedShipment = await commerce.shipments.ship(shipment.id, '1Z999AA10123456785');
  console.log(`      Updated tracking: ${updatedShipment.trackingNumber}`);

  // Mark as delivered
  const deliveredShipment = await commerce.shipments.deliver(shipment.id);
  console.log(`    Delivered: ${deliveredShipment.status}\n`);

  // ============================================
  // 6. Multiple Shipments (Split Shipment)
  // ============================================

  console.log('[6] Split Shipment Example...\n');

  // Create a second shipment for the same order
  const shipment2 = await commerce.shipments.create({
    orderId: order.id,
    recipientName: `${customer.firstName} ${customer.lastName}`,
    shippingAddress: '456 Office Blvd, San Francisco, CA 94110',
    carrier: 'fedex',
    shippingMethod: 'express',
    trackingNumber: 'FDX123456789'
  });

  console.log(`    Second shipment: ${shipment2.shipmentNumber}`);
  console.log(`      Carrier: ${shipment2.carrier} (${shipment2.shippingMethod})`);

  // List all shipments
  const allShipments = await commerce.shipments.list();
  console.log(`\n    Total shipments: ${allShipments.length}`);

  const shipmentCount = await commerce.shipments.count();
  console.log(`    Shipment count: ${shipmentCount}\n`);

  // ============================================
  // 7. Cancel Shipment
  // ============================================

  console.log('[7] Cancelling Shipment...\n');

  const cancelledShipment = await commerce.shipments.cancel(shipment2.id);
  console.log(`    Cancelled: ${cancelledShipment.shipmentNumber}`);
  console.log(`      Status: ${cancelledShipment.status}\n`);

  // ============================================
  // 8. Returns Management
  // ============================================

  console.log('[8] Returns Management...\n');

  // Create a return
  const orderItems = order.items;
  const returnRequest = await commerce.returns.create({
    orderId: order.id,
    reason: 'defective',
    reasonDetails: 'Product arrived damaged in shipping',
    items: [
      { orderItemId: orderItems[0].id, quantity: 1 }
    ]
  });

  console.log(`    Return created: ${returnRequest.id.slice(0, 8)}...`);
  console.log(`      Order: ${returnRequest.orderId.slice(0, 8)}...`);
  console.log(`      Reason: ${returnRequest.reason}`);
  console.log(`      Status: ${returnRequest.status}`);

  // Approve the return
  const approvedReturn = await commerce.returns.approve(returnRequest.id);
  console.log(`    Approved: ${approvedReturn.status}`);

  // Create another return to reject
  const returnToReject = await commerce.returns.create({
    orderId: order.id,
    reason: 'changed_mind',
    reasonDetails: 'Customer changed their mind',
    items: [{ orderItemId: orderItems[0].id, quantity: 1 }]
  });

  const rejectedReturn = await commerce.returns.reject(returnToReject.id, 'Return window expired');
  console.log(`    Rejected return: ${rejectedReturn.status}`);

  // List returns
  const allReturns = await commerce.returns.list();
  console.log(`\n    Total returns: ${allReturns.length}`);

  const returnCount = await commerce.returns.count();
  console.log(`    Return count: ${returnCount}\n`);

  // ============================================
  // 9. Warranty Management
  // ============================================

  console.log('[9] Warranty Management...\n');

  // Create warranty
  const warranty = await commerce.warranties.create({
    customerId: customer.id,
    productId: product.id,
    orderId: order.id,
    warrantyType: 'extended',
    durationMonths: 36,
    serialNumber: 'SN-2024-123456'
  });

  console.log(`    Warranty: ${warranty.warrantyNumber}`);
  console.log(`      Type: ${warranty.warrantyType}`);
  console.log(`      Serial: ${warranty.serialNumber}`);
  console.log(`      Start: ${warranty.startDate}`);
  console.log(`      End: ${warranty.endDate}`);
  console.log(`      Status: ${warranty.status}`);

  // Create warranty claim
  const claim = await commerce.warranties.createClaim({
    warrantyId: warranty.id,
    issueDescription: 'Product stopped working after normal use',
    contactEmail: customer.email,
    contactPhone: '+1-555-0123'
  });

  console.log(`\n    Warranty Claim: ${claim.claimNumber}`);
  console.log(`      Issue: ${claim.issueDescription}`);
  console.log(`      Status: ${claim.status}`);

  // Approve claim
  const approvedClaim = await commerce.warranties.approveClaim(claim.id);
  console.log(`    Claim approved: ${approvedClaim.status}`);

  // Complete claim
  const completedClaim = await commerce.warranties.completeClaim(claim.id, 'Replaced with new unit');
  console.log(`    Claim completed: ${completedClaim.resolution}`);

  // Create another claim to deny
  const claimToDeny = await commerce.warranties.createClaim({
    warrantyId: warranty.id,
    issueDescription: 'Physical damage from drop'
  });

  const deniedClaim = await commerce.warranties.denyClaim(claimToDeny.id, 'Physical damage not covered');
  console.log(`    Claim denied: ${deniedClaim.status}`);

  // List warranties
  const allWarranties = await commerce.warranties.list();
  console.log(`\n    Total warranties: ${allWarranties.length}`);

  const warrantyCount = await commerce.warranties.count();
  console.log(`    Warranty count: ${warrantyCount}\n`);

  console.log('=== Payments & Fulfillment Example Complete ===');
}

main().catch(console.error);
