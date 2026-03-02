import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { WooCommerceAdapter } from '../../src/adapters/woocommerce/index.js';
import { listAdapters, getAdapter } from '../../src/adapters/index.js';

// ---------------------------------------------------------------------------
// WooCommerceAdapter
// ---------------------------------------------------------------------------

describe('woocommerce adapter', () => {
  describe('constructor', () => {
    it('creates with default config', () => {
      const adapter = new WooCommerceAdapter();
      assert.equal(adapter.platformName, 'woocommerce');
    });

    it('accepts config with credentials', () => {
      const adapter = new WooCommerceAdapter({
        siteUrl: 'https://mystore.example.com',
        consumerKey: 'ck_test',
        consumerSecret: 'cs_test',
      });
      assert.ok(adapter.client);
    });

    it('does not create client without credentials', () => {
      const adapter = new WooCommerceAdapter();
      assert.equal(adapter.client, null);
    });

    it('does not create client with partial credentials', () => {
      const adapter = new WooCommerceAdapter({ siteUrl: 'https://example.com' });
      assert.equal(adapter.client, null);
    });
  });

  describe('testConnection()', () => {
    it('returns false when no client', async () => {
      const adapter = new WooCommerceAdapter();
      assert.equal(await adapter.testConnection(), false);
    });
  });

  describe('getSupportedEntities()', () => {
    it('returns customers, products, inventory, orders', () => {
      const adapter = new WooCommerceAdapter();
      const entities = adapter.getSupportedEntities();
      assert.deepEqual(entities, ['customers', 'products', 'inventory', 'orders']);
    });
  });

  describe('getImportOrder()', () => {
    it('returns correct import order', () => {
      const adapter = new WooCommerceAdapter();
      const order = adapter.getImportOrder();
      assert.deepEqual(order, ['customers', 'products', 'inventory', 'orders']);
    });

    it('customers come before orders (FK dependency)', () => {
      const adapter = new WooCommerceAdapter();
      const order = adapter.getImportOrder();
      assert.ok(order.indexOf('customers') < order.indexOf('orders'));
    });
  });

  describe('getSupportedWebhookTopics()', () => {
    it('returns 6 topics', () => {
      const adapter = new WooCommerceAdapter();
      assert.equal(adapter.getSupportedWebhookTopics().length, 6);
    });
  });

  describe('handleWebhook()', () => {
    it('maps order.created', () => {
      const adapter = new WooCommerceAdapter();
      const result = adapter.handleWebhook('order.created', {
        id: 1,
        status: 'processing',
        total: '50.00',
        currency: 'USD',
      });
      assert.equal(result.entityType, 'orders');
      assert.equal(result.externalId, '1');
    });

    it('maps product.created', () => {
      const adapter = new WooCommerceAdapter();
      const result = adapter.handleWebhook('product.created', {
        id: 10,
        name: 'Widget',
        status: 'publish',
        price: '25.00',
      });
      assert.equal(result.entityType, 'products');
      assert.equal(result.externalId, '10');
    });

    it('maps customer.created', () => {
      const adapter = new WooCommerceAdapter();
      const result = adapter.handleWebhook('customer.created', {
        id: 5,
        email: 'test@example.com',
        first_name: 'Test',
      });
      assert.equal(result.entityType, 'customers');
      assert.equal(result.externalId, '5');
    });

    it('returns null for unsupported event', () => {
      const adapter = new WooCommerceAdapter();
      const result = adapter.handleWebhook('unknown.event', {});
      assert.equal(result, null);
    });
  });

  describe('mapToStateSet()', () => {
    it('maps customers', () => {
      const adapter = new WooCommerceAdapter();
      const result = adapter.mapToStateSet('customers', { id: 1, email: 'a@b.com' });
      assert.equal(result.entityType, 'customers');
    });

    it('maps products', () => {
      const adapter = new WooCommerceAdapter();
      const result = adapter.mapToStateSet('products', { id: 1, name: 'P', price: '10' });
      assert.equal(result.entityType, 'products');
    });

    it('throws on unknown entity', () => {
      const adapter = new WooCommerceAdapter();
      assert.throws(() => adapter.mapToStateSet('unknown', {}), /Unknown entity/);
    });
  });

  describe('mapFromStateSet()', () => {
    it('maps customers back', () => {
      const adapter = new WooCommerceAdapter();
      const result = adapter.mapFromStateSet('customers', { email: 'a@b.com' });
      assert.ok(result.email);
    });

    it('returns record as-is for unsupported types', () => {
      const adapter = new WooCommerceAdapter();
      const record = { foo: 'bar' };
      assert.equal(adapter.mapFromStateSet('orders', record), record);
    });
  });

  describe('verifyWebhookSignature()', () => {
    it('returns error when no secret configured', () => {
      const adapter = new WooCommerceAdapter();
      const result = adapter.verifyWebhookSignature('body', 'sig');
      assert.equal(result.valid, false);
      assert.match(result.error, /secret/i);
    });
  });

  describe('fetchBatches()', () => {
    it('throws when no client configured', async () => {
      const adapter = new WooCommerceAdapter();
      const gen = adapter.fetchBatches('customers');
      await assert.rejects(() => gen.next(), /not configured/);
    });
  });

  describe('parseBatchesFromFile()', () => {
    it('throws for non-JSON format', async () => {
      const adapter = new WooCommerceAdapter();
      const gen = adapter.parseBatchesFromFile('customers', 'file.csv', 'csv');
      await assert.rejects(() => gen.next(), /JSON/);
    });
  });
});

// ---------------------------------------------------------------------------
// Adapter registry
// ---------------------------------------------------------------------------

describe('woocommerce adapter — registry', () => {
  it('is registered in the adapter registry', () => {
    assert.ok(listAdapters().includes('woocommerce'));
  });

  it('can be instantiated via getAdapter()', async () => {
    const adapter = await getAdapter('woocommerce');
    assert.equal(adapter.platformName, 'woocommerce');
  });

  it('passes config through to adapter', async () => {
    const adapter = await getAdapter('woocommerce', {
      siteUrl: 'https://mystore.example.com',
      consumerKey: 'ck_test',
      consumerSecret: 'cs_test',
    });
    assert.ok(adapter.client);
  });
});
