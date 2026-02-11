/**
 * Test Fixtures
 *
 * Standard test data objects for consistent testing across tool modules.
 */

export const testCustomer = {
  email: 'alice@example.com',
  firstName: 'Alice',
  lastName: 'Smith',
  phone: '555-0100',
  acceptsMarketing: false,
};

export const testCustomer2 = {
  email: 'bob@example.com',
  firstName: 'Bob',
  lastName: 'Jones',
  phone: '555-0200',
  acceptsMarketing: true,
};

export const testOrder = {
  customerId: 'cust_test_001',
  status: 'pending',
  lineItems: [{ productId: 'prod_test_001', sku: 'WIDGET-001', quantity: 2, price: 29.99 }],
  total: 59.98,
  currency: 'USD',
};

export const testProduct = {
  name: 'Test Widget',
  sku: 'WIDGET-001',
  price: 29.99,
  currency: 'USD',
  description: 'A test widget for unit testing',
};

export const testProduct2 = {
  name: 'Premium Gadget',
  sku: 'GADGET-002',
  price: 99.99,
  currency: 'USD',
  description: 'A premium gadget',
};

export const testInventoryItem = {
  sku: 'WIDGET-001',
  quantity: 100,
  warehouseId: 'wh_default',
};

export const testReturn = {
  orderId: 'ord_test_001',
  reason: 'defective',
  items: [{ lineItemId: 'li_001', quantity: 1 }],
};

export const testCart = {
  customerEmail: 'alice@example.com',
  currency: 'USD',
};

export const testCartItem = {
  productId: 'prod_test_001',
  sku: 'WIDGET-001',
  quantity: 2,
  price: 29.99,
};

export const testShippingAddress = {
  street: '123 Main St',
  city: 'Anytown',
  state: 'CA',
  zip: '90210',
  country: 'US',
};

export const testPayment = {
  orderId: 'ord_test_001',
  amount: 59.98,
  currency: 'USD',
  method: 'credit_card',
};

export const testShipment = {
  orderId: 'ord_test_001',
  carrier: 'FedEx',
  trackingNumber: 'FEDEX123456',
};

export const testSupplier = {
  name: 'Widget Supply Co',
  email: 'orders@widgetsupply.com',
  phone: '555-0300',
};

export const testPurchaseOrder = {
  supplierId: 'sup_test_001',
  items: [{ sku: 'WIDGET-001', quantity: 500, unitPrice: 10.0 }],
};

export const testInvoice = {
  customerId: 'cust_test_001',
  items: [{ description: 'Widget Order', quantity: 10, unitPrice: 29.99 }],
  dueDate: '2026-03-01',
};

export const testWarranty = {
  productId: 'prod_test_001',
  durationMonths: 12,
  type: 'standard',
};

export const testPromotion = {
  name: 'Summer Sale',
  type: 'percentage_off',
  value: 20,
  startDate: '2026-06-01',
  endDate: '2026-08-31',
};

export const testCoupon = {
  code: 'SAVE20',
  promotionId: 'promo_test_001',
  maxUses: 100,
};

export const testSubscriptionPlan = {
  name: 'Coffee Club Monthly',
  price: 29.99,
  currency: 'USD',
  interval: 'month',
  trialDays: 14,
};

export const testBom = {
  name: 'Widget Assembly',
  productId: 'prod_test_001',
  components: [{ sku: 'PART-001', quantity: 2 }],
};

export const testWorkOrder = {
  bomId: 'bom_test_001',
  quantity: 100,
};
