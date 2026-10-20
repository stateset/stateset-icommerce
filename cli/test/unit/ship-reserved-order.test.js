import assert from 'node:assert/strict';
import { test } from 'node:test';
import { z } from 'zod';
import { orderTools } from '../../src/tools/orders.js';
import { createKernelToolExecutor } from '../../src/kernel-tool-execution.js';
import { selectStrictKernelToolDefinitions } from '../../src/kernel-boundary.js';
import { shipmentKernel } from '../fixtures/tui-shipments.js';
const tool = orderTools.find((tool) => tool.name === 'ship_reserved_order');
const params = {
  orderId: 'b734166a-c7c8-42ae-b991-eb1974a3f13f',
  expectedVersion: 3,
  trackingNumber: 'TRACK',
  lines: [{ orderItemId: 'e29e51a4-299c-4f0f-b738-f8adad0b7964', quantity: 1 }],
  idempotencyKey: 'dispatch:one',
};
test('reserved shipping is strictly governed, previews without mutation and has no legacy fallback', async () => {
  assert.ok(selectStrictKernelToolDefinitions(orderTools).includes(tool));
  const commerce = {
    orders: {
      ship: () => {
        throw new Error('legacy shipment must never run');
      },
    },
  };
  assert.deepEqual((await tool.handler({ params, allowApply: false })).wouldShip, params);
  await assert.rejects(tool.handler({ params, commerce, allowApply: true }), /trusted kernel/);
  await assert.rejects(
    createKernelToolExecutor({ commerce, allowApply: true })('ship_reserved_order', params),
    /trusted kernel/,
  );
});
test('agent shipping forwards partial lines, reviewed version and retry key with host-owned scope', async () => {
  const commands = [];
  const commerce = {
    executeKernelCommand: async (command) => {
      commands.push(command);
      return {
        status: command.mode === 'apply' ? 'succeeded' : 'previewed',
        result: { id: params.orderId },
      };
    },
  };
  const execute = createKernelToolExecutor({
    commerce,
    kernel: shipmentKernel(),
    allowApply: true,
  });
  await execute('ship_reserved_order', { ...params, principal: { id: 'evil' }, storeId: 'evil' });
  assert.equal(commands[0].command_type, 'orders.ship_reserved');
  assert.equal(commands[0].expected_version, 3);
  assert.equal(commands[0].idempotency_key, params.idempotencyKey);
  assert.equal(commands[0].principal.id, 'user:tui');
  assert.equal(commands[0].store_id, 'store:tui');
  assert.deepEqual(commands[0].payload.lines, [
    { order_item_id: params.lines[0].orderItemId, quantity: 1 },
  ]);
  await execute('ship_reserved_order', params, { expectedVersion: 4 });
  assert.equal(commands[1].expected_version, 4);
});
test('agent shipping schema refuses unsafe versions, non-whole units and empty line selections', () => {
  const schema = z.object(tool.inputSchema);
  assert.deepEqual(schema.parse(params), params);
  for (const change of [
    { expectedVersion: -1 },
    { expectedVersion: 1.1 },
    { expectedVersion: Number.MAX_SAFE_INTEGER },
    { lines: [] },
    { lines: [{ ...params.lines[0], quantity: 0 }] },
    { lines: [{ ...params.lines[0], quantity: 1.5 }] },
  ])
    assert.equal(schema.safeParse({ ...params, ...change }).success, false);
});
test('get_order exposes native version and shipped quantities needed to plan the next dispatch', async () => {
  const order = {
    id: params.orderId,
    version: 7,
    items: [{ id: params.lines[0].orderItemId, quantity: 2, shippedQuantity: 1 }],
  };
  const result = await orderTools
    .find((tool) => tool.name === 'get_order')
    .handler({
      commerce: { orders: { get: async () => order } },
      params: { identifier: params.orderId },
    });
  assert.equal(result.order.version, 7);
  assert.equal(result.order.items[0].shippedQuantity, 1);
});
