import test from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import Database from 'better-sqlite3';
import { Commerce } from '../../../bindings/node/index.js';
import { SqliteProtocolStore } from '../../../icp-handler/src/sqlite-store.mjs';
import { NativeMerchantCheckout } from '../../../icp-handler/src/native-checkout.mjs';
import { amount } from '../../../icp-handler/src/quote-money.mjs';

async function fixture(initialQuantity = 10) {
  const dir = mkdtempSync(join(tmpdir(), 'native-merchant-'));
  const path = join(dir, 'commerce.db');
  const commerce = new Commerce(path);
  await commerce.inventory.createItem({
    sku: 'NATIVE-100',
    name: 'Native inventory',
    initialQuantity,
  });
  const cart = await commerce.carts.create({
    customerEmail: 'buyer@example.com',
    customerName: 'Buyer',
    currency: 'USD',
    shippingAddress: {
      firstName: 'Ada',
      lastName: 'L',
      line1: '1 Main St',
      city: 'Seattle',
      postalCode: '98101',
      country: 'US',
    },
  });
  const db = new Database(path);
  db.pragma('busy_timeout = 5000');
  const now = new Date().toISOString();
  // Exact-string fixture seeding: no f64 cart-binding conversions.
  db.prepare(
    `INSERT INTO cart_items(id,cart_id,sku,name,quantity,unit_price,total,created_at,updated_at)
    VALUES(?,?,?,?,?,?,?,?,?)`,
  ).run(randomUUID(), cart.id, 'NATIVE-100', 'Native inventory', 2, '12.50', '25.00', now, now);
  db.prepare("UPDATE carts SET subtotal='25.00', grand_total='25.00' WHERE id=?").run(cart.id);
  const store = new SqliteProtocolStore(db);
  const principal = {
    id: 'agent:merchant',
    kind: 'agent',
    tenant_id: 'tenant',
    delegated_by: 'company',
    capabilities: ['checkout.commit'],
  };
  const policy = {
    version: 'native-v1',
    commands: { 'checkout.commit': { required_capabilities: ['checkout.commit'] } },
    trusted_authority_keys: {},
  };
  const quote = {
    quoteId: 'quote:native',
    cartId: cart.id,
    amount: '25',
    currency: 'USD',
    expiresAt: '2099-01-01T00:00:00Z',
  };
  const options = {
    store,
    commerce,
    principal,
    policy,
    storeId: 'store',
    resolveQuote: async () => quote,
    readReceipt: async (key) => {
      const row = db
        .prepare('SELECT receipt FROM kernel_receipts WHERE idempotency_key=?')
        .get(key);
      return row ? JSON.parse(row.receipt) : null;
    },
    allowApply: true,
  };
  return {
    dir,
    path,
    db,
    commerce,
    cart,
    quote,
    options,
    runtime: new NativeMerchantCheckout(options),
    close() {
      db.close();
      rmSync(dir, { recursive: true });
    },
  };
}
const request = { quoteId: 'quote:native', idempotencyKey: 'accept:native' };

test('strict checkout fails closed on missing or malformed native feature support', async () => {
  const f = await fixture();
  try {
    for (const features of [undefined, [], 'checkout.stock_policy.v1', ['other']]) {
      let calls = 0;
      const runtime = new NativeMerchantCheckout({
        ...f.options,
        stockPolicy: 'reject_if_insufficient',
        commerce: {
          kernelFeatures: () => features,
          executeKernelCommand() {
            calls++;
          },
        },
      });
      await assert.rejects(runtime.accept(request), /does not advertise/);
      assert.equal(calls, 0);
      assert.equal(f.options.store.collection('native_checkout_operations').size, 0);
      assert.equal(f.options.store.collection('native_checkout_carts').size, 0);
    }
    assert.throws(
      () => new NativeMerchantCheckout({ ...f.options, stockPolicy: 'ignore' }),
      /invalid checkout stock policy/,
    );
    await assert.rejects(
      f.runtime.accept({ ...request, stockPolicy: 'allow_backorder' }),
      /unknown acceptance argument/,
    );
  } finally {
    f.close();
  }
});

for (const stock of [1, 2]) {
  test(`strict native bridge enforces stock ${stock} in preview and apply`, async () => {
    const f = await fixture(stock);
    try {
      assert.ok(f.commerce.kernelFeatures().includes('checkout.stock_policy.v1'));
      const options = { ...f.options, stockPolicy: 'reject_if_insufficient' };
      const preview = await new NativeMerchantCheckout({ ...options, allowApply: false }).accept(
        request,
      );
      assert.equal(preview.receipt.status, stock === 1 ? 'rejected' : 'previewed');
      assert.equal((await f.commerce.inventory.getStock('NATIVE-100')).totalAllocated, '0');
      const runtime = new NativeMerchantCheckout(options);
      const result = await runtime.accept(request);
      assert.equal(result.status, stock === 1 ? 'rejected' : 'accepted', JSON.stringify(result));
      if (stock === 1)
        assert.equal(result.receipt.error_code, 'commerce.inventory.insufficient_available');
      assert.deepEqual(await runtime.accept(request), result);
      assert.equal(
        f.db.prepare('SELECT COUNT(*) AS count FROM orders').get().count,
        stock === 1 ? 0 : 1,
      );
      assert.equal(f.db.prepare('SELECT COUNT(*) AS count FROM backorders').get().count, 0);
      assert.equal(
        (await f.commerce.inventory.getStock('NATIVE-100')).totalAllocated,
        stock === 1 ? '0' : '2',
      );
    } finally {
      f.close();
    }
  });
}

test('binary downgrade cannot dispatch a persisted strict intent but can recover its receipt', async () => {
  const f = await fixture();
  try {
    const interrupted = new NativeMerchantCheckout({
      ...f.options,
      stockPolicy: 'reject_if_insufficient',
      commerce: {
        kernelFeatures: () => f.commerce.kernelFeatures(),
        executeKernelCommand() {
          throw new Error('stopped before dispatch');
        },
      },
    });
    const first = await interrupted.accept(request);
    assert.equal(first.status, 'reconciling');
    let calls = 0;
    // Deliberately use the default allow_backorder constructor after restart.
    const downgraded = new NativeMerchantCheckout({
      ...f.options,
      commerce: {
        executeKernelCommand() {
          calls++;
          throw new Error('must not dispatch');
        },
      },
    });
    const blocked = await downgraded.resume(first.id);
    assert.equal(blocked.status, 'reconciling');
    assert.match(blocked.error, /does not advertise/);
    assert.equal(calls, 0);
    const restored = await new NativeMerchantCheckout(f.options).resume(first.id);
    assert.equal(restored.status, 'accepted');
    assert.deepEqual(await downgraded.resume(first.id), restored);
    assert.equal(calls, 0);
  } finally {
    f.close();
  }
});

test('authority denial creates neither an order nor a stock hold', async () => {
  const f = await fixture();
  try {
    const runtime = new NativeMerchantCheckout({
      ...f.options,
      principal: { ...f.options.principal, capabilities: [] },
    });
    assert.equal((await runtime.accept(request)).status, 'rejected');
    assert.equal(f.db.prepare('SELECT COUNT(*) AS count FROM orders').get().count, 0);
    assert.equal((await f.commerce.inventory.getStock('NATIVE-100')).totalAllocated, '0');
  } finally {
    f.close();
  }
});

test('receipt lookup outage fails closed and scoped recovery cannot cross principals', async () => {
  const f = await fixture();
  let calls = 0;
  try {
    const runtime = new NativeMerchantCheckout({
      ...f.options,
      readReceipt: async () => {
        throw new Error('ledger unavailable');
      },
      commerce: {
        executeKernelCommand() {
          calls++;
          throw new Error('must not execute');
        },
      },
    });
    const result = await runtime.accept(request);
    assert.equal(result.status, 'reconciling');
    assert.equal(calls, 0);
    const ambiguous = new NativeMerchantCheckout({
      ...f.options,
      readReceipt: async () => undefined,
      commerce: {
        executeKernelCommand() {
          calls++;
          throw new Error('must not execute');
        },
      },
    });
    assert.equal((await ambiguous.resume(result.id)).status, 'reconciling');
    assert.equal(calls, 0);
    const other = new NativeMerchantCheckout({
      ...f.options,
      principal: { ...f.options.principal, id: 'agent:other' },
    });
    await assert.rejects(other.resume(result.id), /acceptance not found/);
    assert.equal((await f.runtime.resume(result.id)).status, 'accepted');
  } finally {
    f.close();
  }
});

test('concurrent same-key acceptance converges on one native order', async () => {
  const f = await fixture();
  try {
    const results = await Promise.all(Array.from({ length: 4 }, () => f.runtime.accept(request)));
    for (const result of results) assert.equal(result.status, 'accepted', JSON.stringify(result));
    assert.equal(new Set(results.map((result) => result.orderId)).size, 1);
    assert.equal(f.db.prepare('SELECT COUNT(*) AS count FROM orders').get().count, 1);
    assert.equal((await f.commerce.inventory.getStock('NATIVE-100')).totalAllocated, '2');
  } finally {
    f.close();
  }
});

for (const limit of ['24', '25']) {
  test(`native budget limit ${limit} is enforced atomically with checkout`, async () => {
    const f = await fixture();
    try {
      await f.commerce.provisionEconomicBudget({
        budget_id: 'merchant-budget',
        principal_id: f.options.principal.id,
        tenant_id: 'tenant',
        store_id: 'store',
        limit: { amount: limit, currency: 'USD' },
        valid_from: '2020-01-01T00:00:00Z',
        expires_at: '2099-01-01T00:00:00Z',
      });
      f.quote.budgetId = 'merchant-budget';
      f.options.policy.commands['checkout.commit'].requires_budget = true;
      const runtime = new NativeMerchantCheckout(f.options);
      const result = await runtime.accept(request);
      assert.equal(result.status, limit === '25' ? 'accepted' : 'rejected', JSON.stringify(result));
      assert.deepEqual(await runtime.accept(request), result);
      const status = await f.commerce.economicBudgetStatus('merchant-budget');
      assert.equal(amount(status.committed.amount), amount(limit === '25' ? '25' : '0'));
      assert.equal(
        f.db.prepare('SELECT COUNT(*) AS count FROM orders').get().count,
        limit === '25' ? 1 : 0,
      );
      assert.equal(
        (await f.commerce.inventory.getStock('NATIVE-100')).totalAllocated,
        limit === '25' ? '2' : '0',
      );
    } finally {
      f.close();
    }
  });
}

test('native checkout explicitly retains its backorder semantics when stock is short', async () => {
  const f = await fixture(1);
  try {
    const result = await f.runtime.accept(request);
    assert.equal(result.status, 'accepted', JSON.stringify(result));
    assert.equal(f.db.prepare('SELECT COUNT(*) AS count FROM orders').get().count, 1);
    assert.equal((await f.commerce.inventory.getStock('NATIVE-100')).totalAllocated, '1');
    assert.equal(f.db.prepare('SELECT COUNT(*) AS count FROM backorders').get().count, 1);
    assert.deepEqual(await f.runtime.accept(request), result);
    assert.equal(f.db.prepare('SELECT COUNT(*) AS count FROM backorders').get().count, 1);
  } finally {
    f.close();
  }
});

test('native merchant acceptance creates one actual order and reserves inventory exactly once', async () => {
  const f = await fixture();
  try {
    const result = await f.runtime.accept(request);
    assert.equal(result.status, 'accepted', JSON.stringify(result));
    const order = await f.commerce.orders.get(result.orderId);
    assert.equal(order.paymentStatus, 'pending');
    assert.equal(f.db.prepare('SELECT COUNT(*) AS count FROM orders').get().count, 1);
    assert.equal((await f.commerce.inventory.getStock('NATIVE-100')).totalAllocated, '2');
    assert.deepEqual(await f.runtime.accept(request), result);
    await assert.rejects(
      f.runtime.accept({ ...request, idempotencyKey: 'another' }),
      /cart is already bound/,
    );
    await assert.rejects(
      f.runtime.accept({ ...request, quoteId: 'changed' }),
      /idempotency conflict/,
    );
    assert.equal((await f.commerce.inventory.getStock('NATIVE-100')).totalAllocated, '2');
  } finally {
    f.close();
  }
});

test('native checkout preview does not create orders, stock holds or acceptance claims', async () => {
  const f = await fixture();
  try {
    const preview = new NativeMerchantCheckout({ ...f.options, allowApply: false });
    const result = await preview.accept(request);
    assert.equal(result.receipt.status, 'previewed', JSON.stringify(result));
    assert.equal(f.db.prepare('SELECT COUNT(*) AS count FROM orders').get().count, 0);
    assert.equal(f.options.store.collection('native_checkout_operations').size, 0);
    assert.equal((await f.commerce.inventory.getStock('NATIVE-100')).totalAllocated, '0');
    await assert.rejects(preview.resume(result.id), /apply is disabled/);
  } finally {
    f.close();
  }
});

test('cart repricing is rejected by the native transaction without an order or reservation', async () => {
  const f = await fixture();
  try {
    f.quote.amount = '24.99';
    const result = await f.runtime.accept(request);
    assert.equal(result.status, 'rejected', JSON.stringify(result));
    assert.equal(f.db.prepare('SELECT COUNT(*) AS count FROM orders').get().count, 0);
    assert.equal((await f.commerce.inventory.getStock('NATIVE-100')).totalAllocated, '0');
  } finally {
    f.close();
  }
});

test('lost native response is recovered from the kernel receipt without executing twice', async () => {
  const f = await fixture();
  let calls = 0;
  try {
    const faulty = new NativeMerchantCheckout({
      ...f.options,
      commerce: {
        async executeKernelCommand(...args) {
          calls++;
          await f.commerce.executeKernelCommand(...args);
          throw new Error('response lost after kernel commit');
        },
      },
    });
    const first = await faulty.accept(request);
    assert.equal(first.status, 'reconciling');
    const db2 = new Database(f.path);
    try {
      const restarted = new NativeMerchantCheckout({
        ...f.options,
        store: new SqliteProtocolStore(db2),
        commerce: {
          executeKernelCommand() {
            throw new Error('must use committed receipt');
          },
        },
      });
      const recovered = await restarted.accept(request);
      assert.equal(recovered.status, 'accepted', JSON.stringify(recovered));
      assert.equal(calls, 1);
      assert.equal((await f.commerce.inventory.getStock('NATIVE-100')).totalAllocated, '2');
    } finally {
      db2.close();
    }
  } finally {
    f.close();
  }
});
