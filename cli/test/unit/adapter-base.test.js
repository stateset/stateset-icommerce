import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import { BasePlatformAdapter } from '../../src/adapters/base-adapter.js';
import { DataImporter } from '../../src/adapters/base-importer.js';

// ---------------------------------------------------------------------------
// Mock helpers
// ---------------------------------------------------------------------------

class MockAdapter extends BasePlatformAdapter {
  constructor(batches = {}) {
    super('mock');
    this._batches = batches;
    this._mapCalls = [];
  }

  async testConnection() {
    return true;
  }

  mapToStateSet(entityType, record, context) {
    this._mapCalls.push({ entityType, record });
    return {
      entityType,
      externalId: String(record.id),
      data: { ...record, mapped: true },
      raw: record,
    };
  }

  mapFromStateSet(entityType, record) {
    return { ...record, unmapped: true };
  }

  async *fetchBatches(entityType, options) {
    const batches = this._batches[entityType] || [];
    for (let i = 0; i < batches.length; i++) {
      yield {
        entityType,
        records: batches[i],
        page: i + 1,
        hasMore: i < batches.length - 1,
      };
    }
  }

  handleWebhook(eventType, payload) {
    return this.mapToStateSet('customers', payload);
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
// BasePlatformAdapter
// ---------------------------------------------------------------------------

describe('BasePlatformAdapter', () => {
  it('cannot be instantiated directly', () => {
    assert.throws(() => new BasePlatformAdapter('test'), /abstract/i);
  });

  it('stores platformName', () => {
    const adapter = new MockAdapter();
    assert.equal(adapter.platformName, 'mock');
  });

  it('returns default supported entities', () => {
    const adapter = new MockAdapter();
    const entities = adapter.getSupportedEntities();
    assert.deepStrictEqual(entities, ['customers', 'products', 'orders', 'inventory']);
  });

  it('returns correct import order', () => {
    const adapter = new MockAdapter();
    const order = adapter.getImportOrder();
    assert.deepStrictEqual(order, ['customers', 'products', 'inventory', 'orders']);
  });
});

// ---------------------------------------------------------------------------
// DataImporter
// ---------------------------------------------------------------------------

describe('DataImporter', () => {
  it('requires adapter, commerce, and idMapStore', () => {
    assert.throws(() => new DataImporter(null, {}, {}), /adapter/);
    assert.throws(() => new DataImporter({}, null, {}), /commerce/);
    assert.throws(() => new DataImporter({}, {}, null), /IdMapStore/);
  });

  it('runs a basic import', async () => {
    const adapter = new MockAdapter({
      customers: [
        [
          { id: 1, email: 'a@b.com' },
          { id: 2, email: 'c@d.com' },
        ],
      ],
    });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    const result = await importer.run({
      source: 'api',
      entities: ['customers'],
    });

    assert.equal(result.success, true);
    assert.equal(result.totalCreated, 2);
    assert.equal(result.totalSkipped, 0);
    assert.equal(result.totalFailed, 0);
    assert.equal(commerce._store.customers.length, 2);
    assert.ok(idMapStore.lookup('mock', 'customers', '1'));
    assert.ok(idMapStore.lookup('mock', 'customers', '2'));
  });

  it('enforces import order (customers before orders)', async () => {
    const importedOrder = [];
    const adapter = new MockAdapter({
      customers: [[{ id: 1, email: 'a@b.com' }]],
      orders: [[{ id: 10, total: 100 }]],
    });

    const originalMap = adapter.mapToStateSet.bind(adapter);
    adapter.mapToStateSet = (entityType, record, ctx) => {
      importedOrder.push(entityType);
      return originalMap(entityType, record, ctx);
    };

    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    await importer.run({
      source: 'api',
      entities: ['orders', 'customers'], // Reversed order
    });

    assert.equal(importedOrder[0], 'customers');
    assert.equal(importedOrder[1], 'orders');
  });

  it('skips existing records in incremental mode', async () => {
    const adapter = new MockAdapter({
      customers: [
        [
          { id: 1, email: 'a@b.com' },
          { id: 2, email: 'c@d.com' },
        ],
      ],
    });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();

    // Pre-populate id_map with customer 1
    idMapStore.store('mock', 'customers', '1', 'existing-id');

    const importer = new DataImporter(adapter, commerce, idMapStore);
    const result = await importer.run({
      source: 'api',
      entities: ['customers'],
      incremental: true,
    });

    assert.equal(result.totalCreated, 1);
    assert.equal(result.totalSkipped, 1);
    assert.equal(commerce._store.customers.length, 1); // Only customer 2
  });

  it('does not skip in non-incremental mode', async () => {
    const adapter = new MockAdapter({
      customers: [[{ id: 1, email: 'a@b.com' }]],
    });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    idMapStore.store('mock', 'customers', '1', 'existing-id');

    const importer = new DataImporter(adapter, commerce, idMapStore);
    const result = await importer.run({
      source: 'api',
      entities: ['customers'],
      incremental: false,
    });

    assert.equal(result.totalCreated, 1);
    assert.equal(result.totalSkipped, 0);
  });

  it('dry-run mode does not write records', async () => {
    const adapter = new MockAdapter({
      customers: [[{ id: 1, email: 'a@b.com' }]],
    });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    const result = await importer.run({
      source: 'api',
      entities: ['customers'],
      dryRun: true,
    });

    assert.equal(result.dryRun, true);
    assert.equal(result.totalCreated, 1); // Counted but not written
    assert.equal(commerce._store.customers.length, 0); // Nothing written
    assert.equal(idMapStore.lookup('mock', 'customers', '1'), null); // No mapping stored
  });

  it('calls progress callback', async () => {
    const adapter = new MockAdapter({
      customers: [[{ id: 1, email: 'a@b.com' }]],
    });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    const progressCalls = [];
    await importer.run({
      source: 'api',
      entities: ['customers'],
      onProgress: (p) => progressCalls.push({ ...p }),
    });

    assert.ok(progressCalls.length >= 1);
    const last = progressCalls[progressCalls.length - 1];
    assert.equal(last.entity, 'customers');
    assert.equal(last.phase, 'complete');
    assert.equal(last.created, 1);
  });

  it('records duration', async () => {
    const adapter = new MockAdapter({ customers: [[{ id: 1 }]] });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    const result = await importer.run({ source: 'api', entities: ['customers'] });
    assert.ok(result.durationMs >= 0);
  });

  it('handles mapper returning null gracefully', async () => {
    const adapter = new MockAdapter({ customers: [[{ id: 1 }]] });
    adapter.mapToStateSet = () => null;

    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    const result = await importer.run({ source: 'api', entities: ['customers'] });
    assert.equal(result.totalFailed, 1);
    assert.equal(result.entities.customers.errors.length, 1);
  });

  it('handles commerce create errors gracefully', async () => {
    const adapter = new MockAdapter({ customers: [[{ id: 1 }]] });
    const commerce = createMockCommerce();
    commerce.customers.create = async () => {
      throw new Error('DB error');
    };
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    const result = await importer.run({ source: 'api', entities: ['customers'] });
    assert.equal(result.totalFailed, 1);
    assert.ok(result.entities.customers.errors[0].error.includes('DB error'));
  });

  it('throws for unknown source type', async () => {
    const adapter = new MockAdapter({ customers: [[{ id: 1 }]] });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    const result = await importer.run({ source: 'ftp', entities: ['customers'] });
    assert.equal(result.totalFailed, 1);
  });

  it('emits events during import', async () => {
    const adapter = new MockAdapter({ customers: [[{ id: 1 }]] });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    const events = [];
    importer.on('entity:start', (e) => events.push(['start', e]));
    importer.on('entity:complete', (e) => events.push(['complete', e]));
    importer.on('import:complete', (e) => events.push(['import:complete', e]));

    await importer.run({ source: 'api', entities: ['customers'] });

    assert.equal(events.length, 3);
    assert.equal(events[0][0], 'start');
    assert.equal(events[1][0], 'complete');
    assert.equal(events[2][0], 'import:complete');
  });

  it('stores last result', async () => {
    const adapter = new MockAdapter({ customers: [[{ id: 1 }]] });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    assert.equal(importer.getLastResult(), null);
    await importer.run({ source: 'api', entities: ['customers'] });
    const lastResult = importer.getLastResult();
    assert.ok(lastResult);
    assert.equal(lastResult.totalCreated, 1);
  });

  it('imports multiple entity types in correct order', async () => {
    const adapter = new MockAdapter({
      customers: [[{ id: 1, email: 'a@b.com' }]],
      products: [[{ id: 10, name: 'Widget' }]],
      orders: [[{ id: 100, total: 50 }]],
    });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    const result = await importer.run({
      source: 'api',
      entities: ['orders', 'products', 'customers'],
    });

    assert.equal(result.totalCreated, 3);
    assert.equal(result.success, true);
    assert.ok(result.entities.customers);
    assert.ok(result.entities.products);
    assert.ok(result.entities.orders);
  });

  it('handles multiple batches per entity', async () => {
    const adapter = new MockAdapter({
      customers: [
        [{ id: 1 }, { id: 2 }],
        [{ id: 3 }, { id: 4 }],
      ],
    });
    const commerce = createMockCommerce();
    const idMapStore = createMockIdMapStore();
    const importer = new DataImporter(adapter, commerce, idMapStore);

    const result = await importer.run({ source: 'api', entities: ['customers'] });
    assert.equal(result.totalCreated, 4);
    assert.equal(commerce._store.customers.length, 4);
  });
});
