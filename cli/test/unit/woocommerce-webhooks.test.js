import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'crypto';

import {
  createWooCommerceWebhookHandlers,
  getSupportedWooCommerceTopics,
  verifyWooCommerceSignature,
} from '../../src/adapters/woocommerce/webhooks.js';

// ---------------------------------------------------------------------------
// Mock commerce and idMapStore
// ---------------------------------------------------------------------------

function createMockCommerce() {
  const records = { orders: [], products: [], customers: [] };
  return {
    orders: {
      create: async (data) => {
        records.orders.push(data);
        return { id: `ord_${records.orders.length}` };
      },
    },
    products: {
      create: async (data) => {
        records.products.push(data);
        return { id: `prod_${records.products.length}` };
      },
    },
    customers: {
      create: async (data) => {
        records.customers.push(data);
        return { id: `cust_${records.customers.length}` };
      },
    },
    _records: records,
  };
}

function createMockIdMapStore() {
  const store = new Map();
  return {
    lookup: (platform, entityType, externalId) => {
      return store.get(`${platform}:${entityType}:${externalId}`) || null;
    },
    store: (platform, entityType, externalId, statesetId, raw) => {
      store.set(`${platform}:${entityType}:${externalId}`, { statesetId, raw });
    },
    _store: store,
  };
}

// ---------------------------------------------------------------------------
// verifyWooCommerceSignature
// ---------------------------------------------------------------------------

describe('woocommerce webhooks — verifyWooCommerceSignature', () => {
  const secret = 'webhook-secret-123';

  it('verifies a valid signature', () => {
    const body = '{"id":1,"status":"processing"}';
    const signature = crypto.createHmac('sha256', secret).update(body, 'utf-8').digest('base64');
    const result = verifyWooCommerceSignature(body, signature, secret);
    assert.equal(result.valid, true);
  });

  it('rejects invalid signature', () => {
    const body = '{"id":1}';
    const result = verifyWooCommerceSignature(body, 'invalid-sig', secret);
    assert.equal(result.valid, false);
    assert.match(result.error, /mismatch/i);
  });

  it('rejects tampered body', () => {
    const body = '{"id":1}';
    const signature = crypto.createHmac('sha256', secret).update(body, 'utf-8').digest('base64');
    const result = verifyWooCommerceSignature('{"id":2}', signature, secret);
    assert.equal(result.valid, false);
  });

  it('rejects missing body', () => {
    const result = verifyWooCommerceSignature(null, 'sig', secret);
    assert.equal(result.valid, false);
    assert.match(result.error, /body/i);
  });

  it('rejects missing signature header', () => {
    const result = verifyWooCommerceSignature('body', null, secret);
    assert.equal(result.valid, false);
    assert.match(result.error, /header/i);
  });

  it('rejects missing secret', () => {
    const result = verifyWooCommerceSignature('body', 'sig', null);
    assert.equal(result.valid, false);
    assert.match(result.error, /secret/i);
  });

  it('rejects empty string body', () => {
    const result = verifyWooCommerceSignature('', 'sig', secret);
    assert.equal(result.valid, false);
  });

  it('rejects empty string signature', () => {
    const result = verifyWooCommerceSignature('body', '', secret);
    assert.equal(result.valid, false);
  });

  it('rejects wrong secret', () => {
    const body = '{"id":1}';
    const signature = crypto.createHmac('sha256', secret).update(body, 'utf-8').digest('base64');
    const result = verifyWooCommerceSignature(body, signature, 'wrong-secret');
    assert.equal(result.valid, false);
  });
});

// ---------------------------------------------------------------------------
// Webhook handlers
// ---------------------------------------------------------------------------

describe('woocommerce webhooks — handlers', () => {
  let commerce;
  let idMapStore;
  let handlers;

  beforeEach(() => {
    commerce = createMockCommerce();
    idMapStore = createMockIdMapStore();
    handlers = createWooCommerceWebhookHandlers(commerce, idMapStore);
  });

  // --- order.created ---

  describe('order.created', () => {
    it('creates an order', async () => {
      const result = await handlers['order.created']({
        id: 100,
        status: 'processing',
        total: '50.00',
        currency: 'USD',
      });
      assert.equal(result.action, 'created');
      assert.equal(result.externalId, '100');
      assert.equal(commerce._records.orders.length, 1);
    });

    it('skips duplicate order', async () => {
      idMapStore.store('woocommerce', 'orders', '100', 'ord_existing', {});
      const result = await handlers['order.created']({
        id: 100,
        status: 'processing',
        total: '50.00',
      });
      assert.equal(result.action, 'skipped');
    });

    it('handles missing orders handler', async () => {
      commerce.orders = null;
      const result = await handlers['order.created']({
        id: 100,
        status: 'processing',
        total: '50.00',
      });
      assert.equal(result.action, 'skipped');
      assert.match(result.reason, /handler/i);
    });
  });

  // --- order.updated ---

  describe('order.updated', () => {
    it('updates existing order', async () => {
      idMapStore.store('woocommerce', 'orders', '100', 'ord_1', {});
      const result = await handlers['order.updated']({
        id: 100,
        status: 'completed',
        total: '50.00',
      });
      assert.equal(result.action, 'updated');
    });

    it('creates order if not exists', async () => {
      const result = await handlers['order.updated']({
        id: 200,
        status: 'processing',
        total: '30.00',
      });
      assert.equal(result.action, 'created');
    });
  });

  // --- product.created ---

  describe('product.created', () => {
    it('creates a product', async () => {
      const result = await handlers['product.created']({
        id: 50,
        name: 'Widget',
        status: 'publish',
        price: '25.00',
      });
      assert.equal(result.action, 'created');
      assert.equal(result.externalId, '50');
      assert.equal(commerce._records.products.length, 1);
    });

    it('skips duplicate product', async () => {
      idMapStore.store('woocommerce', 'products', '50', 'prod_existing', {});
      const result = await handlers['product.created']({
        id: 50,
        name: 'Widget',
        price: '25.00',
      });
      assert.equal(result.action, 'skipped');
    });
  });

  // --- product.updated ---

  describe('product.updated', () => {
    it('updates existing product', async () => {
      idMapStore.store('woocommerce', 'products', '50', 'prod_1', {});
      const result = await handlers['product.updated']({
        id: 50,
        name: 'Updated Widget',
        price: '30.00',
      });
      assert.equal(result.action, 'updated');
    });

    it('creates product if not exists', async () => {
      const result = await handlers['product.updated']({
        id: 60,
        name: 'New Product',
        price: '15.00',
      });
      assert.equal(result.action, 'created');
    });

    it('handles missing products handler', async () => {
      commerce.products = null;
      const result = await handlers['product.updated']({
        id: 50,
        name: 'Widget',
        price: '25.00',
      });
      assert.equal(result.action, 'skipped');
      assert.match(result.reason, /handler/i);
    });
  });

  // --- customer.created ---

  describe('customer.created', () => {
    it('creates a customer', async () => {
      const result = await handlers['customer.created']({
        id: 10,
        email: 'test@example.com',
        first_name: 'Test',
        last_name: 'User',
      });
      assert.equal(result.action, 'created');
      assert.equal(result.externalId, '10');
      assert.equal(commerce._records.customers.length, 1);
    });

    it('skips duplicate customer', async () => {
      idMapStore.store('woocommerce', 'customers', '10', 'cust_existing', {});
      const result = await handlers['customer.created']({
        id: 10,
        email: 'test@example.com',
      });
      assert.equal(result.action, 'skipped');
    });

    it('handles missing customers handler', async () => {
      commerce.customers = null;
      const result = await handlers['customer.created']({
        id: 10,
        email: 'test@example.com',
      });
      assert.equal(result.action, 'skipped');
      assert.match(result.reason, /handler/i);
    });
  });

  // --- customer.updated ---

  describe('customer.updated', () => {
    it('updates existing customer', async () => {
      idMapStore.store('woocommerce', 'customers', '10', 'cust_1', {});
      const result = await handlers['customer.updated']({
        id: 10,
        email: 'updated@example.com',
      });
      assert.equal(result.action, 'updated');
    });

    it('creates customer if not exists', async () => {
      const result = await handlers['customer.updated']({
        id: 20,
        email: 'new@example.com',
        first_name: 'New',
      });
      assert.equal(result.action, 'created');
    });
  });
});

// ---------------------------------------------------------------------------
// getSupportedWooCommerceTopics
// ---------------------------------------------------------------------------

describe('woocommerce webhooks — getSupportedWooCommerceTopics', () => {
  it('returns 6 topics', () => {
    assert.equal(getSupportedWooCommerceTopics().length, 6);
  });

  it('includes order.created', () => {
    assert.ok(getSupportedWooCommerceTopics().includes('order.created'));
  });

  it('includes order.updated', () => {
    assert.ok(getSupportedWooCommerceTopics().includes('order.updated'));
  });

  it('includes product.created', () => {
    assert.ok(getSupportedWooCommerceTopics().includes('product.created'));
  });

  it('includes customer.updated', () => {
    assert.ok(getSupportedWooCommerceTopics().includes('customer.updated'));
  });
});
