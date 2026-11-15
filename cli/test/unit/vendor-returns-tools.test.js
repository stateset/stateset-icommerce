import { test } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';
import { vendorReturnTools } from '../../src/tools/vendor-returns.js';
const tool = (name) => vendorReturnTools.find((t) => t.name === name);
test('vendor returns require stock routing and preserve exact amounts behind apply', async () => {
  const create = tool('create_vendor_return');
  const input = {
    supplierId: 'supplier',
    warehouseId: 2,
    items: [
      {
        productId: 'product',
        sku: 'SKU',
        sourceLocationId: 3,
        quantity: '0.125',
        unitCost: '100000000000.25',
      },
    ],
  };
  const schema = z.object(create.inputSchema);
  assert.equal(schema.safeParse({ ...input, warehouseId: undefined }).success, false);
  assert.equal(
    schema.safeParse({ ...input, items: [{ ...input.items[0], sourceLocationId: undefined }] })
      .success,
    false,
  );
  let calls = 0;
  const commerce = {
    vendorReturns: {
      create: async (data) => {
        calls++;
        assert.deepEqual(data.items, input.items);
        assert.equal(data.warehouseId, 2);
        return data;
      },
    },
  };
  await create.handler({ commerce, params: schema.parse(input), allowApply: false });
  assert.equal(calls, 0);
  await create.handler({ commerce, params: schema.parse(input), allowApply: true });
  assert.equal(calls, 1);
});
test('processing always includes its credit and remains preview gated', async () => {
  const process = tool('process_vendor_return');
  const schema = z.object(process.inputSchema);
  assert.equal(schema.safeParse({ id: 'return', generateCredit: false }).success, false);
  const params = schema.parse({ id: 'return' });
  let calls = 0;
  const commerce = {
    vendorReturns: {
      process: async (id, credit) => {
        calls++;
        assert.equal(id, 'return');
        assert.equal(credit, true);
        return { id };
      },
    },
  };
  await process.handler({ commerce, params, allowApply: false });
  assert.equal(calls, 0);
  await process.handler({ commerce, params, allowApply: true });
  assert.equal(calls, 1);
});
