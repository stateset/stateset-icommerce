import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import Database from 'better-sqlite3';
import {
  PurchaseRuntime,
  SqlitePurchaseStore,
  createKernelPurchaseAdapter,
} from '../../../bindings/node/purchase-runtime.mjs';

const identity = { agentId: 'buyer:1', principalId: 'acme', tenantId: 'tenant', storeId: 'store' };
const request = {
  idempotencyKey: 'purchase:one',
  quoteId: 'quote:one',
  budgetId: 'monthly',
  maxAmount: '50',
  asset: 'USDC',
};

function fixture(overrides = {}) {
  const db = new Database(':memory:');
  const store = new SqlitePurchaseStore(db);
  const effects = new Map();
  const calls = [];
  const evidence = {
    reserve_inventory: { reservation_id: 'reservation:one' },
    pay: { transaction_id: 'transaction:one', amount: '40.000000000000000001', asset: 'USDC' },
    create_order: { order_id: 'order:one' },
    confirm_inventory: { reservation_id: 'reservation:one' },
    release_inventory: { reservation_id: 'reservation:one' },
  };
  const adapters = Object.fromEntries(
    Object.entries(evidence).map(([step, value]) => [
      step,
      {
        async execute({ idempotencyKey }) {
          if (!effects.has(idempotencyKey)) {
            calls.push(step);
            effects.set(idempotencyKey, { status: 'succeeded', evidence: value });
          }
          return effects.get(idempotencyKey);
        },
        async lookup({ idempotencyKey }) {
          return effects.get(idempotencyKey) ?? { status: 'not_found' };
        },
      },
    ]),
  );
  const options = {
    store,
    identity,
    policyVersion: 'v1',
    adapters,
    allowApply: true,
    resolveQuote: async (id) => ({
      id,
      counterpartyId: 'merchant',
      amount: evidence.pay.amount,
      asset: 'USDC',
      expiresAt: '2099-01-01T00:00:00Z',
    }),
    authorize: async () => ({ allowed: true, decisionId: 'policy:one' }),
    ...overrides,
  };
  const runtime = new PurchaseRuntime(options);
  options.store.provisionBudget(runtime.scope, {
    id: 'monthly',
    asset: 'USDC',
    limit: '100',
    expiresAt: '2099-01-01T00:00:00Z',
  });
  return { db, store: options.store, runtime, options, effects, calls };
}

test('preview has no economic effect; replay is exact and changed requests fail closed', async () => {
  const f = fixture({ allowApply: false });
  try {
    assert.equal((await f.runtime.buy(request)).status, 'preview');
    assert.equal(f.store.budget(f.runtime.scope, 'monthly').reserved, '0');
    assert.equal(f.calls.length, 0);
    const runtime = new PurchaseRuntime({ ...f.options, allowApply: true });
    const first = await runtime.buy(request);
    assert.equal(first.status, 'completed');
    assert.equal((await runtime.buy(request)).id, first.id);
    assert.deepEqual(f.calls, ['reserve_inventory', 'pay', 'create_order', 'confirm_inventory']);
    assert.equal(f.store.budget(runtime.scope, 'monthly').spent, '40.000000000000000001');
    await assert.rejects(runtime.buy({ ...request, maxAmount: '49' }), /idempotency conflict/);
  } finally {
    f.db.close();
  }
});

test('lost payment response survives reopening; authoritative lookup prevents duplicate charge', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'stateset-purchase-'));
  let db = new Database(join(dir, 'operations.db'));
  const f = fixture({ store: new SqlitePurchaseStore(db) });
  const original = f.options.adapters.pay.execute;
  f.options.adapters.pay.execute = async (context) => {
    await original(context);
    throw new Error('connection dropped after charge');
  };
  try {
    const uncertain = await f.runtime.buy(request);
    assert.equal(uncertain.status, 'reconciling');
    assert.equal(f.store.budget(f.runtime.scope, 'monthly').reserved, '40.000000000000000001');
    db.close();
    db = new Database(join(dir, 'operations.db'));
    const recovered = new PurchaseRuntime({ ...f.options, store: new SqlitePurchaseStore(db) });
    assert.equal(recovered.pending().operations[0].id, uncertain.id);
    const batch = await recovered.recover();
    const completed = batch.results[0].operation;
    assert.equal(completed.status, 'completed');
    assert.deepEqual(recovered.pending(), { operations: [], nextCursor: null });
    assert.equal(f.calls.filter((step) => step === 'pay').length, 1);
    assert.equal(recovered.store.budget(recovered.scope, 'monthly').reserved, '0');
    assert.equal(recovered.store.budget(recovered.scope, 'monthly').spent, '40.000000000000000001');
  } finally {
    db.close();
    f.db.close();
    rmSync(dir, { recursive: true });
  }
});

test('pending discovery is paginated, read-only and scoped to the configured agent', async () => {
  const f = fixture();
  f.options.adapters.pay.execute = async () => ({ status: 'unknown' });
  try {
    const first = await f.runtime.buy(request);
    const second = await f.runtime.buy({ ...request, idempotencyKey: 'second' });
    const page = f.runtime.pending({ limit: 1 });
    assert.equal(page.operations.length, 1);
    assert.equal(page.nextCursor, page.operations[0].id);
    const last = f.runtime.pending({ limit: 1, after: page.nextCursor });
    assert.equal(last.nextCursor, null);
    assert.deepEqual([page.operations[0].id, last.operations[0].id], [first.id, second.id].sort());
    const preview = new PurchaseRuntime({ ...f.options, allowApply: false });
    assert.equal(preview.pending().operations.length, 2);
    await assert.rejects(preview.recover(), /apply is disabled/);
    for (const field of ['agentId', 'principalId', 'tenantId', 'storeId']) {
      const other = new PurchaseRuntime({
        ...f.options,
        identity: { ...identity, [field]: 'other' },
      });
      assert.deepEqual(other.pending(), { operations: [], nextCursor: null });
    }
    for (const limit of [0, -1, 1001, 1.5, '1', NaN]) {
      assert.throws(() => f.runtime.pending({ limit }), /recovery limit/);
    }
    assert.throws(() => f.runtime.pending({ after: 123 }), /cursor/);
  } finally {
    f.db.close();
  }
});

test('recovery pages progress past uncertain outcomes without resubmitting them', async () => {
  const f = fixture();
  let submissions = 0;
  f.options.adapters.pay.execute = async () => {
    submissions++;
    return { status: 'unknown' };
  };
  f.options.adapters.pay.lookup = async () => ({ status: 'unknown' });
  try {
    await f.runtime.buy(request);
    await f.runtime.buy({ ...request, idempotencyKey: 'second' });
    const first = await f.runtime.recover({ limit: 1 });
    const last = await f.runtime.recover({ limit: 1, after: first.nextCursor });
    assert.notEqual(first.results[0].id, last.results[0].id);
    assert.equal(last.nextCursor, null);
    assert.equal(submissions, 2);
    assert.equal(f.runtime.pending().operations.length, 2);
    assert.equal(f.store.budget(f.runtime.scope, 'monthly').reserved, '80.000000000000000002');
  } finally {
    f.db.close();
  }
});

test('completing one recovery page cannot skip the next pending purchase', async () => {
  const f = fixture();
  const execute = f.options.adapters.pay.execute;
  f.options.adapters.pay.execute = async (context) => {
    await execute(context);
    throw new Error('lost response');
  };
  try {
    await f.runtime.buy(request);
    await f.runtime.buy({ ...request, idempotencyKey: 'second' });
    const first = await f.runtime.recover({ limit: 1 });
    assert.equal(first.results[0].operation.status, 'completed');
    assert.ok(first.nextCursor);
    const last = await f.runtime.recover({ limit: 1, after: first.nextCursor });
    assert.equal(last.results[0].operation.status, 'completed');
    assert.notEqual(first.results[0].id, last.results[0].id);
    assert.deepEqual(f.runtime.pending(), { operations: [], nextCursor: null });
    assert.equal(f.calls.filter((step) => step === 'pay').length, 2);
  } finally {
    f.db.close();
  }
});

test('automatic recovery skips operator attention and respects active worker leases', async () => {
  const f = fixture();
  f.options.adapters.create_order.execute = async () => ({
    status: 'failed',
    reason: 'manual review',
  });
  try {
    const attention = await f.runtime.buy(request);
    assert.equal(attention.status, 'needs_attention');
    let authorizations = 0;
    const runtime = new PurchaseRuntime({
      ...f.options,
      authorize: async () => {
        authorizations++;
        return { allowed: true };
      },
    });
    assert.equal((await runtime.recover()).results[0].operation.skipped, 'needs_attention');
    assert.equal(authorizations, 0);
    assert.equal(f.store.claim(attention.id, 'another-worker', Date.now(), 60_000), true);
    assert.equal((await runtime.recover()).results[0].operation.busy, true);
    f.store.release(attention.id, 'another-worker');
    assert.equal(f.runtime.get(attention.id).status, 'needs_attention');
  } finally {
    f.db.close();
  }
});

test('one recovery failure does not prevent processing the rest of a page', async () => {
  const f = fixture();
  const execute = f.options.adapters.pay.execute;
  f.options.adapters.pay.execute = async (context) => {
    await execute(context);
    throw new Error('lost');
  };
  try {
    await f.runtime.buy(request);
    await f.runtime.buy({ ...request, idempotencyKey: 'second' });
    const [first, second] = f.runtime.pending().operations;
    const claim = f.store.claim.bind(f.store);
    f.store.claim = (id, ...args) => {
      if (id === first.id) throw new Error('simulated lease-store failure');
      return claim(id, ...args);
    };
    const batch = await f.runtime.recover();
    assert.match(batch.results[0].error, /lease-store failure/);
    assert.equal(batch.results[1].id, second.id);
    assert.equal(batch.results[1].operation.status, 'completed');
    f.store.claim = claim;
    assert.equal((await f.runtime.recover()).results[0].operation.status, 'completed');
    assert.equal(f.calls.filter((step) => step === 'pay').length, 2);
  } finally {
    f.db.close();
  }
});

test('recovery rechecks status under the lease when discovery becomes stale', async () => {
  for (const status of ['needs_attention', 'completed']) {
    const f = fixture();
    f.options.adapters.pay.execute = async () => ({ status: 'unknown' });
    try {
      await f.runtime.buy(request);
      const claim = f.store.claim.bind(f.store);
      f.store.claim = (id, owner, now, leaseMs) => {
        assert.equal(claim(id, 'peer-worker', now, leaseMs), true);
        const operation = f.store.get(id);
        operation.status = status;
        f.store.save(operation, 'peer-worker');
        f.store.release(id, 'peer-worker');
        return claim(id, owner, now, leaseMs);
      };
      let authorized = false;
      const runtime = new PurchaseRuntime({
        ...f.options,
        authorize: async () => {
          authorized = true;
          return { allowed: true };
        },
      });
      assert.equal((await runtime.recover()).results[0].operation.status, status);
      assert.equal(authorized, false);
    } finally {
      f.db.close();
    }
  }
});

test('shared principal budget cannot be exceeded by concurrent agents', async () => {
  const f = fixture();
  try {
    const runtimes = ['buyer:1', 'buyer:2', 'buyer:3'].map(
      (agentId) => new PurchaseRuntime({ ...f.options, identity: { ...identity, agentId } }),
    );
    const results = await Promise.allSettled(runtimes.map((runtime) => runtime.buy(request)));
    assert.equal(results.filter((result) => result.status === 'fulfilled').length, 2);
    assert.match(
      results.find((result) => result.status === 'rejected').reason.message,
      /budget exceeded/,
    );
    assert.equal(f.calls.filter((step) => step === 'pay').length, 2);
    assert.equal(f.store.budget(f.runtime.scope, 'monthly').spent, '80.000000000000000002');
  } finally {
    f.db.close();
  }
});

test('unknown payment outcome holds budget and does not resubmit or compensate', async () => {
  const f = fixture();
  let attempts = 0;
  f.options.adapters.pay.execute = async () => {
    attempts++;
    throw new Error('timeout');
  };
  f.options.adapters.pay.lookup = async () => ({ status: 'unknown' });
  try {
    const first = await f.runtime.buy(request);
    for (let i = 0; i < 3; i++)
      assert.equal((await f.runtime.resume(first.id)).status, 'reconciling');
    assert.equal(attempts, 1);
    assert.equal(f.calls.includes('release_inventory'), false);
    assert.equal(f.store.budget(f.runtime.scope, 'monthly').reserved, '40.000000000000000001');
  } finally {
    f.db.close();
  }
});

test('definitive payment rejection releases inventory before releasing budget', async () => {
  const f = fixture();
  f.options.adapters.pay.execute = async () => ({ status: 'failed', reason: 'declined' });
  try {
    assert.equal((await f.runtime.buy(request)).status, 'cancelled');
    assert.deepEqual(f.calls, ['reserve_inventory', 'release_inventory']);
    assert.equal(f.store.budget(f.runtime.scope, 'monthly').available, '100');
  } finally {
    f.db.close();
  }
});

test('wrong payment evidence cannot complete the purchase or release the hold', async () => {
  const f = fixture();
  f.options.adapters.pay.execute = async () => ({
    status: 'succeeded',
    evidence: { transaction_id: 'wrong', amount: '0.1', asset: 'USDC' },
  });
  try {
    const result = await f.runtime.buy(request);
    assert.equal(result.status, 'reconciling');
    assert.match(result.error, /evidence does not match/);
    assert.equal(f.calls.includes('create_order'), false);
    assert.equal(f.store.budget(f.runtime.scope, 'monthly').reserved, '40.000000000000000001');
  } finally {
    f.db.close();
  }
});

test('failed compensation cannot restart the payment path', async () => {
  const f = fixture();
  let payments = 0;
  f.options.adapters.pay.execute = async () => {
    payments++;
    return { status: 'failed', reason: 'declined' };
  };
  const release = f.options.adapters.release_inventory.execute;
  f.options.adapters.release_inventory.execute = async () => ({
    status: 'failed',
    reason: 'locked',
  });
  try {
    const result = await f.runtime.buy(request);
    assert.equal(result.status, 'needs_attention');
    f.options.adapters.release_inventory.execute = release;
    assert.equal((await f.runtime.resume(result.id)).status, 'cancelled');
    assert.equal(payments, 1);
    assert.equal(f.store.budget(f.runtime.scope, 'monthly').available, '100');
  } finally {
    f.db.close();
  }
});

test('mismatched inventory confirmation cannot complete a paid purchase', async () => {
  const f = fixture();
  f.options.adapters.confirm_inventory.execute = async () => ({
    status: 'succeeded',
    evidence: { reservation_id: 'some-other-purchase' },
  });
  try {
    const result = await f.runtime.buy(request);
    assert.equal(result.status, 'reconciling');
    assert.match(result.error, /inventory evidence/);
    assert.equal(result.budgetState, 'spent');
    assert.equal(result.receipt, undefined);
  } finally {
    f.db.close();
  }
});

test('operator authority, scope, leases and decimal strings are enforced', async () => {
  const f = fixture();
  try {
    await assert.rejects(f.runtime.buy({ ...request, maxAmount: 50 }), /maxAmount/);
    await assert.rejects(f.runtime.buy({ ...request, authority: '*' }), /unknown purchase/);
    const denied = new PurchaseRuntime({
      ...f.options,
      authorize: async () => ({ allowed: false }),
    });
    await assert.rejects(denied.buy(request), /not authorized/);
    const result = await f.runtime.buy(request);
    const stranger = new PurchaseRuntime({
      ...f.options,
      identity: { ...identity, agentId: 'stranger' },
    });
    assert.throws(() => stranger.get(result.id), /not found/);
    assert.equal(f.store.claim(result.id, 'worker:one', 100, 100), true);
    assert.equal(f.store.claim(result.id, 'worker:two', 199, 100), false);
    assert.equal(f.store.claim(result.id, 'worker:two', 201, 100), true);
    assert.throws(() => f.store.save(result, 'worker:one'), /lease lost/);
  } finally {
    f.db.close();
  }
});

test('every purchase step recovers a lost response without repeating its effect', async () => {
  for (const step of ['reserve_inventory', 'pay', 'create_order', 'confirm_inventory']) {
    const f = fixture();
    const execute = f.options.adapters[step].execute;
    f.options.adapters[step].execute = async (context) => {
      await execute(context);
      throw new Error(`lost response: ${step}`);
    };
    try {
      const interrupted = await f.runtime.buy(request);
      assert.equal(interrupted.status, 'reconciling');
      const restarted = new PurchaseRuntime(f.options);
      assert.equal((await restarted.resume(interrupted.id)).status, 'completed');
      assert.equal(f.calls.filter((name) => name === step).length, 1);
    } finally {
      f.db.close();
    }
  }
});

test('cancellation preserves an uncertain payment hold until authoritative absence', async () => {
  const f = fixture();
  f.options.adapters.pay.execute = async () => {
    throw new Error('timeout');
  };
  f.options.adapters.pay.lookup = async () => ({ status: 'unknown' });
  try {
    const first = await f.runtime.buy(request);
    assert.equal((await f.runtime.cancel(first.id)).status, 'reconciling');
    assert.equal(f.calls.includes('release_inventory'), false);
    f.options.adapters.pay.lookup = async () => ({ status: 'not_found' });
    assert.equal((await f.runtime.resume(first.id)).status, 'cancelled');
    assert.equal(f.calls.filter((step) => step === 'release_inventory').length, 1);
    assert.equal(f.store.budget(f.runtime.scope, 'monthly').available, '100');
  } finally {
    f.db.close();
  }
});

test('revocation reconciles already-paid effects but stops new mutations', async () => {
  const f = fixture();
  const execute = f.options.adapters.pay.execute;
  f.options.adapters.pay.execute = async (context) => {
    await execute(context);
    throw new Error('lost');
  };
  try {
    const first = await f.runtime.buy(request);
    const revoked = new PurchaseRuntime({
      ...f.options,
      authorize: async () => ({ allowed: false }),
    });
    assert.equal((await revoked.resume(first.id)).status, 'needs_attention');
    assert.equal(f.calls.includes('create_order'), false);
    assert.equal(f.store.budget(f.runtime.scope, 'monthly').spent, '40.000000000000000001');
    assert.equal((await revoked.cancel(first.id)).status, 'needs_attention');
    assert.equal(f.store.budget(f.runtime.scope, 'monthly').available, '59.999999999999999999');
  } finally {
    f.db.close();
  }
});

test('kernel adapter reserves and confirms real inventory with durable receipt lookup', async () => {
  const { Commerce } = await import('../../../bindings/node/index.js');
  const dir = mkdtempSync(join(tmpdir(), 'stateset-kernel-purchase-'));
  const path = join(dir, 'commerce.db');
  const commerce = new Commerce(path);
  await commerce.inventory.createItem({ sku: 'SKU-100', name: 'Inventory', initialQuantity: 50 });
  const db = new Database(path);
  const capabilities = ['inventory.reserve', 'inventory.reservation.confirm'];
  const policy = {
    version: 'v1',
    commands: Object.fromEntries(
      capabilities.map((name) => [name, { required_capabilities: [name] }]),
    ),
    trusted_authority_keys: {},
  };
  const principal = {
    id: 'merchant',
    kind: 'agent',
    tenant_id: 'tenant',
    delegated_by: 'company',
    capabilities,
  };
  const common = {
    commerce,
    policy,
    principal,
    storeId: 'store',
    readReceipt: async (key) => {
      const row = db
        .prepare('SELECT receipt FROM kernel_receipts WHERE idempotency_key=?')
        .get(key);
      return row ? JSON.parse(row.receipt) : null;
    },
  };
  const reserve = createKernelPurchaseAdapter({
    ...common,
    commandType: 'inventory.reserve',
    buildPayload: () => ({
      sku: 'SKU-100',
      quantity: '5',
      reference_type: 'purchase',
      reference_id: 'test',
    }),
    evidence: (receipt) => ({ reservation_id: receipt.aggregate_id }),
  });
  const operation = { createdAt: new Date().toISOString() };
  try {
    const context = { operation, idempotencyKey: 'kernel:reserve' };
    assert.equal((await reserve.lookup(context)).status, 'not_found');
    const reserved = await reserve.execute(context);
    assert.equal(reserved.status, 'succeeded', JSON.stringify(reserved));
    assert.equal(
      (await reserve.lookup(context)).evidence.reservation_id,
      reserved.evidence.reservation_id,
    );
    assert.equal(
      (await reserve.execute(context)).evidence.reservation_id,
      reserved.evidence.reservation_id,
    );
    assert.equal((await commerce.inventory.getStock('SKU-100')).totalAllocated, '5');
    const confirm = createKernelPurchaseAdapter({
      ...common,
      commandType: 'inventory.reservation.confirm',
      buildPayload: () => ({ reservation_id: reserved.evidence.reservation_id }),
      evidence: () => ({ reservation_id: reserved.evidence.reservation_id }),
    });
    const confirmed = await confirm.execute({ operation, idempotencyKey: 'kernel:confirm' });
    assert.equal(confirmed.status, 'succeeded');
    assert.equal(confirmed.evidence.kernel_receipt.result.status, 'confirmed');
    const stock = await commerce.inventory.getStock('SKU-100');
    // Confirmation commits the allocation; only shipment consumes on-hand stock.
    assert.equal(stock.totalAllocated, '5');
    assert.equal(stock.totalOnHand, '50');
  } finally {
    db.close();
    rmSync(dir, { recursive: true });
  }
});
