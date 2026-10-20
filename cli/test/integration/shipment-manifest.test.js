import { test } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { sqliteOperations, sqliteGet, sqliteExec } from '../helpers/sqlite-process.js';
import { Commerce } from '../../../bindings/node/index.js';
import { createNativeToolkit } from '../../../bindings/node/native-toolkit.mjs';
import { createEmbeddedAgentToolkit } from '../../src/agent-toolkit.js';

async function fixture(t, { duplicateSku = false } = {}) {
  const directory = mkdtempSync(join(tmpdir(), 'stateset-manifest-'));
  const dbPath = join(directory, 'store.db');
  let commerce = new Commerce(dbPath);
  t.after(async () => {
    await commerce.close();
    rmSync(directory, { recursive: true, force: true });
  });
  const customer = await commerce.customers.create({
    email: 'manifest@example.com',
    firstName: 'Ada',
    lastName: 'L',
  });
  const line = { sku: 'W-1', name: 'Widget', quantity: 4, unitPriceExact: '19.99' };
  const order = await commerce.orders.create({
    customerId: customer.id,
    items: duplicateSku ? [line, line] : [line],
  });
  const input = {
    orderId: order.id,
    recipientName: 'Ada',
    shippingAddress: '1 Main',
    shippingMethod: 'ground',
  };
  const item = { orderItemId: order.items[0].id, sku: 'W-1', name: 'Widget', quantity: 2 };
  const snapshot = () => {
    const [shipments, items, facts] = sqliteOperations(dbPath, [
      { mode: 'all', sql: 'SELECT * FROM shipments ORDER BY id' },
      { mode: 'all', sql: 'SELECT * FROM shipment_items ORDER BY id' },
      {
        mode: 'all',
        sql: "SELECT * FROM kernel_outbox WHERE aggregate_type = 'shipment' ORDER BY id",
      },
    ]);
    return { shipments, items, facts };
  };
  return {
    commerce,
    dbPath,
    directory,
    customer,
    order,
    input,
    item,
    snapshot,
    reopen: async () => {
      await commerce.close();
      commerce = new Commerce(dbPath);
      return commerce;
    },
  };
}

test('Node creates and edits order-linked manifests with durable facts and packing guards', async (t) => {
  const f = await fixture(t);
  let commerce = f.commerce;
  const shipment = await commerce.shipments.create({ ...f.input, items: [f.item] });
  assert.equal(shipment.items[0].orderItemId, f.item.orderItemId);
  assert.equal(
    shipment.items[0].productId,
    sqliteGet(f.dbPath, 'SELECT product_id FROM order_items WHERE id = ?', f.item.orderItemId)
      .product_id,
  );
  const added = await commerce.shipments.addItem(shipment.id, { ...f.item, quantity: 1 });
  assert.equal((await commerce.shipments.get(shipment.id)).version, 2);
  await commerce.shipments.removeItem(added.id);
  const packed = await commerce.shipments.get(shipment.id);
  assert.equal(packed.version, 3);
  assert.deepEqual(packed.items, shipment.items);
  const facts = f.snapshot().facts;
  assert.deepEqual(
    facts.map((row) => row.event_type).sort(),
    ['shipments.created.v1', 'shipments.item_added.v1', 'shipments.item_removed.v1'].sort(),
  );
  const created = JSON.parse(
    facts.find((row) => row.event_type === 'shipments.created.v1').payload,
  );
  assert.equal(created.items[0].order_item_id, f.item.orderItemId);
  assert.equal(created.items[0].quantity, 2);
  await commerce.shipments.update(shipment.id, { status: 'processing', expectedVersion: 3 });
  await commerce.shipments.update(shipment.id, { status: 'ready_to_ship', expectedVersion: 4 });
  const before = f.snapshot();
  await assert.rejects(
    commerce.shipments.addItem(shipment.id, { ...f.item, quantity: 1 }),
    /Cannot change items/,
  );
  await assert.rejects(commerce.shipments.removeItem(shipment.items[0].id), /Cannot change items/);
  assert.deepEqual(f.snapshot(), before);
  commerce = await f.reopen();
  assert.equal((await commerce.shipments.get(shipment.id)).items[0].quantity, 2);
  const order = await commerce.orders.get(f.order.id);
  assert.equal(order.items[0].shippedQuantity, 0);
});

test('Node refuses fractional, overflowing and malformed item inputs without partial creation', async (t) => {
  const f = await fixture(t);
  const before = f.snapshot();
  for (const quantity of [0, -1, 0.5, 1.5, NaN, Infinity, 2147483648, 4294967297]) {
    await assert.rejects(
      f.commerce.shipments.create({ ...f.input, items: [f.item, { ...f.item, quantity }] }),
      /positive integer/,
    );
    assert.deepEqual(f.snapshot(), before);
  }
  for (const field of ['orderItemId', 'productId']) {
    await assert.rejects(
      f.commerce.shipments.create({ ...f.input, items: [{ ...f.item, [field]: 'invalid' }] }),
      /Invalid .* UUID/,
    );
    assert.deepEqual(f.snapshot(), before);
  }
});

test('Node uses native order membership, product and cumulative quantity guards', async (t) => {
  const f = await fixture(t);
  const other = await f.commerce.orders.create({
    customerId: f.customer.id,
    items: [{ sku: 'W-1', name: 'Widget', quantity: 4, unitPriceExact: '19.99' }],
  });
  for (const item of [
    { ...f.item, orderItemId: other.items[0].id },
    { ...f.item, productId: crypto.randomUUID() },
    { ...f.item, sku: 'wrong' },
  ]) {
    const before = f.snapshot();
    await assert.rejects(
      f.commerce.shipments.create({ ...f.input, items: [item] }),
      /belong|match/,
    );
    assert.deepEqual(f.snapshot(), before);
  }
  const first = await f.commerce.shipments.create({
    ...f.input,
    items: [{ ...f.item, quantity: 3 }],
  });
  const before = f.snapshot();
  await assert.rejects(
    f.commerce.shipments.create({ ...f.input, items: [f.item] }),
    /exceed ordered quantity/,
  );
  await assert.rejects(f.commerce.shipments.addItem(first.id, f.item), /exceed ordered quantity/);
  assert.deepEqual(f.snapshot(), before);
  await f.commerce.shipments.cancel(first.id);
  const replacement = await f.commerce.shipments.create({
    ...f.input,
    items: [{ ...f.item, quantity: 4 }],
  });
  assert.equal(replacement.items[0].quantity, 4);
});

test('ambiguous SKUs require an explicit order line', async (t) => {
  const f = await fixture(t, { duplicateSku: true });
  const { orderItemId, ...item } = f.item;
  await assert.rejects(f.commerce.shipments.create({ ...f.input, items: [item] }), /Ambiguous/);
  assert.equal(await f.commerce.shipments.count(), 0);
  const shipment = await f.commerce.shipments.create({
    ...f.input,
    items: [{ ...item, orderItemId }],
  });
  assert.equal(shipment.items[0].orderItemId, orderItemId);
});

test('concurrent native creators cannot allocate the same remaining order quantity twice', async (t) => {
  const f = await fixture(t);
  const second = new Commerce(f.dbPath);
  t.after(() => second.close());
  const input = { ...f.input, items: [{ ...f.item, quantity: 4 }] };
  const results = await Promise.allSettled([
    f.commerce.shipments.create(input),
    second.shipments.create(input),
  ]);
  assert.equal(results.filter((result) => result.status === 'fulfilled').length, 1);
  assert.equal(results.filter((result) => result.status === 'rejected').length, 1);
  assert.equal(await f.commerce.shipments.count(), 1);
  assert.equal(
    sqliteGet(f.dbPath, 'SELECT SUM(quantity) AS quantity FROM shipment_items').quantity,
    4,
  );
  assert.equal(f.snapshot().facts.length, 1);
});

test('manifest writes roll back when the durable outbox refuses their fact', async (t) => {
  const f = await fixture(t);
  const trigger =
    "CREATE TRIGGER refuse_manifest_fact BEFORE INSERT ON kernel_outbox WHEN NEW.aggregate_type = 'shipment' BEGIN SELECT RAISE(ABORT, 'manifest fact unavailable'); END";
  sqliteExec(f.dbPath, trigger);
  const empty = f.snapshot();
  await assert.rejects(
    f.commerce.shipments.create({ ...f.input, items: [f.item] }),
    /manifest fact unavailable/,
  );
  assert.deepEqual(f.snapshot(), empty);
  sqliteExec(f.dbPath, 'DROP TRIGGER refuse_manifest_fact');
  const shipment = await f.commerce.shipments.create({ ...f.input, items: [f.item] });
  sqliteExec(f.dbPath, trigger);
  const before = f.snapshot();
  await assert.rejects(
    f.commerce.shipments.addItem(shipment.id, { ...f.item, quantity: 1 }),
    /manifest fact unavailable/,
  );
  await assert.rejects(
    f.commerce.shipments.removeItem(shipment.items[0].id),
    /manifest fact unavailable/,
  );
  assert.deepEqual(f.snapshot(), before);
});

test('generated native item tools keep writes preview-only until apply is enabled', async (t) => {
  const f = await fixture(t);
  const shipment = await f.commerce.shipments.create(f.input);
  const preview = createNativeToolkit(f.commerce);
  const applied = createNativeToolkit(f.commerce, { allowApply: true });
  const params = { shipmentId: shipment.id, input: f.item };
  assert.equal((await preview.executeTool('shipments.addItem', params)).preview, true);
  assert.equal((await f.commerce.shipments.get(shipment.id)).items.length, 0);
  const result = await applied.executeTool('shipments.addItem', params);
  assert.equal(result.orderItemId, f.item.orderItemId);
  const item = (await f.commerce.shipments.get(shipment.id)).items[0];
  assert.equal(
    (await preview.executeTool('shipments.removeItem', { itemId: item.id })).preview,
    true,
  );
  assert.equal((await f.commerce.shipments.get(shipment.id)).items.length, 1);
  await applied.executeTool('shipments.removeItem', { itemId: item.id });
  assert.equal((await f.commerce.shipments.get(shipment.id)).items.length, 0);
});

for (const strict of [false, true]) {
  test(`shipment tool preserves manifest items, preview and native limits (strict=${strict})`, async (t) => {
    const f = await fixture(t);
    const capability = 'shipments.create';
    const kernel = strict
      ? {
          strict: true,
          storeId: 'store:test',
          principal: {
            id: 'agent:packing',
            kind: 'agent',
            tenantId: 'tenant:test',
            delegatedBy: 'user:test',
            capabilities: [capability],
          },
          policy: {
            version: 'manifest-v1',
            commands: {
              [capability]: {
                required_capabilities: [capability],
                requires_approval: false,
                requires_tenant: true,
                requires_store: true,
                allowed_tenant_ids: ['tenant:test'],
                allowed_store_ids: ['store:test'],
                requires_agent_delegation: true,
                requires_signed_authority: false,
              },
            },
            trusted_authority_keys: {},
          },
        }
      : undefined;
    const options = {
      commerce: f.commerce,
      dbPath: f.dbPath,
      policyStorePath: join(f.directory, 'policy.json'),
      capabilities: ['create_shipment', capability],
      kernel,
    };
    const preview = createEmbeddedAgentToolkit(options);
    const applied = createEmbeddedAgentToolkit({ ...options, allowApply: true });
    const params = { ...f.input, items: [{ ...f.item, quantity: 4 }] };
    await preview.executeTool('create_shipment', params);
    assert.equal(await f.commerce.shipments.count(), 0);
    const result = await applied.executeTool('create_shipment', params, {
      idempotencyKey: 'manifest-create',
    });
    assert.equal(result.success, true, JSON.stringify(result));
    if (strict) assert.equal(result.result.receipt.status, 'succeeded', JSON.stringify(result));
    const shipments = await f.commerce.shipments.list();
    assert.equal(shipments.length, 1);
    assert.equal(shipments[0].shippingMethod, 'ground');
    assert.equal(shipments[0].items[0].orderItemId, f.item.orderItemId);
    assert.equal(shipments[0].items[0].quantity, 4);
    if (strict) {
      const replay = await applied.executeTool('create_shipment', params, {
        idempotencyKey: 'manifest-create',
      });
      assert.equal(replay.result.receipt.status, 'succeeded');
      assert.equal(await f.commerce.shipments.count(), 1);
    }
    const rejected = await applied.executeTool('create_shipment', params, {
      idempotencyKey: 'manifest-overallocate',
    });
    assert.equal(
      rejected.success && rejected.result?.success !== false,
      false,
      JSON.stringify(rejected),
    );
    assert.equal(await f.commerce.shipments.count(), 1);
  });
}

test('shipment update refuses lossy JavaScript version inputs before mutation', async (t) => {
  const f = await fixture(t);
  const shipment = await f.commerce.shipments.create(f.input);
  const before = f.snapshot();
  for (const expectedVersion of [
    1.5,
    4294967297,
    0,
    -1,
    NaN,
    Infinity,
    -Infinity,
    2147483648,
    Number.MAX_SAFE_INTEGER,
  ]) {
    await assert.rejects(
      f.commerce.shipments.update(shipment.id, { notes: 'must not persist', expectedVersion }),
      /positive integer at most 2147483647/,
    );
    assert.deepEqual(f.snapshot(), before);
  }
  for (const expectedVersion of ['1', true, {}, 1n, null]) {
    await assert.rejects(async () =>
      f.commerce.shipments.update(shipment.id, { notes: 'must not persist', expectedVersion }),
    );
    assert.deepEqual(f.snapshot(), before);
  }
});

test('versioned Node packing edits reject stale reads without changing items or facts', async (t) => {
  const f = await fixture(t);
  const shipment = await f.commerce.shipments.create(f.input);
  const item = await f.commerce.shipments.addItem(shipment.id, f.item, shipment.version);
  const before = f.snapshot();
  for (const version of [1, 2147483647]) {
    await assert.rejects(f.commerce.shipments.addItem(shipment.id, f.item, version), {
      code: 'CONFLICT',
    });
    await assert.rejects(f.commerce.shipments.removeItem(item.id, version), { code: 'CONFLICT' });
    assert.deepEqual(f.snapshot(), before);
  }
  const reopened = await f.reopen();
  const current = await reopened.shipments.get(shipment.id);
  assert.equal(current.version, 2);
  await reopened.shipments.removeItem(item.id, current.version);
  assert.equal((await reopened.shipments.get(shipment.id)).version, 3);
  assert.equal((await reopened.shipments.get(shipment.id)).items.length, 0);
  assert.equal(f.snapshot().facts.length, 3, JSON.stringify(f.snapshot()));
  const unconditional = await reopened.shipments.addItem(shipment.id, f.item, null);
  await reopened.shipments.removeItem(unconditional.id);
  await reopened.shipments.update(shipment.id, {
    notes: 'unconditional',
    expectedVersion: undefined,
  });
  assert.equal((await reopened.shipments.get(shipment.id)).version, 6);
  assert.equal(f.snapshot().facts.length, 6);
});

test('packing and lifecycle methods validate numbers before native version conversion', async (t) => {
  const f = await fixture(t);
  const shipment = await f.commerce.shipments.create({ ...f.input, items: [f.item] });
  const before = f.snapshot();
  const operations = [
    (version) => f.commerce.shipments.addItem(shipment.id, f.item, version),
    (version) => f.commerce.shipments.removeItem(shipment.items[0].id, version),
    (version) => f.commerce.shipments.ship(shipment.id, 'must-not-persist', version),
    (version) => f.commerce.shipments.deliver(shipment.id, version),
    (version) => f.commerce.shipments.cancel(shipment.id, version),
  ];
  for (const operation of operations) {
    for (const version of [
      1.5,
      4294967297,
      0,
      -1,
      NaN,
      Infinity,
      -Infinity,
      2147483648,
      Number.MAX_SAFE_INTEGER,
    ]) {
      await assert.rejects(operation(version), { code: 'VALIDATION' });
      assert.deepEqual(f.snapshot(), before);
    }
    for (const version of ['1', true, {}, 1n]) {
      await assert.rejects(async () => operation(version));
      assert.deepEqual(f.snapshot(), before);
    }
  }
});

test('versioned lifecycle convenience methods preserve transitions, tracking and stale refusal', async (t) => {
  const f = await fixture(t);
  const shipment = await f.commerce.shipments.create({ ...f.input, items: [f.item] });
  await f.commerce.shipments.update(shipment.id, { status: 'processing', expectedVersion: 1 });
  await f.commerce.shipments.update(shipment.id, { status: 'ready_to_ship', expectedVersion: 2 });
  const beforeShip = f.snapshot();
  await assert.rejects(f.commerce.shipments.ship(shipment.id, 'stale', 2), { code: 'CONFLICT' });
  assert.deepEqual(f.snapshot(), beforeShip);
  const shipped = await f.commerce.shipments.ship(shipment.id, 'TRACK-VERSIONED', 3);
  assert.equal(shipped.version, 4);
  assert.equal(shipped.trackingNumber, 'TRACK-VERSIONED');
  const beforeDeliver = f.snapshot();
  await assert.rejects(
    f.commerce.shipments.deliver(shipment.id, 4),
    /Invalid shipment status transition/,
  );
  assert.deepEqual(f.snapshot(), beforeDeliver);
  await f.commerce.shipments.update(shipment.id, { status: 'in_transit', expectedVersion: 4 });
  await f.commerce.shipments.update(shipment.id, {
    status: 'out_for_delivery',
    expectedVersion: 5,
  });
  const ready = f.snapshot();
  await assert.rejects(f.commerce.shipments.deliver(shipment.id, 5), { code: 'CONFLICT' });
  assert.deepEqual(f.snapshot(), ready);
  const delivered = await f.commerce.shipments.deliver(shipment.id, 6);
  assert.equal(delivered.status, 'delivered');
  assert.equal(delivered.version, 7);
  const cancelled = await f.commerce.shipments.create(f.input);
  const beforeCancel = f.snapshot();
  await assert.rejects(f.commerce.shipments.cancel(cancelled.id, 2), { code: 'CONFLICT' });
  assert.deepEqual(f.snapshot(), beforeCancel);
  assert.equal((await f.commerce.shipments.cancel(cancelled.id, 1)).version, 2);
  assert.equal((await f.commerce.orders.get(f.order.id)).items[0].shippedQuantity, 0);
});

test('separate Node handles serialize competing versioned edits with spare allocation capacity', async (t) => {
  const f = await fixture(t);
  const peer = new Commerce(f.dbPath);
  t.after(() => peer.close());
  const shipment = await f.commerce.shipments.create(f.input);
  const item = { ...f.item, quantity: 1 };
  let results = await Promise.allSettled([
    f.commerce.shipments.addItem(shipment.id, item, 1),
    peer.shipments.addItem(shipment.id, item, 1),
  ]);
  assert.equal(results.filter((result) => result.status === 'fulfilled').length, 1);
  assert.equal(results.filter((result) => result.reason?.code === 'CONFLICT').length, 1);
  let current = await f.commerce.shipments.get(shipment.id);
  assert.equal(current.version, 2);
  assert.equal(current.items.length, 1);
  results = await Promise.allSettled([
    f.commerce.shipments.addItem(shipment.id, item, 2),
    peer.shipments.removeItem(current.items[0].id, 2),
  ]);
  assert.equal(results.filter((result) => result.status === 'fulfilled').length, 1);
  assert.equal(results.filter((result) => result.reason?.code === 'CONFLICT').length, 1);
  current = await f.commerce.shipments.get(shipment.id);
  assert.equal(current.version, 3);
  assert.ok([0, 2].includes(current.items.length));
  assert.equal(f.snapshot().facts.length, 3);
});

test('generated native tools forward optional versions and keep guarded edits preview-only', async (t) => {
  const f = await fixture(t);
  const shipment = await f.commerce.shipments.create(f.input);
  const preview = createNativeToolkit(f.commerce);
  const applied = createNativeToolkit(f.commerce, { allowApply: true });
  const params = { shipmentId: shipment.id, input: f.item, expectedVersion: 1 };
  const before = f.snapshot();
  assert.equal((await preview.executeTool('shipments.addItem', params)).preview, true);
  assert.deepEqual(f.snapshot(), before);
  const added = await applied.executeTool('shipments.addItem', params);
  assert.ok(added.id, JSON.stringify(added));
  const packed = f.snapshot();
  assert.equal((await applied.executeTool('shipments.addItem', params)).error.code, 'CONFLICT');
  const remove = { itemId: added.id, expectedVersion: 2 };
  assert.equal((await preview.executeTool('shipments.removeItem', remove)).preview, true);
  assert.equal(
    (await applied.executeTool('shipments.removeItem', { ...remove, expectedVersion: 1 })).error
      .code,
    'CONFLICT',
  );
  assert.equal(
    (await applied.executeTool('shipments.removeItem', { ...remove, expectedVersion: 4294967298 }))
      .error.code,
    'VALIDATION',
  );
  assert.deepEqual(f.snapshot(), packed);
  await applied.executeTool('shipments.removeItem', remove);
  assert.equal((await f.commerce.shipments.get(shipment.id)).version, 3);
  const empty = f.snapshot();
  assert.equal(
    (await applied.executeTool('shipments.cancel', { id: shipment.id, expectedVersion: 2 })).error
      .code,
    'CONFLICT',
  );
  assert.equal(
    (await preview.executeTool('shipments.cancel', { id: shipment.id, expectedVersion: 3 }))
      .preview,
    true,
  );
  assert.deepEqual(f.snapshot(), empty);
  const cancelled = await applied.executeTool('shipments.cancel', {
    id: shipment.id,
    expectedVersion: 3,
  });
  assert.equal(cancelled.status, 'cancelled');
  assert.equal(cancelled.version, 4);
});
