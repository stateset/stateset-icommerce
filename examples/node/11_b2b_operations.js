#!/usr/bin/env node
/**
 * StateSet iCommerce - B2B Operations Example
 *
 * This example demonstrates B2B commerce features:
 * - Supplier management
 * - Purchase order creation and lifecycle
 * - Invoice generation and payment tracking
 * - B2B workflows
 *
 * Run with: node 11_b2b_operations.js
 */

const { Commerce } = require('@stateset/embedded');

async function main() {
  console.log('=== StateSet iCommerce - B2B Operations ===\n');

  const commerce = new Commerce(':memory:');

  // ============================================
  // Setup
  // ============================================

  console.log('[Setup] Creating test data...');

  const customer = await commerce.customers.create({
    email: 'purchasing@acme-corp.com',
    firstName: 'ACME',
    lastName: 'Corporation'
  });

  const product = await commerce.products.create({
    name: 'Industrial Widget',
    variants: [{ sku: 'IND-WIDGET-001', name: 'Standard', price: 149.99 }]
  });

  const order = await commerce.orders.create({
    customerId: customer.id,
    items: [{ sku: 'IND-WIDGET-001', name: 'Industrial Widget', quantity: 50, unitPrice: 149.99 }],
    currency: 'USD'
  });

  console.log('    Test data created\n');

  // ============================================
  // 1. Supplier Management
  // ============================================

  console.log('[1] Supplier Management...\n');

  // Create suppliers
  const supplier1 = await commerce.purchaseOrders.createSupplier({
    name: 'Global Components Inc.',
    supplierCode: 'GCI-001',
    email: 'orders@globalcomponents.com',
    phone: '+1-800-555-0100'
  });

  console.log(`    Supplier: ${supplier1.name}`);
  console.log(`      Code: ${supplier1.supplierCode}`);
  console.log(`      Email: ${supplier1.email}`);
  console.log(`      Active: ${supplier1.isActive}`);

  const supplier2 = await commerce.purchaseOrders.createSupplier({
    name: 'Precision Parts Ltd.',
    supplierCode: 'PPL-001',
    email: 'sales@precisionparts.com',
    phone: '+1-800-555-0200'
  });
  console.log(`    Supplier: ${supplier2.name}`);

  // List suppliers
  const suppliers = await commerce.purchaseOrders.listSuppliers();
  console.log(`\n    Total suppliers: ${suppliers.length}`);

  // Get supplier by ID
  const foundSupplier = await commerce.purchaseOrders.getSupplier(supplier1.id);
  console.log(`    Retrieved: ${foundSupplier.name}\n`);

  // ============================================
  // 2. Create Purchase Order
  // ============================================

  console.log('[2] Creating Purchase Order...\n');

  const purchaseOrder = await commerce.purchaseOrders.create({
    supplierId: supplier1.id,
    items: [
      { sku: 'COMP-CPU-001', name: 'CPU Module A', quantity: 100, unitCost: 85.00 },
      { sku: 'COMP-MEM-001', name: 'Memory Module 16GB', quantity: 200, unitCost: 45.00 },
      { sku: 'COMP-SSD-001', name: 'SSD 1TB', quantity: 150, unitCost: 75.00 }
    ],
    notes: 'Q1 2024 component order'
  });

  console.log(`    PO Number: ${purchaseOrder.poNumber}`);
  console.log(`      Supplier: ${supplier1.name}`);
  console.log(`      Status: ${purchaseOrder.status}`);
  console.log(`      Subtotal: $${purchaseOrder.subtotal.toFixed(2)}`);
  console.log(`      Total: $${purchaseOrder.total.toFixed(2)}\n`);

  // Create another PO
  const purchaseOrder2 = await commerce.purchaseOrders.create({
    supplierId: supplier2.id,
    items: [
      { sku: 'PART-A001', name: 'Precision Part A', quantity: 500, unitCost: 12.50 },
      { sku: 'PART-B002', name: 'Precision Part B', quantity: 300, unitCost: 18.75 }
    ]
  });
  console.log(`    PO Number: ${purchaseOrder2.poNumber} ($${purchaseOrder2.total.toFixed(2)})\n`);

  // ============================================
  // 3. Purchase Order Lifecycle
  // ============================================

  console.log('[3] Purchase Order Lifecycle...\n');

  // Submit PO
  const submittedPO = await commerce.purchaseOrders.submit(purchaseOrder.id);
  console.log(`    Submitted: ${submittedPO.poNumber}`);
  console.log(`      Status: ${submittedPO.status}`);

  // Approve PO
  const approvedPO = await commerce.purchaseOrders.approve(purchaseOrder.id, 'John Manager');
  console.log(`    Approved: ${approvedPO.poNumber}`);
  console.log(`      Status: ${approvedPO.status}`);

  // Send PO to supplier
  const sentPO = await commerce.purchaseOrders.send(purchaseOrder.id);
  console.log(`    Sent to supplier: ${sentPO.poNumber}`);
  console.log(`      Status: ${sentPO.status}\n`);

  // Cancel a PO
  const cancelledPO = await commerce.purchaseOrders.cancel(purchaseOrder2.id);
  console.log(`    Cancelled: ${cancelledPO.poNumber}`);
  console.log(`      Status: ${cancelledPO.status}\n`);

  // ============================================
  // 4. List and Query Purchase Orders
  // ============================================

  console.log('[4] Querying Purchase Orders...\n');

  // List all POs
  const allPOs = await commerce.purchaseOrders.list();
  console.log(`    Total purchase orders: ${allPOs.length}`);

  // Get specific PO
  const retrievedPO = await commerce.purchaseOrders.get(purchaseOrder.id);
  console.log(`    Retrieved: ${retrievedPO.poNumber}`);
  console.log(`      Created: ${retrievedPO.createdAt}`);
  console.log(`      Updated: ${retrievedPO.updatedAt}`);

  // Count POs
  const poCount = await commerce.purchaseOrders.count();
  console.log(`    PO count: ${poCount}\n`);

  // ============================================
  // 5. Invoice Creation
  // ============================================

  console.log('[5] Creating Invoices...\n');

  // Create invoice for order
  const invoice = await commerce.invoices.create({
    customerId: customer.id,
    orderId: order.id,
    items: [
      { description: 'Industrial Widget', sku: 'IND-WIDGET-001', quantity: 50, unitPrice: 149.99 },
      { description: 'Shipping & Handling', quantity: 1, unitPrice: 75.00 }
    ],
    billingEmail: 'ap@acme-corp.com',
    billingName: 'ACME Corporation - Accounts Payable',
    notes: 'Net 30 payment terms'
  });

  console.log(`    Invoice: ${invoice.invoiceNumber}`);
  console.log(`      Customer: ${customer.firstName} ${customer.lastName}`);
  console.log(`      Status: ${invoice.status}`);
  console.log(`      Subtotal: $${invoice.subtotal.toFixed(2)}`);
  console.log(`      Tax: $${invoice.taxAmount.toFixed(2)}`);
  console.log(`      Total: $${invoice.total.toFixed(2)}`);
  console.log(`      Due: ${invoice.dueDate}\n`);

  // Create standalone invoice (not tied to order)
  const standaloneInvoice = await commerce.invoices.create({
    customerId: customer.id,
    items: [
      { description: 'Consulting Services - Q1', quantity: 40, unitPrice: 150.00 },
      { description: 'Software License - Annual', quantity: 1, unitPrice: 2500.00 }
    ],
    billingEmail: 'ap@acme-corp.com',
    billingName: 'ACME Corporation'
  });
  console.log(`    Standalone invoice: ${standaloneInvoice.invoiceNumber} ($${standaloneInvoice.total.toFixed(2)})\n`);

  // ============================================
  // 6. Invoice Lifecycle
  // ============================================

  console.log('[6] Invoice Lifecycle...\n');

  // Send invoice
  const sentInvoice = await commerce.invoices.send(invoice.id);
  console.log(`    Sent: ${sentInvoice.invoiceNumber}`);
  console.log(`      Status: ${sentInvoice.status}`);

  // Record partial payment
  const partialPaid = await commerce.invoices.recordPayment(invoice.id, {
    amount: 4000.00,
    paymentMethod: 'wire_transfer',
    reference: 'WT-2024-001234'
  });
  console.log(`    Partial payment recorded: $4000.00`);
  console.log(`      Amount paid: $${partialPaid.amountPaid.toFixed(2)}`);
  console.log(`      Remaining: $${(partialPaid.total - partialPaid.amountPaid).toFixed(2)}`);
  console.log(`      Status: ${partialPaid.status}`);

  // Record remaining payment
  const remaining = partialPaid.total - partialPaid.amountPaid;
  const fullyPaid = await commerce.invoices.recordPayment(invoice.id, {
    amount: remaining,
    paymentMethod: 'check',
    reference: 'CHK-789012'
  });
  console.log(`    Final payment recorded: $${remaining.toFixed(2)}`);
  console.log(`      Status: ${fullyPaid.status}\n`);

  // ============================================
  // 7. Void Invoice
  // ============================================

  console.log('[7] Voiding Invoice...\n');

  // Send the standalone invoice first
  await commerce.invoices.send(standaloneInvoice.id);

  // Void it
  const voidedInvoice = await commerce.invoices.void(standaloneInvoice.id);
  console.log(`    Voided: ${voidedInvoice.invoiceNumber}`);
  console.log(`      Status: ${voidedInvoice.status}\n`);

  // ============================================
  // 8. Overdue Invoices
  // ============================================

  console.log('[8] Checking Overdue Invoices...\n');

  // Create an overdue invoice for demo
  const overdueInvoice = await commerce.invoices.create({
    customerId: customer.id,
    items: [
      { description: 'Past Due Service', quantity: 1, unitPrice: 500.00 }
    ]
  });
  await commerce.invoices.send(overdueInvoice.id);

  // Get overdue invoices
  const overdueInvoices = await commerce.invoices.getOverdue();
  console.log(`    Overdue invoices: ${overdueInvoices.length}`);

  for (const inv of overdueInvoices) {
    console.log(`      ${inv.invoiceNumber}: $${inv.total.toFixed(2)} (Due: ${inv.dueDate})`);
  }
  console.log('');

  // ============================================
  // 9. List and Query Invoices
  // ============================================

  console.log('[9] Querying Invoices...\n');

  // List all invoices
  const allInvoices = await commerce.invoices.list();
  console.log(`    Total invoices: ${allInvoices.length}`);

  // Get specific invoice
  const retrievedInvoice = await commerce.invoices.get(invoice.id);
  console.log(`    Retrieved: ${retrievedInvoice.invoiceNumber}`);

  // Count invoices
  const invoiceCount = await commerce.invoices.count();
  console.log(`    Invoice count: ${invoiceCount}`);

  // Invoice summary
  console.log('\n    Invoice Summary:');
  for (const inv of allInvoices) {
    console.log(`      ${inv.invoiceNumber}: $${inv.total.toFixed(2)} (${inv.status})`);
  }
  console.log('');

  // ============================================
  // 10. B2B Workflow Summary
  // ============================================

  console.log('[10] B2B Workflow Summary...\n');

  console.log('    Suppliers:');
  for (const sup of suppliers) {
    console.log(`      - ${sup.name} (${sup.supplierCode})`);
  }

  console.log('\n    Purchase Orders:');
  for (const po of allPOs) {
    console.log(`      - ${po.poNumber}: $${po.total.toFixed(2)} (${po.status})`);
  }

  console.log('\n    Invoices:');
  for (const inv of allInvoices) {
    const outstanding = inv.total - inv.amountPaid;
    console.log(`      - ${inv.invoiceNumber}: $${inv.total.toFixed(2)}`);
    console.log(`        Paid: $${inv.amountPaid.toFixed(2)}, Outstanding: $${outstanding.toFixed(2)}`);
    console.log(`        Status: ${inv.status}`);
  }

  console.log('\n=== B2B Operations Example Complete ===');
}

main().catch(console.error);
