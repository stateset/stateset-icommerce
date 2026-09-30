import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Commerce } from '../../../bindings/node/index.js';
import { createEmbeddedAgentToolkit } from '../../src/agent-toolkit.js';
import { shipmentTools } from '../../src/tools/shipments.js';
import { selectStrictKernelToolDefinitions } from '../../src/kernel-boundary.js';

const writes = [
  'create_shipment',
  'update_shipment',
  'ship_shipment',
  'deliver_shipment',
  'get_shipment',
];
async function fixture(t) {
  const directory = mkdtempSync(join(tmpdir(), 'stateset-shipment-tools-'));
  const dbPath = join(directory, 'store.db');
  const commerce = new Commerce(dbPath);
  t.after(() => {
    commerce.close();
    rmSync(directory, { recursive: true, force: true });
  });
  const customer = await commerce.customers.create({
    email: 'shipment@example.com',
    firstName: 'Ada',
    lastName: 'L',
  });
  const order = await commerce.orders.create({
    customerId: customer.id,
    items: [{ sku: 'W-1', name: 'Widget', quantity: 1, unitPrice: 1 }],
  });
  const options = {
    commerce,
    dbPath,
    policyStorePath: join(directory, 'policy.json'),
    capabilities: writes,
  };
  return {
    commerce,
    input: {
      orderId: order.id,
      recipientName: 'Ada',
      shippingAddress: '1 Main',
      carrier: 'UPS',
      service: 'Ground',
    },
    preview: createEmbeddedAgentToolkit(options),
    applied: createEmbeddedAgentToolkit({ ...options, allowApply: true }),
    admin: createEmbeddedAgentToolkit({
      ...options,
      allowApply: true,
      capabilities: [...writes, 'cancel_shipment'],
    }),
  };
}
function shipment(result) {
  assert.equal(result.success, true, JSON.stringify(result));
  return result.result.shipment;
}

test('MCP-backed toolkit completes native shipment lifecycle with preview and version checks', async (t) => {
  const { commerce, input, preview, applied } = await fixture(t);
  assert.equal((await preview.executeTool('create_shipment', input)).preview, true);
  assert.equal(await commerce.shipments.count(), 0);
  const initial = shipment(await applied.executeTool('create_shipment', input));
  assert.equal(initial.shippingMethod, 'ground');
  assert.equal(initial.recipientName, 'Ada');
  assert.equal(initial.shippingAddress, '1 Main');
  const shipmentId = initial.id;
  assert.equal(
    (await preview.executeTool('update_shipment', { shipmentId, status: 'processing' })).preview,
    true,
  );
  assert.equal((await commerce.shipments.get(shipmentId)).version, 1);
  const invalid = await applied.executeTool('deliver_shipment', { shipmentId });
  assert.equal(invalid.success, false);
  assert.equal((await commerce.shipments.get(shipmentId)).version, 1);
  shipment(
    await applied.executeTool('update_shipment', {
      shipmentId,
      status: 'processing',
      expectedVersion: 1,
    }),
  );
  const stale = await applied.executeTool('update_shipment', {
    shipmentId,
    notes: 'stale',
    expectedVersion: 1,
  });
  assert.equal(stale.success, false);
  shipment(
    await applied.executeTool('update_shipment', {
      shipmentId,
      status: 'ready_to_ship',
      expectedVersion: 2,
    }),
  );
  const shipped = shipment(
    await applied.executeTool('ship_shipment', {
      shipmentId,
      trackingNumber: 'TRACK',
      expectedVersion: 3,
    }),
  );
  assert.equal(shipped.version, 4);
  assert.match(shipped.trackingUrl, /ups.com/);
  shipment(
    await applied.executeTool('update_shipment', {
      shipmentId,
      status: 'in_transit',
      expectedVersion: 4,
    }),
  );
  shipment(
    await applied.executeTool('update_shipment', {
      shipmentId,
      status: 'out_for_delivery',
      expectedVersion: 5,
    }),
  );
  const delivered = shipment(
    await applied.executeTool('deliver_shipment', { shipmentId, expectedVersion: 6 }),
  );
  assert.equal(delivered.version, 7);
  assert.equal(delivered.status, 'delivered');
  const replay = shipment(
    await applied.executeTool('deliver_shipment', { shipmentId, expectedVersion: 7 }),
  );
  assert.deepEqual(replay, delivered);
});

test('shipment cancellation retains its separate capability and strict kernel exposure stays closed', async (t) => {
  const { commerce, input, applied, admin } = await fixture(t);
  const initial = shipment(await applied.executeTool('create_shipment', input));
  for (const status of ['cancelled', 'canceled', 'CANCELLED']) {
    const result = await applied.executeTool('update_shipment', { shipmentId: initial.id, status });
    assert.equal(result.success, false);
  }
  await assert.rejects(
    applied.executeTool('cancel_shipment', { shipmentId: initial.id }),
    /capability scope/,
  );
  assert.deepEqual(await commerce.shipments.get(initial.id), initial);
  const cancelled = shipment(
    await admin.executeTool('cancel_shipment', { shipmentId: initial.id, expectedVersion: 1 }),
  );
  assert.equal(cancelled.status, 'cancelled');
  assert.equal(cancelled.version, 2);
  const exposed = selectStrictKernelToolDefinitions(shipmentTools);
  assert.equal(
    exposed.some((tool) => tool.name === 'update_shipment'),
    false,
  );
  assert.equal(
    exposed.some((tool) => tool.name === 'get_shipment'),
    true,
  );
});
