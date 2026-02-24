import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { execute } from '../../src/commands/inventory.js';

function buildProducts(count) {
  return [
    {
      name: 'Load Test Product',
      variants: Array.from({ length: count }, (_, i) => ({
        sku: `SKU-${String(i + 1).padStart(3, '0')}`,
        name: `Variant ${i + 1}`,
      })),
    },
  ];
}

describe('inventory command concurrency', () => {
  it('bounds stock lookups for list action', async () => {
    const products = buildProducts(24);
    let active = 0;
    let maxActive = 0;

    const commerce = {
      products: {
        list: async () => products,
      },
      inventory: {
        getStock: async () => {
          active += 1;
          maxActive = Math.max(maxActive, active);
          await new Promise((resolve) => setTimeout(resolve, 8));
          active -= 1;
          return {
            totalOnHand: 10,
            totalAvailable: 8,
            totalAllocated: 2,
          };
        },
      },
    };

    const result = await execute('list', [], {
      commerce,
      output: { table: () => '' },
      jsonOutput: true,
      resolveSku: async (sku) => sku,
    });

    assert.equal(Array.isArray(result), true);
    assert.equal(result.length, 24);
    assert.ok(maxActive <= 8, `Expected max concurrency <= 8, got ${maxActive}`);
  });
});
