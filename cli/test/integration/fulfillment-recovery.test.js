import { test } from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import Database from 'better-sqlite3';
import { Commerce } from '../../../bindings/node/index.js';
import { createEmbeddedAgentToolkit } from '../../src/agent-toolkit.js';
import { shipmentTools } from '../../src/tools/shipments.js';
import { selectStrictKernelToolDefinitions } from '../../src/kernel-boundary.js';

async function fixture(t) {
  const directory = mkdtempSync(join(tmpdir(), 'stateset-recovery-'));
  const dbPath = join(directory, 'store.db');
  const commerce = new Commerce(dbPath);
  const db = new Database(dbPath);
  t.after(() => {
    db.close();
    commerce.close();
    rmSync(directory, { recursive: true, force: true });
  });
  const customer = await commerce.customers.create({
    email: 'recovery@example.com',
    firstName: 'Ada',
    lastName: 'L',
  });
  const order = await commerce.orders.create({
    customerId: customer.id,
    items: [{ sku: 'W-1', name: 'Widget', quantity: 5, unitPriceExact: '12.34' }],
  });
  const shipment = await commerce.shipments.create({
    orderId: order.id,
    recipientName: 'Ada',
    shippingAddress: '1 Main',
    recipientEmail: 'recovery@example.com',
  });
  const options = {
    commerce,
    dbPath,
    policyStorePath: join(directory, 'policy.json'),
    capabilities: ['plan_partial_shipment', 'handle_fulfillment_exception'],
  };
  return {
    commerce,
    db,
    order,
    shipment,
    reader: createEmbeddedAgentToolkit(options),
    writer: createEmbeddedAgentToolkit({ ...options, allowApply: true }),
  };
}

test('native reads expose persisted shipment contents and partial fulfillment; planning has no commerce writes', async (t) => {
  const { commerce, db, order, shipment, reader, writer } = await fixture(t);
  const itemId = randomUUID();
  // Seed an imported partial-fulfillment snapshot. This tests binding reads, not fulfillment execution.
  db.transaction(() => {
    db.prepare('UPDATE order_items SET shipped_quantity = 2 WHERE id = ?').run(order.items[0].id);
    db.prepare(
      "UPDATE orders SET status = 'partially_shipped', version = version + 1 WHERE id = ?",
    ).run(order.id);
    db.prepare(
      `INSERT INTO shipment_items
      (id, shipment_id, order_item_id, sku, name, quantity, created_at, updated_at)
      VALUES (?, ?, ?, 'W-1', 'Widget', 2, ?, ?)`,
    ).run(itemId, shipment.id, order.items[0].id, shipment.createdAt, shipment.createdAt);
  })();
  const beforeOrder = await commerce.orders.get(order.id);
  const beforeShipment = await commerce.shipments.get(shipment.id);
  const outboxCount = () => db.prepare('SELECT COUNT(*) AS count FROM kernel_outbox').get().count;
  const beforeEvents = outboxCount();
  assert.equal(beforeOrder.items[0].shippedQuantity, 2);
  assert.equal(beforeShipment.recipientEmail, 'recovery@example.com');
  assert.equal(beforeShipment.items[0].id, itemId);
  assert.equal(beforeShipment.items[0].orderItemId, order.items[0].id);
  assert.equal(beforeShipment.items[0].quantity, 2);
  assert.deepEqual((await commerce.shipments.list())[0].items, beforeShipment.items);

  const input = { orderId: order.id, shipmentId: shipment.id };
  const result = await reader.executeTool('plan_partial_shipment', input);
  assert.equal(result.success, true, JSON.stringify(result));
  assert.equal(result.result.plan.items[0].quantity, 3);
  assert.equal(result.result.plan.executable, false);
  assert.equal(result.result.plan.orderVersion, beforeOrder.version);
  const compensation = {
    ...input,
    exceptionType: 'partial_shipment',
    autoExecuteCompensation: true,
  };
  assert.equal(
    (await reader.executeTool('handle_fulfillment_exception', compensation)).preview,
    true,
  );
  for (let replay = 0; replay < 2; replay++) {
    const blocked = await writer.executeTool('handle_fulfillment_exception', compensation);
    assert.equal(blocked.success, false, JSON.stringify(blocked));
  }
  assert.equal(await commerce.shipments.count(), 1);
  assert.deepEqual(await commerce.orders.get(order.id), beforeOrder);
  assert.deepEqual(await commerce.shipments.get(shipment.id), beforeShipment);
  assert.equal(outboxCount(), beforeEvents);
  const strict = selectStrictKernelToolDefinitions(shipmentTools);
  assert.ok(strict.some((tool) => tool.name === 'plan_partial_shipment'));
  assert.ok(!strict.some((tool) => tool.name === 'handle_fulfillment_exception'));
});

test('native order shipment updates the quantities used by subsequent recovery plans', async (t) => {
  const { commerce, order, reader } = await fixture(t);
  assert.equal(order.items[0].shippedQuantity, 0);
  await commerce.orders.updateStatus(order.id, 'confirmed');
  await commerce.orders.updateStatus(order.id, 'processing');
  const shipped = await commerce.orders.ship(order.id, 'TRACK');
  assert.equal(shipped.items[0].shippedQuantity, 5);
  const result = await reader.executeTool('plan_partial_shipment', { orderId: order.id });
  assert.equal(result.success, true, JSON.stringify(result));
  assert.equal(result.result.plan.status, 'nothing_to_fulfill');
  assert.deepEqual(result.result.plan.items, []);
});

test('public recovery planning rejects excess quantities and a shipment from another order', async (t) => {
  const { commerce, order, reader } = await fixture(t);
  const excess = await reader.executeTool('plan_partial_shipment', {
    orderId: order.id,
    remainingItems: [{ sku: 'W-1', quantity: 6 }],
  });
  assert.equal(excess.success, false);
  const other = await commerce.orders.create({
    customerId: order.customerId,
    items: [{ sku: 'OTHER', name: 'Other', quantity: 1, unitPriceExact: '1.00' }],
  });
  const foreign = await commerce.shipments.create({
    orderId: other.id,
    recipientName: 'Ada',
    shippingAddress: '1 Main',
  });
  const mismatch = await reader.executeTool('plan_partial_shipment', {
    orderId: order.id,
    shipmentId: foreign.id,
  });
  assert.equal(mismatch.success, false);
  assert.equal(await commerce.shipments.count(), 2);
});
