import test from 'node:test';
import assert from 'node:assert/strict';
import { NativeMerchantFulfillment, mergeNativeFulfillment } from '../src/native-fulfillment.mjs';

function fixture(options = {}) {
  const namespaces = new Map();
  const store = {
    atomic: (fn) => fn(),
    collection(name) {
      if (!namespaces.has(name)) namespaces.set(name, new Map());
      const rows = namespaces.get(name);
      return {
        get: (id) => structuredClone(rows.get(id)),
        set: (id, value) => rows.set(id, structuredClone(value)),
        values: () => [...rows.values()].map((value) => structuredClone(value)),
        get size() {
          return rows.size;
        },
      };
    },
  };
  const terms = {
    orderId: 'order-a',
    quoteId: 'quote-a',
    buyer: 'buyer-a',
    amount: '25.00',
    currency: 'USD',
  };
  let calls = 0;
  const receipt = (command) => ({
    command_id: command.command_id,
    command_type: command.command_type,
    idempotency_key: command.idempotency_key,
    status: 'succeeded',
    version_before: 3,
    version_after: 4,
    result: {
      id: command.payload.order_id,
      version: 4,
      status: 'shipped',
      payment_status: 'paid',
      currency: 'USD',
      total_amount: '25.00',
      tracking_number: 'TRACK',
      items: [{ id: 'line-a', quantity: 2, shipped_quantity: 2 }],
    },
  });
  const bridge = new NativeMerchantFulfillment({
    store,
    principal: { id: 'operator', tenant_id: 'tenant' },
    policy: { version: 'v1' },
    storeId: 'store',
    allowApply: true,
    readReceipt: async () => null,
    resolveDispatch: async (t) => ({
      reference: 'dispatch-one',
      orderId: t.orderId,
      orderVersion: 3,
      trackingNumber: 'TRACK',
      items: [{ orderItemId: 'line-a', quantity: 2 }],
    }),
    commerce: {
      kernelFeatures: async () => ['orders.ship_reserved.v1'],
      orders: {
        get: async (id) => ({
          id,
          status: 'processing',
          paymentStatus: 'paid',
          currency: 'USD',
          totalAmountExact: '25.00',
          version: 3,
          items: [{ id: 'line-a', quantity: 2, shippedQuantity: 0 }],
        }),
      },
      async executeKernelCommand(command) {
        calls++;
        return receipt(command);
      },
    },
    ...options,
  });
  return { bridge, terms, store, receipt, calls: () => calls };
}

test('dispatch bridge preview neither looks up approvals nor claims them', async () => {
  const f = fixture({
    allowApply: false,
    resolveDispatch: () => {
      throw new Error('must not call');
    },
  });
  assert.deepEqual(await f.bridge.reconcile(f.terms), { status: 'preview' });
  assert.equal(f.calls(), 0);
  assert.equal(f.store.collection('native_fulfillment_operations').size, 0);
  assert.equal(f.store.collection('native_fulfillment_claims').size, 0);
});

test('one operator dispatch reference cannot record two orders in the same journal', async () => {
  const f = fixture();
  assert.equal((await f.bridge.reconcile(f.terms)).status, 'recorded');
  assert.equal(
    (await f.bridge.reconcile({ ...f.terms, orderId: 'order-b', quoteId: 'quote-b' })).status,
    'needs_attention',
  );
  assert.equal(f.calls(), 1);
  assert.equal(f.store.collection('native_fulfillment_operations').size, 1);
});

test('old binaries cannot silently downgrade a stock-consuming dispatch', async () => {
  const f = fixture();
  f.bridge.commerce.kernelFeatures = async () => [];
  f.bridge.resolveDispatch = () => assert.fail('unsupported binary must refuse before approval');
  assert.equal((await f.bridge.reconcile(f.terms)).status, 'needs_attention');
  assert.equal(f.calls(), 0);
  assert.equal(f.store.collection('native_fulfillment_operations').size, 0);
});

test('legacy pinned shipment receipts retain their unverified inventory meaning', async () => {
  const f = fixture();
  f.bridge.readReceipt = () => {
    throw new Error('offline');
  };
  assert.equal((await f.bridge.reconcile(f.terms)).status, 'reconciling');
  const operation = f.bridge.operations.values()[0];
  operation.command.command_type = 'orders.ship';
  delete operation.format;
  delete operation.beforeItems;
  f.bridge.operations.set(
    operation.command.idempotency_key.slice('native-fulfillment:'.length),
    operation,
  );
  f.bridge.readReceipt = () => f.receipt(operation.command);
  f.bridge.commerce.kernelFeatures = () => assert.fail('historical recovery needs no new feature');
  const result = await f.bridge.reconcile(f.terms);
  assert.equal(result.status, 'recorded');
  assert.equal(result.inventory, 'depletion_unverified');
  assert.equal(f.calls(), 0);
});

for (const corrupt of ['identity', 'order', 'quantity', 'payment']) {
  test(`a mismatched ${corrupt} receipt cannot prove dispatch or authorize another command`, async () => {
    const f = fixture();
    // First pin the command while the authoritative ledger is unavailable.
    f.bridge.readReceipt = () => {
      throw new Error('offline');
    };
    assert.equal((await f.bridge.reconcile(f.terms)).status, 'reconciling');
    // Recover the saved command through the normal executor, return an
    // inconsistent ledger receipt, and ensure later retries only read it.
    let saved;
    f.bridge.readReceipt = () => saved ?? null;
    f.bridge.commerce.executeKernelCommand = async (command) => {
      saved = f.receipt(command);
      if (corrupt === 'identity') saved.command_id = 'other-command';
      if (corrupt === 'order') saved.result.id = 'other-order';
      if (corrupt === 'quantity') saved.result.items[0].shipped_quantity = 1;
      if (corrupt === 'payment') saved.result.payment_status = 'refunded';
      return saved;
    };
    assert.equal((await f.bridge.reconcile(f.terms)).status, 'reconciling');
    f.bridge.commerce.executeKernelCommand = () => assert.fail('must recover existing receipt');
    assert.equal((await f.bridge.reconcile(f.terms)).status, 'reconciling');
  });
}

function partialFixture() {
  const f = fixture();
  let order = {
    id: f.terms.orderId,
    status: 'processing',
    paymentStatus: 'paid',
    currency: 'USD',
    totalAmountExact: '25.00',
    version: 3,
    items: [
      { id: 'line-a', quantity: 2, shippedQuantity: 0 },
      { id: 'line-b', quantity: 1, shippedQuantity: 0 },
    ],
  };
  let dispatch;
  const receipts = new Map();
  const commands = [];
  f.bridge.commerce.kernelFeatures = async () => [
    'orders.ship_reserved.v1',
    'orders.ship_reserved.partial.v1',
  ];
  f.bridge.commerce.orders.get = async () => structuredClone(order);
  f.bridge.resolveDispatch = async () => structuredClone(dispatch);
  f.bridge.readReceipt = async (key) => structuredClone(receipts.get(key) ?? null);
  f.bridge.commerce.executeKernelCommand = async (command) => {
    if (receipts.has(command.idempotency_key))
      return structuredClone(receipts.get(command.idempotency_key));
    commands.push(structuredClone(command));
    assert.equal(command.expected_version, order.version);
    const before = order.version;
    order = structuredClone(order);
    for (const line of command.payload.lines)
      order.items.find((item) => item.id === line.order_item_id).shippedQuantity += line.quantity;
    order.version++;
    order.status = order.items.every((item) => item.quantity === item.shippedQuantity)
      ? 'shipped'
      : 'partially_shipped';
    const receipt = {
      ...f.receipt(command),
      version_before: before,
      version_after: order.version,
      result: {
        id: order.id,
        version: order.version,
        status: order.status,
        payment_status: 'paid',
        currency: 'USD',
        total_amount: '25.00',
        tracking_number: command.payload.tracking_number,
        items: order.items.map((item) => ({
          id: item.id,
          quantity: item.quantity,
          shipped_quantity: item.shippedQuantity,
        })),
      },
    };
    receipts.set(command.idempotency_key, receipt);
    return structuredClone(receipt);
  };
  return {
    ...f,
    commands,
    receipts,
    approve(reference, items = [{ orderItemId: 'line-a', quantity: 1 }]) {
      dispatch = {
        reference,
        orderId: order.id,
        orderVersion: order.version,
        trackingNumber: reference,
        items,
      };
    },
  };
}

test('partial dispatches preserve untouched lines, use distinct commands, and ignore historical approvals', async () => {
  const f = partialFixture();
  f.approve('first');
  const first = await f.bridge.reconcile(f.terms);
  assert.equal(first.order_status, 'partially_shipped');
  assert.deepEqual(
    first.items.map((item) => item.shipped_quantity),
    [1, 0],
  );
  assert.deepEqual(await f.bridge.reconcile(f.terms), first);
  f.approve('second');
  const second = await f.bridge.reconcile(f.terms);
  assert.equal(second.order_version, 5);
  assert.deepEqual(
    second.items.map((item) => item.shipped_quantity),
    [2, 0],
  );
  f.approve('first');
  assert.deepEqual(await f.bridge.reconcile(f.terms), second);
  f.approve('third', [{ orderItemId: 'line-b', quantity: 1 }]);
  assert.equal((await f.bridge.reconcile(f.terms)).order_status, 'shipped');
  assert.equal(new Set(f.commands.map((c) => c.idempotency_key)).size, 3);
  assert.equal(f.bridge.operations.size, 3);
  f.bridge.resolveDispatch = () => assert.fail('completed order must not resolve another dispatch');
  assert.equal((await f.bridge.reconcile(f.terms)).order_status, 'shipped');
});

test('an unresolved partial dispatch recovers its pinned receipt before a new approval is read', async () => {
  const f = partialFixture();
  f.approve('first');
  const lookup = f.bridge.readReceipt;
  f.bridge.readReceipt = () => {
    throw new Error('ledger offline');
  };
  assert.equal((await f.bridge.reconcile(f.terms)).status, 'reconciling');
  f.approve('replacement');
  f.bridge.resolveDispatch = () => assert.fail('must recover pinned operation');
  f.bridge.readReceipt = lookup;
  const result = await f.bridge.reconcile(f.terms);
  assert.equal(result.order_status, 'partially_shipped');
  assert.equal(f.commands[0].payload.tracking_number, 'first');
  assert.equal(f.commands.length, 1);
});

test('a rejected subsequent command keeps prior progress and cannot be replaced by the resolver', async () => {
  const f = partialFixture();
  f.approve('first');
  const first = await f.bridge.reconcile(f.terms);
  f.approve('second');
  f.bridge.commerce.executeKernelCommand = async (command) => {
    const receipt = { ...f.receipt(command), status: 'rejected' };
    f.receipts.set(command.idempotency_key, receipt);
    return receipt;
  };
  const result = await f.bridge.reconcile(f.terms);
  assert.equal(result.order_version, first.order_version);
  assert.equal(result.next_dispatch_status, 'needs_attention');
  f.bridge.resolveDispatch = () => assert.fail('rejected command needs operator reconciliation');
  assert.deepEqual(await f.bridge.reconcile(f.terms), result);
  assert.equal(f.bridge.operations.size, 2);
});

test('partial dispatches refuse a full-shipment-only native binary before pinning a command', async () => {
  const f = partialFixture();
  f.approve('first');
  f.bridge.commerce.kernelFeatures = async () => ['orders.ship_reserved.v1'];
  assert.equal((await f.bridge.reconcile(f.terms)).status, 'needs_attention');
  assert.equal(f.bridge.operations.size, 0);
});

test('receipt validation detects changes to an untouched line and to the resulting version', async () => {
  for (const fault of ['untouched', 'version', 'status', 'duplicate']) {
    const f = partialFixture();
    f.approve('first');
    const execute = f.bridge.commerce.executeKernelCommand;
    f.bridge.commerce.executeKernelCommand = async (command) => {
      const receipt = await execute(command);
      if (fault === 'untouched') receipt.result.items[1].shipped_quantity = 1;
      if (fault === 'version') receipt.version_after++;
      if (fault === 'status') receipt.result.status = 'shipped';
      if (fault === 'duplicate') receipt.result.items[1] = receipt.result.items[0];
      f.receipts.set(command.idempotency_key, receipt);
      return receipt;
    };
    assert.equal((await f.bridge.reconcile(f.terms)).status, 'reconciling', fault);
    f.bridge.resolveDispatch = () =>
      assert.fail('mismatched receipt cannot authorize another dispatch');
    assert.equal((await f.bridge.reconcile(f.terms)).status, 'reconciling', fault);
    assert.equal(f.commands.length, 1);
  }
});

test('stale fulfillment responses cannot regress partial or completed shipment projections', () => {
  const first = { status: 'recorded', order_status: 'partially_shipped', order_version: 4 };
  const second = { ...first, order_version: 5 };
  const final = { ...second, order_status: 'shipped', order_version: 6 };
  const attention = { ...first, next_dispatch_status: 'needs_attention' };
  assert.deepEqual(mergeNativeFulfillment(second, first), second);
  assert.deepEqual(mergeNativeFulfillment(first, second), second);
  assert.deepEqual(mergeNativeFulfillment(final, attention), final);
  assert.deepEqual(mergeNativeFulfillment(first, { status: 'reconciling' }), first);
  assert.deepEqual(mergeNativeFulfillment(attention, first), attention);
  assert.deepEqual(mergeNativeFulfillment(attention, second), second);
  const legacy = { status: 'recorded', inventory: 'depletion_unverified' };
  assert.deepEqual(mergeNativeFulfillment(legacy, first), legacy);
});

test('competing approvals at one order version converge on one immutable next command', async () => {
  const f = partialFixture();
  f.approve('first');
  await f.bridge.reconcile(f.terms);
  const peer = new NativeMerchantFulfillment({
    store: f.store,
    commerce: f.bridge.commerce,
    principal: f.bridge.principal,
    policy: f.bridge.policy,
    storeId: f.bridge.storeId,
    readReceipt: f.bridge.readReceipt,
    allowApply: true,
    resolveDispatch: async (terms) => ({
      reference: 'competing',
      orderId: terms.orderId,
      orderVersion: 4,
      trackingNumber: 'competing',
      items: [{ orderItemId: 'line-a', quantity: 1 }],
    }),
  });
  f.approve('second');
  const readOrder = f.bridge.commerce.orders.get;
  let arrivals = 0;
  let release;
  const gate = new Promise((resolve) => {
    release = resolve;
  });
  f.bridge.commerce.orders.get = async () => {
    const snapshot = await readOrder();
    if (++arrivals === 2) release();
    await gate;
    return snapshot;
  };
  const results = await Promise.all([f.bridge.reconcile(f.terms), peer.reconcile(f.terms)]);
  assert.ok(
    results.every((r) => r.order_version === 5),
    JSON.stringify(results),
  );
  assert.equal(f.commands.length, 2);
  assert.equal(f.bridge.operations.size, 2);
  assert.equal(f.bridge.claims.size, 2);
});

test('receipt outage on a subsequent pinned operation retains its approval and prior progress', async () => {
  const f = partialFixture();
  f.approve('first');
  const first = await f.bridge.reconcile(f.terms);
  f.approve('second');
  const lookup = f.bridge.readReceipt;
  f.bridge.readReceipt = async (key) => {
    const result = await lookup(key);
    if (result === null) throw new Error('unknown outcome');
    return result;
  };
  const waiting = await f.bridge.reconcile(f.terms);
  assert.equal(waiting.order_version, first.order_version);
  assert.equal(waiting.next_dispatch_status, 'reconciling');
  assert.equal(f.bridge.operations.size, 2);
  f.bridge.resolveDispatch = () =>
    assert.fail('pending second command must not read another approval');
  f.bridge.readReceipt = lookup;
  const recovered = await f.bridge.reconcile(f.terms);
  assert.equal(recovered.order_version, first.order_version + 1);
  assert.equal(f.commands[1].payload.tracking_number, 'second');
});
