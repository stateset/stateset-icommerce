/**
 * Unit tests for Express Checkout & Payment Links
 *
 * Tests cli/src/checkout/express.js:
 *   - createPaymentLink() — creation, validation, total calculation, expiry, metadata
 *   - resolvePaymentLink() — by ID, by short code, view counting, expiry detection
 *   - expressCheckout() — conversion, ID generation, status guards
 *   - agentCheckout() — agent flow, escrow IDs, metadata
 *   - getPaymentLinkStatus() — status/metrics reporting
 *   - listPaymentLinks() — filtering, pagination, ordering
 *   - revokePaymentLink() — revocation, status guards
 *   - Edge cases — large arrays, zero-price items, special characters, concurrency
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';
import { createExpressCheckout } from '../../src/checkout/express.js';

// ===========================================================================
// Helpers
// ===========================================================================

function makeStore() {
  const db = new Database(':memory:');
  db.pragma('journal_mode = WAL');
  return { db };
}

function makeSvc() {
  return createExpressCheckout(makeStore());
}

function sampleItems(n = 2) {
  const items = [];
  for (let i = 0; i < n; i++) {
    items.push({
      name: `Item ${i + 1}`,
      sku: `SKU-${String(i + 1).padStart(3, '0')}`,
      quantity: i + 1,
      unitPrice: 10.0 + i * 5,
    });
  }
  return items;
}

// ===========================================================================
// createPaymentLink
// ===========================================================================

describe('createPaymentLink', () => {
  let svc;
  beforeEach(() => { svc = makeSvc(); });

  it('creates a payment link and returns required fields', () => {
    const result = svc.createPaymentLink({ items: sampleItems() });
    assert.ok(result.linkId, 'linkId should be present');
    assert.ok(result.shortCode, 'shortCode should be present');
    assert.ok(result.url, 'url should be present');
    assert.equal(typeof result.total, 'number');
    assert.equal(result.status, 'active');
  });

  it('generates an 8-character short code', () => {
    const result = svc.createPaymentLink({ items: sampleItems() });
    assert.equal(result.shortCode.length, 8);
    assert.match(result.shortCode, /^[A-Z0-9_-]{8}$/);
  });

  it('calculates total correctly: sum(qty * unitPrice)', () => {
    const items = [
      { name: 'A', quantity: 3, unitPrice: 10 },
      { name: 'B', quantity: 2, unitPrice: 25 },
    ];
    const result = svc.createPaymentLink({ items });
    assert.equal(result.total, 80); // 3*10 + 2*25
  });

  it('handles fractional prices with rounding', () => {
    const items = [
      { name: 'A', quantity: 3, unitPrice: 1.33 },
    ];
    const result = svc.createPaymentLink({ items });
    assert.equal(result.total, 3.99);
  });

  it('sets default currency to USD', () => {
    const result = svc.createPaymentLink({ items: sampleItems() });
    const status = svc.getPaymentLinkStatus(result.linkId);
    assert.equal(status.link.currency, 'USD');
  });

  it('uses custom currency when provided', () => {
    const result = svc.createPaymentLink({ items: sampleItems(), currency: 'EUR' });
    const status = svc.getPaymentLinkStatus(result.linkId);
    assert.equal(status.link.currency, 'EUR');
  });

  it('currency is normalised to uppercase', () => {
    const result = svc.createPaymentLink({ items: sampleItems(), currency: 'eur' });
    const status = svc.getPaymentLinkStatus(result.linkId);
    assert.equal(status.link.currency, 'EUR');
  });

  it('handles expiry: sets expires_at when expiresIn given', () => {
    const before = Date.now();
    const result = svc.createPaymentLink({ items: sampleItems(), expiresIn: 3600 });
    const after = Date.now();
    assert.ok(result.expiresAt);
    const exp = new Date(result.expiresAt).getTime();
    assert.ok(exp >= before + 3600 * 1000 - 100);
    assert.ok(exp <= after + 3600 * 1000 + 100);
  });

  it('has no expiry when expiresIn is null', () => {
    const result = svc.createPaymentLink({ items: sampleItems(), expiresIn: null });
    assert.equal(result.expiresAt, null);
  });

  it('validates non-empty items array', () => {
    assert.throws(() => svc.createPaymentLink({ items: [] }), /non-empty/);
  });

  it('validates items is an array', () => {
    assert.throws(() => svc.createPaymentLink({ items: 'not-array' }), /non-empty array/);
  });

  it('validates positive quantities', () => {
    const items = [{ name: 'A', quantity: 0, unitPrice: 10 }];
    assert.throws(() => svc.createPaymentLink({ items }), /positive integer/);
  });

  it('validates non-negative unitPrice', () => {
    const items = [{ name: 'A', quantity: 1, unitPrice: -5 }];
    assert.throws(() => svc.createPaymentLink({ items }), /non-negative/);
  });

  it('returns URL containing short code', () => {
    const result = svc.createPaymentLink({ items: sampleItems() });
    assert.ok(result.url.includes(result.shortCode));
    assert.ok(result.url.startsWith('https://pay.stateset.com/l/'));
  });

  it('stores metadata as JSON', () => {
    const meta = { source: 'email', campaign: 'spring-2026' };
    const result = svc.createPaymentLink({ items: sampleItems(), metadata: meta });
    const status = svc.getPaymentLinkStatus(result.linkId);
    assert.deepEqual(status.link.metadata, meta);
  });

  it('associates customer ID', () => {
    const result = svc.createPaymentLink({ items: sampleItems(), customerId: 'cust-123' });
    const status = svc.getPaymentLinkStatus(result.linkId);
    assert.equal(status.link.customer_id, 'cust-123');
  });

  it('multiple links get unique short codes', () => {
    const codes = new Set();
    for (let i = 0; i < 20; i++) {
      const result = svc.createPaymentLink({ items: sampleItems() });
      codes.add(result.shortCode);
    }
    assert.equal(codes.size, 20);
  });

  it('items are stored as JSON and retrievable', () => {
    const items = sampleItems(3);
    const result = svc.createPaymentLink({ items });
    const status = svc.getPaymentLinkStatus(result.linkId);
    assert.deepEqual(status.link.items, items);
  });
});

// ===========================================================================
// resolvePaymentLink
// ===========================================================================

describe('resolvePaymentLink', () => {
  let svc;
  beforeEach(() => { svc = makeSvc(); });

  it('resolves by ID', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    const result = svc.resolvePaymentLink(link.linkId);
    assert.ok(result);
    assert.equal(result.link.id, link.linkId);
  });

  it('resolves by short code', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    const result = svc.resolvePaymentLink(link.shortCode);
    assert.ok(result);
    assert.equal(result.link.short_code, link.shortCode);
  });

  it('increments view count on resolve', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    svc.resolvePaymentLink(link.linkId);
    svc.resolvePaymentLink(link.linkId);
    const result = svc.resolvePaymentLink(link.linkId);
    assert.equal(result.link.views, 3);
  });

  it('detects expired link', () => {
    // Create a link that expired 1 second ago
    const link = svc.createPaymentLink({ items: sampleItems(), expiresIn: -1 });
    const result = svc.resolvePaymentLink(link.linkId);
    assert.equal(result.expired, true);
    assert.equal(result.status, 'expired');
  });

  it('returns expired=false for non-expired link', () => {
    const link = svc.createPaymentLink({ items: sampleItems(), expiresIn: 86400 });
    const result = svc.resolvePaymentLink(link.linkId);
    assert.equal(result.expired, false);
    assert.equal(result.status, 'active');
  });

  it('returns parsed items array', () => {
    const items = sampleItems(3);
    const link = svc.createPaymentLink({ items });
    const result = svc.resolvePaymentLink(link.linkId);
    assert.deepEqual(result.items, items);
  });

  it('returns null for unknown link', () => {
    const result = svc.resolvePaymentLink('nonexistent-id');
    assert.equal(result, null);
  });

  it('performs case-insensitive short code lookup', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    const lower = link.shortCode.toLowerCase();
    const result = svc.resolvePaymentLink(lower);
    assert.ok(result);
    assert.equal(result.link.id, link.linkId);
  });

  it('tracks view count across multiple resolves', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    for (let i = 0; i < 5; i++) {
      svc.resolvePaymentLink(link.linkId);
    }
    const status = svc.getPaymentLinkStatus(link.linkId);
    assert.equal(status.views, 5);
  });

  it('returns total matching the created link', () => {
    const items = [{ name: 'Widget', quantity: 5, unitPrice: 9.99 }];
    const link = svc.createPaymentLink({ items });
    const result = svc.resolvePaymentLink(link.linkId);
    assert.equal(result.total, 49.95);
  });

  it('returns no-expiry link as not expired', () => {
    const link = svc.createPaymentLink({ items: sampleItems(), expiresIn: null });
    const result = svc.resolvePaymentLink(link.linkId);
    assert.equal(result.expired, false);
  });

  it('revoked link shows revoked status', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    svc.revokePaymentLink(link.linkId);
    const result = svc.resolvePaymentLink(link.linkId);
    assert.equal(result.status, 'revoked');
  });
});

// ===========================================================================
// expressCheckout
// ===========================================================================

describe('expressCheckout', () => {
  let svc;
  beforeEach(() => { svc = makeSvc(); });

  it('converts a link and returns order/payment IDs', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    const result = svc.expressCheckout({ linkId: link.linkId });
    assert.ok(result.orderId);
    assert.ok(result.paymentId);
    assert.equal(result.shortCode, link.shortCode);
  });

  it('generates UUID-format order ID', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    const result = svc.expressCheckout({ linkId: link.linkId });
    assert.match(result.orderId, /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
  });

  it('generates UUID-format payment ID', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    const result = svc.expressCheckout({ linkId: link.linkId });
    assert.match(result.paymentId, /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
  });

  it('marks link as converted', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    svc.expressCheckout({ linkId: link.linkId });
    const status = svc.getPaymentLinkStatus(link.linkId);
    assert.equal(status.status, 'converted');
  });

  it('increments conversion count', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    svc.expressCheckout({ linkId: link.linkId });
    const status = svc.getPaymentLinkStatus(link.linkId);
    assert.equal(status.conversions, 1);
  });

  it('sets converted_at timestamp', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    const before = new Date().toISOString();
    svc.expressCheckout({ linkId: link.linkId });
    const status = svc.getPaymentLinkStatus(link.linkId);
    assert.ok(status.link.converted_at);
    assert.ok(status.link.converted_at >= before);
  });

  it('stores order_id and payment_id on the link', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    const result = svc.expressCheckout({ linkId: link.linkId });
    const status = svc.getPaymentLinkStatus(link.linkId);
    assert.equal(status.link.order_id, result.orderId);
    assert.equal(status.link.payment_id, result.paymentId);
  });

  it('associates customer ID on checkout', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    svc.expressCheckout({ linkId: link.linkId, customerId: 'cust-abc' });
    const status = svc.getPaymentLinkStatus(link.linkId);
    assert.equal(status.link.customer_id, 'cust-abc');
  });

  it('fails on expired link', () => {
    const link = svc.createPaymentLink({ items: sampleItems(), expiresIn: -1 });
    assert.throws(
      () => svc.expressCheckout({ linkId: link.linkId }),
      /expired/i,
    );
  });

  it('fails on revoked link', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    svc.revokePaymentLink(link.linkId);
    assert.throws(
      () => svc.expressCheckout({ linkId: link.linkId }),
      /revoked/i,
    );
  });

  it('fails on already-converted link', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    svc.expressCheckout({ linkId: link.linkId });
    assert.throws(
      () => svc.expressCheckout({ linkId: link.linkId }),
      /already been converted/i,
    );
  });

  it('fails when linkId is missing', () => {
    assert.throws(
      () => svc.expressCheckout({}),
      /linkId is required/,
    );
  });

  it('fails when link not found', () => {
    assert.throws(
      () => svc.expressCheckout({ linkId: 'nonexistent' }),
      /not found/i,
    );
  });
});

// ===========================================================================
// agentCheckout
// ===========================================================================

describe('agentCheckout', () => {
  let svc;
  beforeEach(() => { svc = makeSvc(); });

  it('creates and converts a link in one call', () => {
    const result = svc.agentCheckout({
      buyerAgent: 'agent-buyer-1',
      sellerAgent: 'agent-seller-1',
      items: sampleItems(),
    });
    assert.ok(result.orderId);
    assert.ok(result.escrowId);
    assert.ok(result.linkId);
  });

  it('returns a valid escrow ID (UUID)', () => {
    const result = svc.agentCheckout({
      buyerAgent: 'agent-buyer',
      sellerAgent: 'agent-seller',
      items: sampleItems(),
    });
    assert.match(result.escrowId, /^[0-9a-f]{8}-[0-9a-f]{4}/);
  });

  it('includes both agent names in link metadata', () => {
    const result = svc.agentCheckout({
      buyerAgent: 'buyer-x',
      sellerAgent: 'seller-y',
      items: sampleItems(),
    });
    const status = svc.getPaymentLinkStatus(result.linkId);
    assert.equal(status.link.metadata.buyerAgent, 'buyer-x');
    assert.equal(status.link.metadata.sellerAgent, 'seller-y');
  });

  it('calculates correct total', () => {
    const items = [
      { name: 'A', quantity: 2, unitPrice: 15 },
      { name: 'B', quantity: 1, unitPrice: 30 },
    ];
    const result = svc.agentCheckout({
      buyerAgent: 'b',
      sellerAgent: 's',
      items,
    });
    const status = svc.getPaymentLinkStatus(result.linkId);
    assert.equal(status.link.total, 60);
  });

  it('handles custom currency', () => {
    const result = svc.agentCheckout({
      buyerAgent: 'b',
      sellerAgent: 's',
      items: sampleItems(),
      currency: 'USDC',
    });
    const status = svc.getPaymentLinkStatus(result.linkId);
    assert.equal(status.link.currency, 'USDC');
  });

  it('generates all required IDs as unique values', () => {
    const result = svc.agentCheckout({
      buyerAgent: 'b',
      sellerAgent: 's',
      items: sampleItems(),
    });
    const ids = [result.orderId, result.escrowId, result.linkId];
    const unique = new Set(ids);
    assert.equal(unique.size, 3, 'orderId, escrowId, linkId should all be unique');
  });

  it('fails without buyerAgent', () => {
    assert.throws(
      () => svc.agentCheckout({ sellerAgent: 's', items: sampleItems() }),
      /buyerAgent is required/,
    );
  });

  it('fails without sellerAgent', () => {
    assert.throws(
      () => svc.agentCheckout({ buyerAgent: 'b', items: sampleItems() }),
      /sellerAgent is required/,
    );
  });
});

// ===========================================================================
// getPaymentLinkStatus
// ===========================================================================

describe('getPaymentLinkStatus', () => {
  let svc;
  beforeEach(() => { svc = makeSvc(); });

  it('returns status for active link', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    const result = svc.getPaymentLinkStatus(link.linkId);
    assert.ok(result);
    assert.equal(result.status, 'active');
    assert.equal(result.views, 0);
    assert.equal(result.conversions, 0);
  });

  it('includes views and conversions counts', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    svc.resolvePaymentLink(link.linkId);
    svc.resolvePaymentLink(link.linkId);
    svc.expressCheckout({ linkId: link.linkId });
    const result = svc.getPaymentLinkStatus(link.linkId);
    assert.equal(result.views, 2);
    assert.equal(result.conversions, 1);
  });

  it('works for converted links', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    svc.expressCheckout({ linkId: link.linkId });
    const result = svc.getPaymentLinkStatus(link.linkId);
    assert.equal(result.status, 'converted');
  });

  it('works for revoked links', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    svc.revokePaymentLink(link.linkId);
    const result = svc.getPaymentLinkStatus(link.linkId);
    assert.equal(result.status, 'revoked');
  });

  it('returns null for unknown link', () => {
    const result = svc.getPaymentLinkStatus('nonexistent');
    assert.equal(result, null);
  });
});

// ===========================================================================
// listPaymentLinks
// ===========================================================================

describe('listPaymentLinks', () => {
  let svc;
  beforeEach(() => { svc = makeSvc(); });

  it('lists all links when no filters given', () => {
    svc.createPaymentLink({ items: sampleItems() });
    svc.createPaymentLink({ items: sampleItems() });
    svc.createPaymentLink({ items: sampleItems() });
    const result = svc.listPaymentLinks();
    assert.equal(result.length, 3);
  });

  it('filters by status', () => {
    const l1 = svc.createPaymentLink({ items: sampleItems() });
    svc.createPaymentLink({ items: sampleItems() });
    svc.revokePaymentLink(l1.linkId);
    const revoked = svc.listPaymentLinks({ status: 'revoked' });
    assert.equal(revoked.length, 1);
    assert.equal(revoked[0].status, 'revoked');
  });

  it('filters by customer', () => {
    svc.createPaymentLink({ items: sampleItems(), customerId: 'alice' });
    svc.createPaymentLink({ items: sampleItems(), customerId: 'bob' });
    svc.createPaymentLink({ items: sampleItems(), customerId: 'alice' });
    const result = svc.listPaymentLinks({ customerId: 'alice' });
    assert.equal(result.length, 2);
    for (const link of result) {
      assert.equal(link.customer_id, 'alice');
    }
  });

  it('respects limit', () => {
    for (let i = 0; i < 10; i++) {
      svc.createPaymentLink({ items: sampleItems() });
    }
    const result = svc.listPaymentLinks({ limit: 3 });
    assert.equal(result.length, 3);
  });

  it('respects offset', () => {
    const ids = [];
    for (let i = 0; i < 5; i++) {
      const link = svc.createPaymentLink({ items: sampleItems() });
      ids.push(link.linkId);
    }
    const all = svc.listPaymentLinks();
    const offset = svc.listPaymentLinks({ offset: 2 });
    assert.equal(offset.length, 3);
    assert.equal(offset[0].id, all[2].id);
  });

  it('returns empty array for no matches', () => {
    svc.createPaymentLink({ items: sampleItems() });
    const result = svc.listPaymentLinks({ status: 'converted' });
    assert.equal(result.length, 0);
  });

  it('orders by created_at descending', () => {
    const l1 = svc.createPaymentLink({ items: sampleItems() });
    const l2 = svc.createPaymentLink({ items: sampleItems() });
    const l3 = svc.createPaymentLink({ items: sampleItems() });
    const result = svc.listPaymentLinks();
    // Most recent first
    assert.equal(result[0].id, l3.linkId);
    assert.equal(result[2].id, l1.linkId);
  });

  it('combines multiple filters', () => {
    svc.createPaymentLink({ items: sampleItems(), customerId: 'alice' });
    const l2 = svc.createPaymentLink({ items: sampleItems(), customerId: 'alice' });
    svc.createPaymentLink({ items: sampleItems(), customerId: 'bob' });
    svc.revokePaymentLink(l2.linkId);
    const result = svc.listPaymentLinks({ status: 'revoked', customerId: 'alice' });
    assert.equal(result.length, 1);
    assert.equal(result[0].id, l2.linkId);
  });

  it('returns parsed items and metadata in each result', () => {
    const items = sampleItems();
    const meta = { source: 'test' };
    svc.createPaymentLink({ items, metadata: meta });
    const result = svc.listPaymentLinks();
    assert.deepEqual(result[0].items, items);
    assert.deepEqual(result[0].metadata, meta);
  });

  it('default limit is 50', () => {
    for (let i = 0; i < 55; i++) {
      svc.createPaymentLink({ items: sampleItems() });
    }
    const result = svc.listPaymentLinks();
    assert.equal(result.length, 50);
  });
});

// ===========================================================================
// revokePaymentLink
// ===========================================================================

describe('revokePaymentLink', () => {
  let svc;
  beforeEach(() => { svc = makeSvc(); });

  it('revokes an active link', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    const result = svc.revokePaymentLink(link.linkId);
    assert.equal(result.success, true);
    assert.equal(result.link.status, 'revoked');
  });

  it('sets revoked_at timestamp', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    const before = new Date().toISOString();
    const result = svc.revokePaymentLink(link.linkId);
    assert.ok(result.link.revoked_at);
    assert.ok(result.link.revoked_at >= before);
  });

  it('fails on unknown link', () => {
    assert.throws(
      () => svc.revokePaymentLink('nonexistent'),
      /not found/i,
    );
  });

  it('fails on already-revoked link', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    svc.revokePaymentLink(link.linkId);
    assert.throws(
      () => svc.revokePaymentLink(link.linkId),
      /already revoked/i,
    );
  });

  it('fails on converted link', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    svc.expressCheckout({ linkId: link.linkId });
    assert.throws(
      () => svc.revokePaymentLink(link.linkId),
      /converted/i,
    );
  });

  it('returns updated link with parsed items', () => {
    const items = sampleItems();
    const link = svc.createPaymentLink({ items });
    const result = svc.revokePaymentLink(link.linkId);
    assert.deepEqual(result.link.items, items);
  });

  it('can revoke by short code', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    const result = svc.revokePaymentLink(link.shortCode);
    assert.equal(result.success, true);
    assert.equal(result.link.status, 'revoked');
  });

  it('updates updated_at on revocation', () => {
    const link = svc.createPaymentLink({ items: sampleItems() });
    const statusBefore = svc.getPaymentLinkStatus(link.linkId);
    const updatedBefore = statusBefore.link.updated_at;
    // Revoke slightly later
    const result = svc.revokePaymentLink(link.linkId);
    assert.ok(result.link.updated_at >= updatedBefore);
  });
});

// ===========================================================================
// Edge cases
// ===========================================================================

describe('Edge cases', () => {
  let svc;
  beforeEach(() => { svc = makeSvc(); });

  it('handles large items array (100 items)', () => {
    const items = [];
    for (let i = 0; i < 100; i++) {
      items.push({ name: `Item-${i}`, quantity: 1, unitPrice: 1 });
    }
    const link = svc.createPaymentLink({ items });
    assert.equal(link.total, 100);
    const resolved = svc.resolvePaymentLink(link.linkId);
    assert.equal(resolved.items.length, 100);
  });

  it('handles zero-price items', () => {
    const items = [
      { name: 'Free Sample', quantity: 5, unitPrice: 0 },
    ];
    const link = svc.createPaymentLink({ items });
    assert.equal(link.total, 0);
  });

  it('handles special characters in metadata values', () => {
    const meta = {
      note: 'Hello "world" & <script>alert(1)</script>',
      unicode: '\u00e9\u00e0\u00fc\u00f1',
    };
    const link = svc.createPaymentLink({ items: sampleItems(), metadata: meta });
    const status = svc.getPaymentLinkStatus(link.linkId);
    assert.deepEqual(status.link.metadata, meta);
  });

  it('handles special characters in item names', () => {
    const items = [{ name: 'T-Shirt (L) "Deluxe" & More', quantity: 1, unitPrice: 29.99 }];
    const link = svc.createPaymentLink({ items });
    const resolved = svc.resolvePaymentLink(link.linkId);
    assert.equal(resolved.items[0].name, 'T-Shirt (L) "Deluxe" & More');
  });

  it('rejects unsupported currency codes', () => {
    assert.throws(
      () => svc.createPaymentLink({ items: sampleItems(), currency: 'DOGECOIN' }),
      /Unsupported currency/,
    );
  });

  it('concurrent creates do not collide on short codes', () => {
    const codes = new Set();
    for (let i = 0; i < 50; i++) {
      const link = svc.createPaymentLink({ items: sampleItems() });
      codes.add(link.shortCode);
    }
    assert.equal(codes.size, 50, 'All 50 short codes should be unique');
  });

  it('handles item with very long name', () => {
    const items = [{ name: 'A'.repeat(2000), quantity: 1, unitPrice: 1 }];
    const link = svc.createPaymentLink({ items });
    const resolved = svc.resolvePaymentLink(link.linkId);
    assert.equal(resolved.items[0].name.length, 2000);
  });

  it('validates item missing name', () => {
    const items = [{ quantity: 1, unitPrice: 10 }];
    assert.throws(() => svc.createPaymentLink({ items }), /name/);
  });

  it('validates item with non-integer quantity', () => {
    const items = [{ name: 'A', quantity: 1.5, unitPrice: 10 }];
    assert.throws(() => svc.createPaymentLink({ items }), /positive integer/);
  });

  it('validates item with negative quantity', () => {
    const items = [{ name: 'A', quantity: -1, unitPrice: 10 }];
    assert.throws(() => svc.createPaymentLink({ items }), /positive integer/);
  });
});

// ===========================================================================
// Multiple service instances sharing same store
// ===========================================================================

describe('Shared store', () => {
  it('two instances sharing a store see each others links', () => {
    const store = makeStore();
    const svc1 = createExpressCheckout(store);
    const svc2 = createExpressCheckout(store);

    const link = svc1.createPaymentLink({ items: sampleItems() });
    const resolved = svc2.resolvePaymentLink(link.linkId);
    assert.ok(resolved);
    assert.equal(resolved.link.id, link.linkId);
  });

  it('create in one, checkout in the other', () => {
    const store = makeStore();
    const svc1 = createExpressCheckout(store);
    const svc2 = createExpressCheckout(store);

    const link = svc1.createPaymentLink({ items: sampleItems() });
    const result = svc2.expressCheckout({ linkId: link.linkId });
    assert.ok(result.orderId);

    const status = svc1.getPaymentLinkStatus(link.linkId);
    assert.equal(status.status, 'converted');
  });
});

// ===========================================================================
// Idempotent schema creation
// ===========================================================================

describe('Schema idempotence', () => {
  it('creating service twice on same store does not error', () => {
    const store = makeStore();
    createExpressCheckout(store);
    assert.doesNotThrow(() => createExpressCheckout(store));
  });
});
