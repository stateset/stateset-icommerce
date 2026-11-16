/**
 * `stateset-direct inventory list` lists inventory items, not product
 * variants: an item created with `inventory create` appears even when no
 * product carries its SKU. Bindings without `inventory.list` fall back to the
 * product walk (see inventory-command-concurrency.test.js).
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { Commerce } from '@stateset/embedded';
import { execute } from '../../src/commands/inventory.js';

const options = (commerce, jsonOutput = true) => ({
  commerce,
  output: { table: (rows) => rows.map((row) => row.sku).join('\n') },
  jsonOutput,
  resolveSku: async (sku) => sku,
});

describe('inventory list', () => {
  it('pages through inventory.list and never walks products', async () => {
    const calls = [];
    const items = Array.from({ length: 501 }, (_, i) => ({
      sku: `SKU-${String(i).padStart(4, '0')}`,
      name: `Item ${i}`,
      onHand: '5',
      allocated: '1',
      available: '4',
    }));
    const commerce = {
      products: {
        list: async () => assert.fail('the product walk must not run when inventory.list exists'),
      },
      inventory: {
        list: async (filter) => {
          calls.push(filter);
          return items.slice(filter.offset, filter.offset + filter.limit);
        },
      },
    };
    const result = await execute('list', [], options(commerce));
    assert.equal(result.length, 501);
    assert.deepEqual(result[500], {
      sku: 'SKU-0500',
      name: 'Item 500',
      onHand: '5',
      allocated: '1',
      available: '4',
    });
    assert.deepEqual(calls, [
      { limit: 500, offset: 0 },
      { limit: 500, offset: 500 },
    ]);
  });

  it('lists an item created with inventory create on the real engine', async (t) => {
    const dir = mkdtempSync(path.join(os.tmpdir(), 'inventory-list-'));
    const commerce = new Commerce(path.join(dir, 'store.db'));
    t.after(() => {
      commerce.close?.();
      rmSync(dir, { recursive: true, force: true });
    });
    assert.equal(typeof commerce.inventory.list, 'function', 'this binding exposes inventory.list');
    await execute('create', ['LOOSE-BOLT', 'Loose bolt', '12'], options(commerce));
    assert.deepEqual(await commerce.products.list(), [], 'no product carries the SKU');
    const listed = await execute('list', [], options(commerce));
    assert.deepEqual(listed, [
      { sku: 'LOOSE-BOLT', name: 'Loose bolt', onHand: '12', allocated: '0', available: '12' },
    ]);
    const table = await execute('list', [], options(commerce, false));
    assert.match(table.formatted, /LOOSE-BOLT/);
  });
});
