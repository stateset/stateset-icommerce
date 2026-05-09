// Unit tests for cli/src/mcp/auto-index.js
//
// Covers:
//  - No-ops when vectorAutoIndex is missing or entity is missing/idless
//  - Routes product/customer/order to the right index method
//  - Calls index method with entity.id stringified (numeric ids → string)
//  - Unsupported entityType → no-op (no error thrown)
//  - Indexer rejection is swallowed + logged (does not throw)

import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';

import { autoIndexEntity } from '../../src/mcp/auto-index.js';

function makeIndexer({ failProduct = false } = {}) {
  return {
    indexProduct: mock.fn(() =>
      failProduct
        ? Promise.reject(new Error('product index boom'))
        : Promise.resolve('p-ok'),
    ),
    indexCustomer: mock.fn(() => Promise.resolve('c-ok')),
    indexOrder: mock.fn(() => Promise.resolve('o-ok')),
  };
}

describe('autoIndexEntity', () => {
  it('no-ops when vectorAutoIndex is null/undefined', () => {
    // Should not throw.
    assert.doesNotThrow(() =>
      autoIndexEntity(null, 'product', { id: 'p1' }),
    );
    assert.doesNotThrow(() =>
      autoIndexEntity(undefined, 'product', { id: 'p1' }),
    );
  });

  it('no-ops when entity is null/undefined', () => {
    const indexer = makeIndexer();
    autoIndexEntity(indexer, 'product', null);
    autoIndexEntity(indexer, 'product', undefined);
    assert.equal(indexer.indexProduct.mock.callCount(), 0);
  });

  it('no-ops when entity.id is missing or falsy', () => {
    const indexer = makeIndexer();
    autoIndexEntity(indexer, 'product', {});
    autoIndexEntity(indexer, 'product', { id: '' });
    autoIndexEntity(indexer, 'product', { id: null });
    autoIndexEntity(indexer, 'product', { id: 0 });
    assert.equal(indexer.indexProduct.mock.callCount(), 0);
  });

  it('routes "product" entityType to indexProduct with stringified id', () => {
    const indexer = makeIndexer();
    autoIndexEntity(indexer, 'product', { id: 'prod_123' });
    assert.equal(indexer.indexProduct.mock.callCount(), 1);
    assert.deepEqual(indexer.indexProduct.mock.calls[0].arguments, ['prod_123']);
    assert.equal(indexer.indexCustomer.mock.callCount(), 0);
    assert.equal(indexer.indexOrder.mock.callCount(), 0);
  });

  it('routes "customer" entityType to indexCustomer', () => {
    const indexer = makeIndexer();
    autoIndexEntity(indexer, 'customer', { id: 'cust_42' });
    assert.equal(indexer.indexCustomer.mock.callCount(), 1);
    assert.deepEqual(indexer.indexCustomer.mock.calls[0].arguments, ['cust_42']);
  });

  it('routes "order" entityType to indexOrder', () => {
    const indexer = makeIndexer();
    autoIndexEntity(indexer, 'order', { id: 'ord_99' });
    assert.equal(indexer.indexOrder.mock.callCount(), 1);
    assert.deepEqual(indexer.indexOrder.mock.calls[0].arguments, ['ord_99']);
  });

  it('stringifies numeric ids before passing to the indexer', () => {
    const indexer = makeIndexer();
    autoIndexEntity(indexer, 'product', { id: 12345 });
    assert.deepEqual(indexer.indexProduct.mock.calls[0].arguments, ['12345']);
  });

  it('no-ops on unsupported entityType (does not throw)', () => {
    const indexer = makeIndexer();
    assert.doesNotThrow(() =>
      autoIndexEntity(indexer, 'shipment', { id: 'ship_1' }),
    );
    assert.equal(indexer.indexProduct.mock.callCount(), 0);
    assert.equal(indexer.indexCustomer.mock.callCount(), 0);
    assert.equal(indexer.indexOrder.mock.callCount(), 0);
  });

  it('swallows indexer rejections (best-effort enrichment)', async () => {
    const indexer = makeIndexer({ failProduct: true });
    // Capture console.error so the rejection logs are quiet during test runs.
    const errSpy = mock.method(console, 'error', () => {});
    try {
      autoIndexEntity(indexer, 'product', { id: 'prod_1' });
      // The rejection is async — wait a microtask cycle for `.catch` to fire.
      await new Promise((resolve) => setImmediate(resolve));
      assert.equal(errSpy.mock.callCount(), 1);
      assert.match(errSpy.mock.calls[0].arguments[0], /\[AutoIndex\] Failed to index product prod_1/);
      assert.match(errSpy.mock.calls[0].arguments[0], /product index boom/);
    } finally {
      errSpy.mock.restore();
    }
  });
});
