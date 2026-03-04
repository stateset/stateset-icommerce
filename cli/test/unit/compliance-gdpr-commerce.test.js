/**
 * Unit tests for GDPR erasure of main commerce tables.
 *
 * Covers: customer PII anonymization, address deletion, payment method deletion,
 * order/cart/payment/invoice/shipment/subscription/warranty anonymization,
 * GDPR export with commerce data, keepTransactions mode, and edge cases.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';
import { createComplianceService } from '../../src/compliance/exports.js';

// ---------------------------------------------------------------------------
// A2A schema (minimal — just enough for the compliance service to initialize)
// ---------------------------------------------------------------------------

const A2A_SCHEMA = `
CREATE TABLE IF NOT EXISTS a2a_payments (
  id TEXT PRIMARY KEY,
  status TEXT NOT NULL DEFAULT 'pending',
  sender_address TEXT NOT NULL,
  recipient_address TEXT NOT NULL,
  amount INTEGER NOT NULL,
  amount_decimal REAL NOT NULL,
  asset TEXT NOT NULL DEFAULT 'USDC',
  network TEXT NOT NULL DEFAULT 'set_chain',
  memo TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT
);

CREATE TABLE IF NOT EXISTS a2a_disputes (
  id TEXT PRIMARY KEY,
  status TEXT NOT NULL DEFAULT 'filed',
  escrow_id TEXT NOT NULL,
  filed_by TEXT NOT NULL,
  filed_against TEXT NOT NULL,
  reason TEXT NOT NULL,
  category TEXT NOT NULL DEFAULT 'non_delivery',
  amount_disputed INTEGER NOT NULL,
  amount_decimal REAL NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  resolved_at TEXT
);

CREATE TABLE IF NOT EXISTS a2a_circuit_breaker_events (
  id TEXT PRIMARY KEY,
  agent_name TEXT NOT NULL,
  event_type TEXT NOT NULL,
  reason TEXT,
  amount REAL,
  state_before TEXT,
  state_after TEXT,
  metadata TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS a2a_spending_ledger (
  id TEXT PRIMARY KEY,
  agent_name TEXT NOT NULL,
  amount REAL NOT NULL,
  success INTEGER NOT NULL DEFAULT 1,
  error TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS agent_cards (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  wallet_address TEXT UNIQUE NOT NULL,
  description TEXT,
  trust_level TEXT DEFAULT 'sandbox',
  active INTEGER DEFAULT 1,
  created_at TEXT,
  updated_at TEXT
);

CREATE TABLE IF NOT EXISTS a2a_notification_log (
  id TEXT PRIMARY KEY,
  recipient_address TEXT NOT NULL,
  endpoint_url TEXT NOT NULL DEFAULT '',
  event_type TEXT NOT NULL,
  payload TEXT NOT NULL DEFAULT '{}',
  status TEXT NOT NULL DEFAULT 'pending',
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS a2a_sla_violations (
  id TEXT PRIMARY KEY,
  sla_id TEXT NOT NULL,
  service_id TEXT NOT NULL,
  metric TEXT NOT NULL DEFAULT 'latency',
  severity TEXT NOT NULL DEFAULT 'warning',
  created_at TEXT NOT NULL
);
`;

// ---------------------------------------------------------------------------
// Commerce schema — mirrors main commerce database tables
// ---------------------------------------------------------------------------

const COMMERCE_SCHEMA = `
CREATE TABLE IF NOT EXISTS customers (
  id TEXT PRIMARY KEY,
  email TEXT NOT NULL UNIQUE,
  first_name TEXT NOT NULL,
  last_name TEXT NOT NULL,
  phone TEXT,
  status TEXT NOT NULL DEFAULT 'active',
  accepts_marketing INTEGER NOT NULL DEFAULT 0,
  email_verified INTEGER NOT NULL DEFAULT 0,
  tags TEXT NOT NULL DEFAULT '[]',
  metadata TEXT,
  default_shipping_address_id TEXT,
  default_billing_address_id TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS customer_addresses (
  id TEXT PRIMARY KEY,
  customer_id TEXT NOT NULL REFERENCES customers(id),
  address_type TEXT NOT NULL DEFAULT 'both',
  first_name TEXT NOT NULL,
  last_name TEXT NOT NULL,
  company TEXT,
  line1 TEXT NOT NULL,
  line2 TEXT,
  city TEXT NOT NULL,
  state TEXT,
  postal_code TEXT NOT NULL,
  country TEXT NOT NULL,
  phone TEXT,
  is_default INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS orders (
  id TEXT PRIMARY KEY,
  order_number TEXT NOT NULL UNIQUE,
  customer_id TEXT NOT NULL REFERENCES customers(id),
  status TEXT NOT NULL DEFAULT 'pending',
  total_amount TEXT NOT NULL DEFAULT '0',
  currency TEXT NOT NULL DEFAULT 'USD',
  payment_status TEXT NOT NULL DEFAULT 'pending',
  fulfillment_status TEXT NOT NULL DEFAULT 'unfulfilled',
  shipping_address TEXT,
  billing_address TEXT,
  notes TEXT,
  tracking_number TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS carts (
  id TEXT PRIMARY KEY,
  cart_number TEXT NOT NULL UNIQUE,
  customer_id TEXT,
  status TEXT NOT NULL DEFAULT 'active',
  currency TEXT NOT NULL DEFAULT 'USD',
  customer_email TEXT,
  customer_phone TEXT,
  customer_name TEXT,
  shipping_address TEXT,
  billing_address TEXT,
  notes TEXT,
  metadata TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS payments (
  id TEXT PRIMARY KEY,
  payment_number TEXT UNIQUE NOT NULL,
  order_id TEXT,
  customer_id TEXT,
  status TEXT NOT NULL DEFAULT 'pending',
  payment_method TEXT NOT NULL DEFAULT 'credit_card',
  amount TEXT NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  billing_email TEXT,
  billing_name TEXT,
  billing_address TEXT,
  card_brand TEXT,
  card_last4 TEXT,
  card_exp_month INTEGER,
  card_exp_year INTEGER,
  description TEXT,
  metadata TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS payment_methods (
  id TEXT PRIMARY KEY,
  customer_id TEXT NOT NULL,
  method_type TEXT NOT NULL DEFAULT 'credit_card',
  is_default INTEGER NOT NULL DEFAULT 0,
  card_brand TEXT,
  card_last4 TEXT,
  card_exp_month INTEGER,
  card_exp_year INTEGER,
  cardholder_name TEXT,
  billing_address TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS invoices (
  id TEXT PRIMARY KEY,
  invoice_number TEXT UNIQUE NOT NULL,
  customer_id TEXT NOT NULL,
  order_id TEXT,
  status TEXT NOT NULL DEFAULT 'draft',
  currency TEXT NOT NULL DEFAULT 'USD',
  billing_name TEXT,
  billing_email TEXT,
  billing_address TEXT,
  billing_city TEXT,
  billing_state TEXT,
  billing_postal_code TEXT,
  billing_country TEXT,
  total TEXT NOT NULL DEFAULT '0',
  notes TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS shipments (
  id TEXT PRIMARY KEY,
  shipment_number TEXT UNIQUE NOT NULL,
  order_id TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',
  recipient_name TEXT NOT NULL,
  recipient_email TEXT,
  recipient_phone TEXT,
  shipping_address TEXT NOT NULL,
  notes TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS subscriptions (
  id TEXT PRIMARY KEY,
  subscription_number TEXT NOT NULL UNIQUE,
  customer_id TEXT NOT NULL,
  plan_id TEXT NOT NULL,
  plan_name TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',
  price TEXT NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  shipping_address TEXT,
  billing_address TEXT,
  metadata TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS warranties (
  id TEXT PRIMARY KEY,
  warranty_number TEXT UNIQUE NOT NULL,
  customer_id TEXT NOT NULL,
  product_id TEXT,
  status TEXT NOT NULL DEFAULT 'active',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS warranty_claims (
  id TEXT PRIMARY KEY,
  claim_number TEXT UNIQUE NOT NULL,
  warranty_id TEXT NOT NULL,
  customer_id TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'submitted',
  issue_description TEXT NOT NULL,
  contact_phone TEXT,
  contact_email TEXT,
  shipping_address TEXT,
  customer_notes TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
`;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeA2AStore() {
  const db = new Database(':memory:');
  db.pragma('journal_mode = WAL');
  db.exec(A2A_SCHEMA);
  return { db };
}

function makeCommerceDb() {
  const db = new Database(':memory:');
  db.pragma('journal_mode = WAL');
  db.exec(COMMERCE_SCHEMA);
  return db;
}

const CUSTOMER_ID = 'cust-001';
const CUSTOMER_EMAIL = 'alice@example.com';

function seedCommerceData(cdb) {
  const now = new Date().toISOString();

  // Customer
  cdb.prepare(
    `INSERT INTO customers (id, email, first_name, last_name, phone, metadata, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
  ).run(CUSTOMER_ID, CUSTOMER_EMAIL, 'Alice', 'Johnson', '+1-555-0123', '{"vip":true}', now, now);

  // Addresses
  cdb.prepare(
    `INSERT INTO customer_addresses (id, customer_id, first_name, last_name, company, line1, city, state, postal_code, country, phone, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
  ).run('addr-1', CUSTOMER_ID, 'Alice', 'Johnson', 'Acme Inc', '123 Main St', 'Springfield', 'IL', '62701', 'US', '+1-555-0123', now, now);

  cdb.prepare(
    `INSERT INTO customer_addresses (id, customer_id, first_name, last_name, line1, city, postal_code, country, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
  ).run('addr-2', CUSTOMER_ID, 'Alice', 'Johnson', '456 Oak Ave', 'Chicago', '60601', 'US', now, now);

  // Orders
  const shippingAddr = JSON.stringify({
    first_name: 'Alice', last_name: 'Johnson', line1: '123 Main St',
    city: 'Springfield', state: 'IL', postal_code: '62701', country: 'US',
  });
  cdb.prepare(
    `INSERT INTO orders (id, order_number, customer_id, total_amount, shipping_address, billing_address, notes, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
  ).run('ord-1', 'ORD-001', CUSTOMER_ID, '99.99', shippingAddr, shippingAddr, 'Gift wrap please', now, now);

  cdb.prepare(
    `INSERT INTO orders (id, order_number, customer_id, total_amount, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?)`,
  ).run('ord-2', 'ORD-002', CUSTOMER_ID, '49.50', now, now);

  // Carts
  cdb.prepare(
    `INSERT INTO carts (id, cart_number, customer_id, customer_email, customer_phone, customer_name, shipping_address, notes, metadata, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
  ).run('cart-1', 'CART-001', CUSTOMER_ID, CUSTOMER_EMAIL, '+1-555-0123', 'Alice Johnson', shippingAddr, 'Rush order', '{"source":"web"}', now, now);

  // Payments
  cdb.prepare(
    `INSERT INTO payments (id, payment_number, customer_id, amount, billing_email, billing_name, billing_address, card_brand, card_last4, card_exp_month, card_exp_year, description, metadata, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
  ).run('pay-1', 'PAY-001', CUSTOMER_ID, '99.99', CUSTOMER_EMAIL, 'Alice Johnson', shippingAddr, 'visa', '4242', 12, 2028, 'Order payment', '{"ip":"192.168.1.1"}', now, now);

  // Payment methods
  cdb.prepare(
    `INSERT INTO payment_methods (id, customer_id, card_brand, card_last4, card_exp_month, card_exp_year, cardholder_name, billing_address, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
  ).run('pm-1', CUSTOMER_ID, 'visa', '4242', 12, 2028, 'Alice Johnson', shippingAddr, now, now);

  cdb.prepare(
    `INSERT INTO payment_methods (id, customer_id, card_brand, card_last4, cardholder_name, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, ?)`,
  ).run('pm-2', CUSTOMER_ID, 'mastercard', '5555', 'Alice J', now, now);

  // Invoices
  cdb.prepare(
    `INSERT INTO invoices (id, invoice_number, customer_id, billing_name, billing_email, billing_address, billing_city, billing_state, billing_postal_code, billing_country, total, notes, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
  ).run('inv-1', 'INV-001', CUSTOMER_ID, 'Alice Johnson', CUSTOMER_EMAIL, '123 Main St', 'Springfield', 'IL', '62701', 'US', '99.99', 'Net 30', now, now);

  // Shipments
  cdb.prepare(
    `INSERT INTO shipments (id, shipment_number, order_id, recipient_name, recipient_email, recipient_phone, shipping_address, notes, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
  ).run('ship-1', 'SHIP-001', 'ord-1', 'Alice Johnson', CUSTOMER_EMAIL, '+1-555-0123', shippingAddr, 'Handle with care', now, now);

  // Subscriptions
  cdb.prepare(
    `INSERT INTO subscriptions (id, subscription_number, customer_id, plan_id, plan_name, price, shipping_address, billing_address, metadata, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
  ).run('sub-1', 'SUB-001', CUSTOMER_ID, 'plan-1', 'Monthly Coffee', '29.99', shippingAddr, shippingAddr, '{"size":"large"}', now, now);

  // Warranties
  cdb.prepare(
    `INSERT INTO warranties (id, warranty_number, customer_id, product_id, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?)`,
  ).run('war-1', 'WAR-001', CUSTOMER_ID, 'prod-1', now, now);

  // Warranty claims
  cdb.prepare(
    `INSERT INTO warranty_claims (id, claim_number, warranty_id, customer_id, issue_description, contact_phone, contact_email, shipping_address, customer_notes, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
  ).run('claim-1', 'CLM-001', 'war-1', CUSTOMER_ID, 'Screen cracked', '+1-555-0123', CUSTOMER_EMAIL, shippingAddr, 'Happened during shipping', now, now);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('GDPR Commerce Erasure', () => {
  /** @type {ReturnType<typeof createComplianceService>} */
  let svc;
  let a2aStore;
  let commerceDb;

  beforeEach(() => {
    a2aStore = makeA2AStore();
    commerceDb = makeCommerceDb();
    seedCommerceData(commerceDb);
    svc = createComplianceService(a2aStore, { _commerceDbOverride: commerceDb });
  });

  afterEach(() => {
    svc.close();
    commerceDb.close();
    a2aStore.db.close();
  });

  describe('deleteGDPRData — commerce tables', () => {
    it('anonymizes customer profile (keepTransactions=true)', () => {
      const result = svc.deleteGDPRData(CUSTOMER_ID, { keepTransactions: true });

      // Verify customer was anonymized
      const customer = commerceDb.prepare('SELECT * FROM customers WHERE id = ?').get(CUSTOMER_ID);
      assert.ok(customer, 'customer row should still exist');
      assert.equal(customer.status, 'deleted');
      assert.equal(customer.first_name, '[REDACTED]');
      assert.equal(customer.last_name, '[REDACTED]');
      assert.ok(customer.email.endsWith('@redacted.invalid'), 'email should be anonymized');
      assert.equal(customer.phone, null);
      assert.equal(customer.metadata, null);

      // Check retained entry
      const custRetained = result.retained.find((r) => r.table === 'customers');
      assert.ok(custRetained, 'customers should appear in retained');
      assert.equal(custRetained.action, 'anonymized');
    });

    it('anonymizes customer profile (keepTransactions=false)', () => {
      const result = svc.deleteGDPRData(CUSTOMER_ID, { keepTransactions: false });

      const customer = commerceDb.prepare('SELECT * FROM customers WHERE id = ?').get(CUSTOMER_ID);
      assert.ok(customer, 'customer row should still exist (soft-delete)');
      assert.equal(customer.status, 'deleted');
      assert.equal(customer.first_name, '[REDACTED]');

      const custRetained = result.retained.find((r) => r.table === 'customers');
      assert.ok(custRetained);
      assert.ok(custRetained.action.includes('soft-deleted'));
    });

    it('deletes all customer addresses', () => {
      const result = svc.deleteGDPRData(CUSTOMER_ID);

      const addrs = commerceDb.prepare('SELECT * FROM customer_addresses WHERE customer_id = ?').all(CUSTOMER_ID);
      assert.equal(addrs.length, 0, 'all addresses should be deleted');

      const addrDeleted = result.deleted.find((d) => d.table === 'customer_addresses');
      assert.ok(addrDeleted);
      assert.equal(addrDeleted.count, 2);
    });

    it('deletes all payment methods', () => {
      const result = svc.deleteGDPRData(CUSTOMER_ID);

      const pms = commerceDb.prepare('SELECT * FROM payment_methods WHERE customer_id = ?').all(CUSTOMER_ID);
      assert.equal(pms.length, 0, 'all payment methods should be deleted');

      const pmDeleted = result.deleted.find((d) => d.table === 'payment_methods');
      assert.ok(pmDeleted);
      assert.equal(pmDeleted.count, 2);
    });

    it('anonymizes order shipping/billing addresses', () => {
      svc.deleteGDPRData(CUSTOMER_ID);

      const orders = commerceDb.prepare('SELECT * FROM orders WHERE customer_id = ?').all(CUSTOMER_ID);
      assert.equal(orders.length, 2, 'orders should still exist');

      // Order with addresses should be anonymized
      const ord1 = orders.find((o) => o.id === 'ord-1');
      const addr = JSON.parse(ord1.shipping_address);
      assert.equal(addr.first_name, '[REDACTED]');
      assert.equal(addr.last_name, '[REDACTED]');
      assert.equal(ord1.notes, null, 'notes should be cleared');
    });

    it('anonymizes cart PII', () => {
      svc.deleteGDPRData(CUSTOMER_ID);

      const carts = commerceDb.prepare('SELECT * FROM carts WHERE customer_id = ?').all(CUSTOMER_ID);
      assert.equal(carts.length, 1, 'cart should still exist');
      assert.equal(carts[0].customer_email, null);
      assert.equal(carts[0].customer_phone, null);
      assert.equal(carts[0].customer_name, null);
      assert.equal(carts[0].shipping_address, null);
      assert.equal(carts[0].notes, null);
      assert.equal(carts[0].metadata, null);
    });

    it('anonymizes payment billing PII', () => {
      svc.deleteGDPRData(CUSTOMER_ID);

      const payments = commerceDb.prepare('SELECT * FROM payments WHERE customer_id = ?').all(CUSTOMER_ID);
      assert.equal(payments.length, 1, 'payment record should still exist');
      assert.equal(payments[0].billing_email, null);
      assert.equal(payments[0].billing_name, null);
      assert.equal(payments[0].billing_address, null);
      assert.equal(payments[0].card_last4, null);
      assert.equal(payments[0].card_brand, null);
      assert.equal(payments[0].card_exp_month, null);
      assert.equal(payments[0].card_exp_year, null);
      assert.equal(payments[0].description, null);
      assert.equal(payments[0].metadata, null);
      // Amount should be preserved for accounting
      assert.equal(payments[0].amount, '99.99');
    });

    it('anonymizes invoice billing PII', () => {
      svc.deleteGDPRData(CUSTOMER_ID);

      const invoices = commerceDb.prepare('SELECT * FROM invoices WHERE customer_id = ?').all(CUSTOMER_ID);
      assert.equal(invoices.length, 1);
      assert.equal(invoices[0].billing_name, null);
      assert.equal(invoices[0].billing_email, null);
      assert.equal(invoices[0].billing_address, null);
      assert.equal(invoices[0].billing_city, null);
      assert.equal(invoices[0].billing_state, null);
      assert.equal(invoices[0].billing_postal_code, null);
      assert.equal(invoices[0].billing_country, null);
      assert.equal(invoices[0].notes, null);
      // Totals preserved for accounting
      assert.equal(invoices[0].total, '99.99');
    });

    it('anonymizes shipment recipient PII', () => {
      svc.deleteGDPRData(CUSTOMER_ID);

      const shipments = commerceDb.prepare('SELECT * FROM shipments').all();
      assert.equal(shipments.length, 1);
      assert.equal(shipments[0].recipient_name, '[REDACTED]');
      assert.equal(shipments[0].recipient_email, null);
      assert.equal(shipments[0].recipient_phone, null);
      assert.equal(shipments[0].notes, null);
      const addr = JSON.parse(shipments[0].shipping_address);
      assert.equal(addr.first_name, '[REDACTED]');
    });

    it('anonymizes subscription addresses', () => {
      svc.deleteGDPRData(CUSTOMER_ID);

      const subs = commerceDb.prepare('SELECT * FROM subscriptions WHERE customer_id = ?').all(CUSTOMER_ID);
      assert.equal(subs.length, 1);
      assert.equal(subs[0].shipping_address, null);
      assert.equal(subs[0].billing_address, null);
      assert.equal(subs[0].metadata, null);
      // Plan info preserved
      assert.equal(subs[0].plan_name, 'Monthly Coffee');
    });

    it('anonymizes warranty claim contact PII', () => {
      svc.deleteGDPRData(CUSTOMER_ID);

      const claims = commerceDb.prepare('SELECT * FROM warranty_claims WHERE customer_id = ?').all(CUSTOMER_ID);
      assert.equal(claims.length, 1);
      assert.equal(claims[0].contact_phone, null);
      assert.equal(claims[0].contact_email, null);
      assert.equal(claims[0].shipping_address, null);
      assert.equal(claims[0].customer_notes, null);
      // Issue description preserved for record keeping
      assert.equal(claims[0].issue_description, 'Screen cracked');
    });

    it('handles customer lookup by email', () => {
      const result = svc.deleteGDPRData(CUSTOMER_EMAIL, { keepTransactions: true });

      const customer = commerceDb.prepare('SELECT * FROM customers WHERE id = ?').get(CUSTOMER_ID);
      assert.equal(customer.status, 'deleted');
      assert.equal(customer.first_name, '[REDACTED]');

      // Addresses should be deleted
      const addrs = commerceDb.prepare('SELECT * FROM customer_addresses WHERE customer_id = ?').all(CUSTOMER_ID);
      assert.equal(addrs.length, 0);
    });

    it('handles cart lookup by email', () => {
      // Add a cart linked by email only (no customer_id)
      const now = new Date().toISOString();
      commerceDb.prepare(
        `INSERT INTO carts (id, cart_number, customer_email, customer_name, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)`,
      ).run('cart-email-only', 'CART-EMAIL', CUSTOMER_EMAIL, 'Alice', now, now);

      svc.deleteGDPRData(CUSTOMER_EMAIL);

      const cart = commerceDb.prepare('SELECT * FROM carts WHERE id = ?').get('cart-email-only');
      assert.equal(cart.customer_email, null);
      assert.equal(cart.customer_name, null);
    });

    it('returns complete result with all tables', () => {
      const result = svc.deleteGDPRData(CUSTOMER_ID);

      assert.ok(result.deleted.length > 0, 'should have deleted entries');
      assert.ok(result.retained.length > 0, 'should have retained entries');
      assert.ok(result.deletedAt, 'should have deletedAt timestamp');

      // Verify all expected tables appear
      const allTables = [
        ...result.deleted.map((d) => d.table),
        ...result.retained.map((r) => r.table),
      ];
      assert.ok(allTables.includes('customer_addresses'));
      assert.ok(allTables.includes('payment_methods'));
      assert.ok(allTables.includes('customers'));
      assert.ok(allTables.includes('orders'));
      assert.ok(allTables.includes('carts'));
      assert.ok(allTables.includes('payments'));
      assert.ok(allTables.includes('invoices'));
      assert.ok(allTables.includes('shipments'));
      assert.ok(allTables.includes('subscriptions'));
      assert.ok(allTables.includes('warranty_claims'));
    });

    it('is idempotent — second call does not fail', () => {
      svc.deleteGDPRData(CUSTOMER_ID);
      const result2 = svc.deleteGDPRData(CUSTOMER_ID);
      // Second call should succeed with zero changes
      assert.ok(result2.deletedAt);
    });
  });

  describe('generateGDPRExport — commerce data', () => {
    it('includes customer profile in export', () => {
      const result = svc.generateGDPRExport(CUSTOMER_ID);

      assert.ok(result.commerceData, 'should have commerceData');
      assert.ok(result.commerceData.customers);
      assert.equal(result.commerceData.customers.length, 1);
      assert.equal(result.commerceData.customers[0].email, CUSTOMER_EMAIL);
    });

    it('includes addresses in export', () => {
      const result = svc.generateGDPRExport(CUSTOMER_ID);

      assert.ok(result.commerceData.addresses);
      assert.equal(result.commerceData.addresses.length, 2);
    });

    it('includes orders in export', () => {
      const result = svc.generateGDPRExport(CUSTOMER_ID);

      assert.ok(result.commerceData.orders);
      assert.equal(result.commerceData.orders.length, 2);
    });

    it('includes carts in export', () => {
      const result = svc.generateGDPRExport(CUSTOMER_ID);

      assert.ok(result.commerceData.carts);
      assert.equal(result.commerceData.carts.length, 1);
    });

    it('includes payments in export', () => {
      const result = svc.generateGDPRExport(CUSTOMER_ID);

      assert.ok(result.commerceData.commercePayments);
      assert.equal(result.commerceData.commercePayments.length, 1);
    });

    it('includes payment methods in export', () => {
      const result = svc.generateGDPRExport(CUSTOMER_ID);

      assert.ok(result.commerceData.paymentMethods);
      assert.equal(result.commerceData.paymentMethods.length, 2);
    });

    it('includes invoices in export', () => {
      const result = svc.generateGDPRExport(CUSTOMER_ID);

      assert.ok(result.commerceData.invoices);
      assert.equal(result.commerceData.invoices.length, 1);
    });

    it('includes subscriptions in export', () => {
      const result = svc.generateGDPRExport(CUSTOMER_ID);

      assert.ok(result.commerceData.subscriptions);
      assert.equal(result.commerceData.subscriptions.length, 1);
    });

    it('includes warranties in export', () => {
      const result = svc.generateGDPRExport(CUSTOMER_ID);

      assert.ok(result.commerceData.warranties);
      assert.equal(result.commerceData.warranties.length, 1);
    });

    it('includes warranty claims in export', () => {
      const result = svc.generateGDPRExport(CUSTOMER_ID);

      assert.ok(result.commerceData.warrantyClaims);
      assert.equal(result.commerceData.warrantyClaims.length, 1);
    });

    it('includes shipments in export', () => {
      const result = svc.generateGDPRExport(CUSTOMER_ID);

      assert.ok(result.commerceData.shipments);
      assert.equal(result.commerceData.shipments.length, 1);
    });

    it('finds customer by email', () => {
      const result = svc.generateGDPRExport(CUSTOMER_EMAIL);

      assert.ok(result.commerceData);
      assert.ok(result.commerceData.customers);
      assert.equal(result.commerceData.customers.length, 1);
      assert.equal(result.commerceData.customers[0].id, CUSTOMER_ID);
    });

    it('returns no commerceData for unknown customer', () => {
      const result = svc.generateGDPRExport('unknown-customer');

      // Commerce data should be empty or undefined
      if (result.commerceData) {
        const totalRecords = Object.values(result.commerceData).reduce(
          (sum, arr) => sum + (Array.isArray(arr) ? arr.length : 0),
          0,
        );
        assert.equal(totalRecords, 0);
      }
    });
  });

  describe('edge cases', () => {
    it('works without commerce database', () => {
      const svcNoCommerce = createComplianceService(a2aStore);
      const result = svcNoCommerce.deleteGDPRData('test-customer');
      assert.ok(result.deletedAt);
      // Should only have A2A results
      svcNoCommerce.close();
    });

    it('handles missing commerce tables gracefully', () => {
      const emptyDb = new Database(':memory:');
      emptyDb.pragma('journal_mode = WAL');
      // No tables created
      const svcEmpty = createComplianceService(a2aStore, { _commerceDbOverride: emptyDb });
      const result = svcEmpty.deleteGDPRData(CUSTOMER_ID);
      assert.ok(result.deletedAt);
      svcEmpty.close();
      emptyDb.close();
    });

    it('handles customer with no related records', () => {
      // Create a customer with no orders, carts, payments, etc.
      const now = new Date().toISOString();
      commerceDb.prepare(
        `INSERT INTO customers (id, email, first_name, last_name, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)`,
      ).run('cust-lonely', 'lonely@example.com', 'Lonely', 'Person', now, now);

      const result = svc.deleteGDPRData('cust-lonely');
      assert.ok(result.deletedAt);
      const custRetained = result.retained.find((r) => r.table === 'customers');
      assert.ok(custRetained);
    });

    it('preserves other customers data', () => {
      // Add another customer
      const now = new Date().toISOString();
      commerceDb.prepare(
        `INSERT INTO customers (id, email, first_name, last_name, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)`,
      ).run('cust-other', 'bob@example.com', 'Bob', 'Smith', now, now);

      commerceDb.prepare(
        `INSERT INTO customer_addresses (id, customer_id, first_name, last_name, line1, city, postal_code, country, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      ).run('addr-bob', 'cust-other', 'Bob', 'Smith', '789 Elm St', 'Denver', '80201', 'US', now, now);

      // Delete Alice's data
      svc.deleteGDPRData(CUSTOMER_ID);

      // Bob's data should be untouched
      const bob = commerceDb.prepare('SELECT * FROM customers WHERE id = ?').get('cust-other');
      assert.equal(bob.first_name, 'Bob');
      assert.equal(bob.email, 'bob@example.com');

      const bobAddrs = commerceDb.prepare('SELECT * FROM customer_addresses WHERE customer_id = ?').all('cust-other');
      assert.equal(bobAddrs.length, 1);
      assert.equal(bobAddrs[0].first_name, 'Bob');
    });
  });
});

