import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import {
  createShopifyWebhookHandlers,
  getSupportedTopics,
} from '../../src/adapters/shopify/webhooks.js';

// ---------------------------------------------------------------------------
// Mock helpers
// ---------------------------------------------------------------------------

function createMockCommerce() {
  const store = {
    customers: [],
    products: [],
    orders: [],
    inventory: [],
    shipments: [],
  };

  return {
    customers: {
      create: async (data) => {
        const id = `cust-${store.customers.length + 1}`;
        const record = { id, ...data };
        store.customers.push(record);
        return record;
      },
    },
    products: {
      create: async (data) => {
        const id = `prod-${store.products.length + 1}`;
        const record = { id, ...data };
        store.products.push(record);
        return record;
      },
    },
    orders: {
      create: async (data) => {
        const id = `ord-${store.orders.length + 1}`;
        const record = { id, ...data };
        store.orders.push(record);
        return record;
      },
      cancel: async (id) => {
        const idx = store.orders.findIndex((o) => o.id === id);
        if (idx >= 0) store.orders[idx].status = 'cancelled';
      },
    },
    inventory: {
      create: async (data) => {
        const id = `inv-${store.inventory.length + 1}`;
        const record = { id, ...data };
        store.inventory.push(record);
        return record;
      },
      adjust: async (data) => {
        const idx = store.inventory.findIndex((i) => i.sku === data.sku);
        if (idx >= 0) store.inventory[idx].quantity = data.quantity;
      },
    },
    shipments: {
      create: async (data) => {
        const id = `ship-${store.shipments.length + 1}`;
        const record = { id, ...data };
        store.shipments.push(record);
        return record;
      },
    },
    _store: store,
  };
}

function createMockIdMapStore() {
  const map = new Map();

  return {
    lookup: (platform, entityType, externalId) => {
      const key = `${platform}:${entityType}:${externalId}`;
      return map.get(key) || null;
    },
    store: (platform, entityType, externalId, statesetId, raw) => {
      const key = `${platform}:${entityType}:${externalId}`;
      map.set(key, { statesetId, importedAt: new Date().toISOString(), externalData: raw });
    },
    _map: map,
  };
}

// ---------------------------------------------------------------------------
// getSupportedTopics
// ---------------------------------------------------------------------------

describe('getSupportedTopics', () => {
  it('returns all supported topics including fulfillments', () => {
    const topics = getSupportedTopics();
    assert.equal(topics.length, 10);
    assert.ok(topics.includes('customers/create'));
    assert.ok(topics.includes('customers/update'));
    assert.ok(topics.includes('products/create'));
    assert.ok(topics.includes('products/update'));
    assert.ok(topics.includes('orders/create'));
    assert.ok(topics.includes('orders/updated'));
    assert.ok(topics.includes('fulfillments/create'));
    assert.ok(topics.includes('fulfillments/update'));
    assert.ok(topics.includes('orders/cancelled'));
    assert.ok(topics.includes('inventory_levels/update'));
  });
});

// ---------------------------------------------------------------------------
// customers/create
// ---------------------------------------------------------------------------

describe('webhook: customers/create', () => {
  let commerce, idMapStore, handlers;

  beforeEach(() => {
    commerce = createMockCommerce();
    idMapStore = createMockIdMapStore();
    handlers = createShopifyWebhookHandlers(commerce, idMapStore);
  });

  it('creates a new customer and stores id mapping', async () => {
    const payload = {
      id: 1001,
      email: 'test@example.com',
      first_name: 'Test',
      last_name: 'User',
      phone: null,
      state: 'enabled',
      accepts_marketing: false,
      tags: '',
      note: '',
    };

    const result = await handlers['customers/create'](payload);
    assert.equal(result.action, 'created');
    assert.equal(result.externalId, '1001');
    assert.ok(result.statesetId);
    assert.equal(commerce._store.customers.length, 1);
    assert.ok(idMapStore.lookup('shopify', 'customers', '1001'));
  });

  it('skips if customer already exists in id_map', async () => {
    idMapStore.store('shopify', 'customers', '1001', 'existing-id');

    const payload = {
      id: 1001,
      email: 'test@example.com',
      first_name: 'Test',
      last_name: 'User',
      state: 'enabled',
    };
    const result = await handlers['customers/create'](payload);
    assert.equal(result.action, 'skipped');
    assert.equal(result.statesetId, 'existing-id');
    assert.equal(commerce._store.customers.length, 0);
  });
});

// ---------------------------------------------------------------------------
// customers/update
// ---------------------------------------------------------------------------

describe('webhook: customers/update', () => {
  let commerce, idMapStore, handlers;

  beforeEach(() => {
    commerce = createMockCommerce();
    idMapStore = createMockIdMapStore();
    handlers = createShopifyWebhookHandlers(commerce, idMapStore);
  });

  it('updates an existing customer mapping', async () => {
    idMapStore.store('shopify', 'customers', '1001', 'existing-id');

    const payload = {
      id: 1001,
      email: 'updated@example.com',
      first_name: 'Updated',
      last_name: 'User',
      state: 'enabled',
    };
    const result = await handlers['customers/update'](payload);
    assert.equal(result.action, 'updated');
    assert.equal(result.statesetId, 'existing-id');
  });

  it('creates customer if not exists', async () => {
    const payload = {
      id: 1001,
      email: 'new@example.com',
      first_name: 'New',
      last_name: 'User',
      state: 'enabled',
    };
    const result = await handlers['customers/update'](payload);
    assert.equal(result.action, 'created');
    assert.equal(commerce._store.customers.length, 1);
  });
});

// ---------------------------------------------------------------------------
// products/create
// ---------------------------------------------------------------------------

describe('webhook: products/create', () => {
  let commerce, idMapStore, handlers;

  beforeEach(() => {
    commerce = createMockCommerce();
    idMapStore = createMockIdMapStore();
    handlers = createShopifyWebhookHandlers(commerce, idMapStore);
  });

  it('creates a new product', async () => {
    const payload = {
      id: 2001,
      title: 'New Product',
      body_html: '<p>Description</p>',
      handle: 'new-product',
      status: 'active',
      variants: [{ id: 3001, title: 'Default', sku: 'SKU-1', price: '10.00' }],
    };

    const result = await handlers['products/create'](payload);
    assert.equal(result.action, 'created');
    assert.equal(result.externalId, '2001');
    assert.equal(commerce._store.products.length, 1);
  });

  it('skips if product already exists', async () => {
    idMapStore.store('shopify', 'products', '2001', 'existing-prod');

    const payload = {
      id: 2001,
      title: 'Existing',
      handle: 'existing',
      status: 'active',
      variants: [],
    };
    const result = await handlers['products/create'](payload);
    assert.equal(result.action, 'skipped');
    assert.equal(commerce._store.products.length, 0);
  });
});

// ---------------------------------------------------------------------------
// products/update
// ---------------------------------------------------------------------------

describe('webhook: products/update', () => {
  let commerce, idMapStore, handlers;

  beforeEach(() => {
    commerce = createMockCommerce();
    idMapStore = createMockIdMapStore();
    handlers = createShopifyWebhookHandlers(commerce, idMapStore);
  });

  it('updates existing product mapping', async () => {
    idMapStore.store('shopify', 'products', '2001', 'existing-prod');

    const payload = {
      id: 2001,
      title: 'Updated',
      handle: 'updated',
      status: 'active',
      variants: [],
    };
    const result = await handlers['products/update'](payload);
    assert.equal(result.action, 'updated');
  });

  it('creates product if not exists', async () => {
    const payload = { id: 2001, title: 'New', handle: 'new', status: 'active', variants: [] };
    const result = await handlers['products/update'](payload);
    assert.equal(result.action, 'created');
  });
});

// ---------------------------------------------------------------------------
// orders/create
// ---------------------------------------------------------------------------

describe('webhook: orders/create', () => {
  let commerce, idMapStore, handlers;

  beforeEach(() => {
    commerce = createMockCommerce();
    idMapStore = createMockIdMapStore();
    handlers = createShopifyWebhookHandlers(commerce, idMapStore);
  });

  it('creates an order with customer resolution', async () => {
    idMapStore.store('shopify', 'customers', '1001', 'stateset-cust-1');

    const payload = {
      id: 5001,
      order_number: 1001,
      customer: { id: 1001 },
      financial_status: 'paid',
      fulfillment_status: null,
      currency: 'USD',
      total_price: '19.99',
      line_items: [{ id: 6001, name: 'Widget', sku: 'W-1', quantity: 1, price: '19.99' }],
    };

    const result = await handlers['orders/create'](payload);
    assert.equal(result.action, 'created');
    assert.equal(commerce._store.orders.length, 1);
    assert.equal(commerce._store.orders[0].customerId, 'stateset-cust-1');
  });

  it('creates order with null customerId when customer not in map', async () => {
    const payload = {
      id: 5001,
      customer: { id: 9999 },
      financial_status: 'pending',
      total_price: '10.00',
      line_items: [{ id: 6001, name: 'Item', sku: 'S-1', quantity: 1, price: '10.00' }],
    };

    const result = await handlers['orders/create'](payload);
    assert.equal(result.action, 'created');
    assert.equal(commerce._store.orders[0].customerId, null);
  });
});

// ---------------------------------------------------------------------------
// fulfillments/create and fulfillments/update
// ---------------------------------------------------------------------------

describe('webhook: fulfillments/*', () => {
  let commerce, idMapStore, handlers;

  beforeEach(() => {
    commerce = createMockCommerce();
    idMapStore = createMockIdMapStore();
    handlers = createShopifyWebhookHandlers(commerce, idMapStore);
  });

  it('creates a fulfillment shipment record', async () => {
    idMapStore.store('shopify', 'orders', '5001', 'ord-1');
    const result = await handlers['fulfillments/create']({
      id: 7001,
      order_id: 5001,
      status: 'success',
      tracking_number: 'TRACK-1',
      tracking_company: 'FedEx',
    });

    assert.equal(result.action, 'created');
    assert.equal(result.externalId, '7001');
    assert.equal(commerce._store.shipments.length, 1);
    assert.equal(commerce._store.shipments[0].orderId, 'ord-1');
  });

  it('updates existing fulfillment mapping', async () => {
    idMapStore.store('shopify', 'fulfillments', '7002', 'ship-4');
    const result = await handlers['fulfillments/update']({
      id: 7002,
      order_id: 5002,
      status: 'cancelled',
    });

    assert.equal(result.action, 'updated');
    assert.equal(result.statesetId, 'ship-4');
  });
});

// ---------------------------------------------------------------------------
// orders/cancelled
// ---------------------------------------------------------------------------

describe('webhook: orders/cancelled', () => {
  let commerce, idMapStore, handlers;

  beforeEach(() => {
    commerce = createMockCommerce();
    idMapStore = createMockIdMapStore();
    handlers = createShopifyWebhookHandlers(commerce, idMapStore);
  });

  it('cancels an existing order', async () => {
    // Create an order first
    commerce._store.orders.push({ id: 'ord-1', status: 'active' });
    idMapStore.store('shopify', 'orders', '5001', 'ord-1');

    const result = await handlers['orders/cancelled']({ id: 5001 });
    assert.equal(result.action, 'cancelled');
    assert.equal(result.statesetId, 'ord-1');
    assert.equal(commerce._store.orders[0].status, 'cancelled');
  });

  it('skips if order not found in id_map', async () => {
    const result = await handlers['orders/cancelled']({ id: 9999 });
    assert.equal(result.action, 'skipped');
    assert.ok(result.reason);
  });
});

// ---------------------------------------------------------------------------
// inventory_levels/update
// ---------------------------------------------------------------------------

describe('webhook: inventory_levels/update', () => {
  let commerce, idMapStore, handlers;

  beforeEach(() => {
    commerce = createMockCommerce();
    idMapStore = createMockIdMapStore();
    handlers = createShopifyWebhookHandlers(commerce, idMapStore);
  });

  it('adjusts existing inventory level', async () => {
    commerce._store.inventory.push({ id: 'inv-1', sku: 'WIDGET-SM', quantity: 10 });
    idMapStore.store('shopify', 'inventory', '4001', 'inv-1');

    const result = await handlers['inventory_levels/update']({
      inventory_item_id: 4001,
      sku: 'WIDGET-SM',
      available: 25,
      location_id: 'loc-1',
    });
    assert.equal(result.action, 'adjusted');
    assert.equal(commerce._store.inventory[0].quantity, 25);
  });

  it('creates inventory item if not exists', async () => {
    const result = await handlers['inventory_levels/update']({
      inventory_item_id: 4001,
      sku: 'NEW-SKU',
      available: 50,
    });
    assert.equal(result.action, 'created');
    assert.equal(commerce._store.inventory.length, 1);
    assert.equal(commerce._store.inventory[0].sku, 'NEW-SKU');
    assert.equal(commerce._store.inventory[0].quantity, 50);
  });
});
