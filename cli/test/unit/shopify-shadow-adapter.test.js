import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { getAdapter, listAdapters } from '../../src/adapters/index.js';

describe('shopify-shadow adapter', () => {
  it('is registered in adapter registry', () => {
    const adapters = listAdapters();
    assert.ok(adapters.includes('shopify-shadow'));
  });

  it('exposes expected entity support and import order', async () => {
    const adapter = await getAdapter('shopify-shadow', {});
    assert.equal(adapter.platformName, 'shopify-shadow');
    assert.deepEqual(adapter.getSupportedEntities(), [
      'customers',
      'products',
      'inventory',
      'orders',
      'fulfillments',
    ]);
    assert.deepEqual(adapter.getImportOrder(), [
      'customers',
      'products',
      'inventory',
      'orders',
      'fulfillments',
    ]);
  });
});
