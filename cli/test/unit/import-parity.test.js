import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

import { BasePlatformAdapter } from '../../src/adapters/base-adapter.js';
import { DataImporter } from '../../src/adapters/base-importer.js';
import {
  mapToStateSet as mapperToStateSet,
  mapFromStateSet,
  mapCustomerToStateSet,
  mapProductToStateSet,
} from '../../src/adapters/shopify/mapper.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const fixturesDir = path.join(__dirname, '..', 'fixtures', 'shopify');

// ---------------------------------------------------------------------------
// Mock helpers
// ---------------------------------------------------------------------------

class RoundTripAdapter extends BasePlatformAdapter {
  constructor(records = {}) {
    super('roundtrip');
    this._records = records;
  }

  async testConnection() {
    return true;
  }

  mapToStateSet(entityType, record) {
    // mapperToStateSet returns { entityType, externalId, data, raw }
    return mapperToStateSet(entityType, record);
  }

  mapFromStateSet(entityType, record) {
    return mapFromStateSet(entityType, record);
  }

  async *fetchBatches(entityType) {
    const records = this._records[entityType] || [];
    if (records.length > 0) {
      yield {
        entityType,
        records,
        page: 1,
        hasMore: false,
      };
    }
  }

  handleWebhook() {
    return null;
  }
}

function createMockCommerce() {
  const store = { customers: [], products: [], orders: [], inventory: [] };
  return {
    customers: {
      create: async (data) => {
        const id = `cust-${store.customers.length + 1}`;
        store.customers.push({ id, ...data });
        return { id };
      },
    },
    products: {
      create: async (data) => {
        const id = `prod-${store.products.length + 1}`;
        store.products.push({ id, ...data });
        return { id };
      },
    },
    orders: {
      create: async (data) => {
        const id = `ord-${store.orders.length + 1}`;
        store.orders.push({ id, ...data });
        return { id };
      },
    },
    inventory: {
      create: async (data) => {
        const id = `inv-${store.inventory.length + 1}`;
        store.inventory.push({ id, ...data });
        return { id };
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
      map.set(key, { statesetId, importedAt: new Date().toISOString() });
    },
    _map: map,
  };
}

// ---------------------------------------------------------------------------
// Round-trip: Shopify JSON → StateSet → Shopify-like JSON
// ---------------------------------------------------------------------------

describe('Import parity — round-trip mapping', () => {
  it('customer round-trips preserve key fields', () => {
    const shopifyCustomer = {
      id: 1001,
      email: 'alice@example.com',
      first_name: 'Alice',
      last_name: 'Smith',
      phone: '+15551234567',
      state: 'enabled',
      accepts_marketing: true,
      tags: 'vip, wholesale',
      note: 'Important customer',
    };

    const mapped = mapCustomerToStateSet(shopifyCustomer);
    assert.equal(mapped.data.email, 'alice@example.com');
    assert.equal(mapped.data.firstName, 'Alice');
    assert.equal(mapped.data.lastName, 'Smith');
    assert.equal(mapped.data.status, 'active');

    const back = mapFromStateSet('customers', mapped.data);
    assert.equal(back.email, 'alice@example.com');
    assert.equal(back.first_name, 'Alice');
    assert.equal(back.last_name, 'Smith');
  });

  it('product round-trips preserve name and variants', () => {
    const shopifyProduct = {
      id: 2001,
      title: 'Classic Widget',
      body_html: '<p>A <strong>premium</strong> widget.</p>',
      handle: 'classic-widget',
      status: 'active',
      vendor: 'WidgetCo',
      product_type: 'Widgets',
      variants: [
        { id: 3001, title: 'Small', sku: 'WIDGET-SM', price: '19.99', grams: 100 },
        { id: 3002, title: 'Large', sku: 'WIDGET-LG', price: '29.99', grams: 200 },
      ],
    };

    const mapped = mapProductToStateSet(shopifyProduct);
    assert.equal(mapped.data.name, 'Classic Widget');
    assert.equal(mapped.data.description, 'A premium widget.');
    assert.equal(mapped.data.variants.length, 2);
    assert.equal(mapped.data.variants[0].sku, 'WIDGET-SM');
    assert.equal(mapped.data.variants[1].sku, 'WIDGET-LG');

    const back = mapFromStateSet('products', mapped.data);
    assert.equal(back.title, 'Classic Widget');
  });
});

// ---------------------------------------------------------------------------
// Fixture-based import
// ---------------------------------------------------------------------------

describe('Import parity — fixture data', () => {
  let customersJson, productsJson, ordersJson;

  beforeEach(async () => {
    customersJson = JSON.parse(await readFile(path.join(fixturesDir, 'customers.json'), 'utf-8'));
    productsJson = JSON.parse(await readFile(path.join(fixturesDir, 'products.json'), 'utf-8'));
    ordersJson = JSON.parse(await readFile(path.join(fixturesDir, 'orders.json'), 'utf-8'));
  });

  it('imports all 5 fixture customers', async () => {
    const adapter = new RoundTripAdapter({ customers: customersJson.customers });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    const result = await importer.run({
      source: 'api',
      entities: ['customers'],
    });

    assert.equal(result.success, true);
    assert.equal(result.totalCreated, 5);
    assert.equal(commerce._store.customers.length, 5);
  });

  it('imports all fixture products with variants', async () => {
    const adapter = new RoundTripAdapter({ products: productsJson.products });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    const result = await importer.run({
      source: 'api',
      entities: ['products'],
    });

    assert.equal(result.success, true);
    assert.equal(result.totalCreated, 5);

    // Verify variants are preserved
    const firstProduct = commerce._store.products[0];
    assert.ok(firstProduct.variants);
    assert.ok(firstProduct.variants.length >= 1);
  });

  it('imports all fixture orders', async () => {
    const adapter = new RoundTripAdapter({ orders: ordersJson.orders });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    const result = await importer.run({
      source: 'api',
      entities: ['orders'],
    });

    assert.equal(result.success, true);
    assert.equal(result.totalCreated, 5);
  });

  it('incremental import skips already-imported records', async () => {
    const adapter = new RoundTripAdapter({ customers: customersJson.customers });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    // First import
    await importer.run({ source: 'api', entities: ['customers'] });
    assert.equal(commerce._store.customers.length, 5);

    // Second import — incremental should skip all
    const commerce2 = createMockCommerce();
    const importer2 = new DataImporter(adapter, commerce2, idMapStore);
    const result2 = await importer2.run({
      source: 'api',
      entities: ['customers'],
      incremental: true,
    });

    assert.equal(result2.totalSkipped, 5);
    assert.equal(result2.totalCreated, 0);
    assert.equal(commerce2._store.customers.length, 0);
  });

  it('dry-run does not persist records', async () => {
    const adapter = new RoundTripAdapter({ customers: customersJson.customers });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    const result = await importer.run({
      source: 'api',
      entities: ['customers'],
      dryRun: true,
    });

    assert.equal(result.dryRun, true);
    assert.equal(result.totalCreated, 5); // counted but not written
    assert.equal(commerce._store.customers.length, 0);
  });
});

// ---------------------------------------------------------------------------
// Multi-entity import ordering
// ---------------------------------------------------------------------------

describe('Import parity — multi-entity ordering', () => {
  let customersJson, productsJson, ordersJson;

  beforeEach(async () => {
    customersJson = JSON.parse(await readFile(path.join(fixturesDir, 'customers.json'), 'utf-8'));
    productsJson = JSON.parse(await readFile(path.join(fixturesDir, 'products.json'), 'utf-8'));
    ordersJson = JSON.parse(await readFile(path.join(fixturesDir, 'orders.json'), 'utf-8'));
  });

  it('imports customers before orders even when specified in reverse', async () => {
    const importOrder = [];
    const adapter = new RoundTripAdapter({
      customers: customersJson.customers,
      orders: ordersJson.orders,
    });

    const origMap = adapter.mapToStateSet.bind(adapter);
    adapter.mapToStateSet = (entityType, record) => {
      importOrder.push(entityType);
      return origMap(entityType, record);
    };

    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    await importer.run({
      source: 'api',
      entities: ['orders', 'customers'],
    });

    // Customers should appear before orders
    const firstCustomerIdx = importOrder.indexOf('customers');
    const firstOrderIdx = importOrder.indexOf('orders');
    assert.ok(firstCustomerIdx < firstOrderIdx, 'customers should be imported before orders');
  });

  it('full import with all entity types succeeds', async () => {
    const adapter = new RoundTripAdapter({
      customers: customersJson.customers,
      products: productsJson.products,
      orders: ordersJson.orders,
    });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    const result = await importer.run({
      source: 'api',
      entities: ['customers', 'products', 'orders'],
    });

    assert.equal(result.success, true);
    assert.equal(result.totalCreated, 15); // 5+5+5
    assert.ok(result.entities.customers);
    assert.ok(result.entities.products);
    assert.ok(result.entities.orders);
  });
});

// ---------------------------------------------------------------------------
// ID map queryability
// ---------------------------------------------------------------------------

describe('Import parity — ID map queryability', () => {
  let customersJson;

  beforeEach(async () => {
    customersJson = JSON.parse(await readFile(path.join(fixturesDir, 'customers.json'), 'utf-8'));
  });

  it('all imported records are queryable via id map', async () => {
    const adapter = new RoundTripAdapter({ customers: customersJson.customers });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    await importer.run({ source: 'api', entities: ['customers'] });

    for (const customer of customersJson.customers) {
      const mapping = idMapStore.lookup('roundtrip', 'customers', String(customer.id));
      assert.ok(mapping, `Customer ${customer.id} should be in id map`);
      assert.ok(mapping.statesetId, `Customer ${customer.id} should have statesetId`);
    }
  });

  it('id map entries have correct platform prefix', async () => {
    const adapter = new RoundTripAdapter({ customers: customersJson.customers.slice(0, 1) });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    await importer.run({ source: 'api', entities: ['customers'] });

    const key = `roundtrip:customers:${customersJson.customers[0].id}`;
    assert.ok(idMapStore._map.has(key));
  });
});
