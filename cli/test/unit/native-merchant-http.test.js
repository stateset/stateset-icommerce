import test from 'node:test';
import assert from 'node:assert/strict';
import { generateKeyPairSync, randomUUID } from 'node:crypto';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import Database from 'better-sqlite3';
import { Commerce } from '../../../bindings/node/index.js';
import { SqliteProtocolStore } from '../../../icp-handler/src/sqlite-store.mjs';
import {
  ICPClient,
  generateIdentity,
  generatePrincipalIdentity,
} from '../../../packages/icp-client/src/index.mjs';
import { amount } from '../../../icp-handler/src/quote-money.mjs';
import { sqliteGet, sqliteOperations } from '../helpers/sqlite-process.js';

const launcher = fileURLToPath(new URL('../../examples/native-merchant.mjs', import.meta.url));
const referenceLauncher = fileURLToPath(
  new URL('../../examples/durable-merchant.mjs', import.meta.url),
);
const config = fileURLToPath(new URL('../fixtures/native-http-config.mjs', import.meta.url));
const merchant = 'aid:v1:zNativeHttpMerchant';
const principal = 'did:web:native.example';

async function kill(proc) {
  if (proc.exitCode !== null || proc.signalCode !== null) return;
  const exited = once(proc, 'exit');
  proc.kill('SIGKILL');
  await exited;
}

async function fixture({ stock = 10, buyers = 1, budget = '100' } = {}) {
  const dir = mkdtempSync(join(tmpdir(), 'native-http-'));
  const path = join(dir, 'commerce.db');
  const protocol = join(dir, 'protocol.db');
  const stripePath = join(dir, 'stripe.json');
  const dispatchPath = join(dir, 'dispatch.json');
  writeFileSync(dispatchPath, 'null');
  const keyFile = join(dir, 'merchant.pem');
  const { privateKey } = generateKeyPairSync('ed25519');
  writeFileSync(keyFile, privateKey.export({ type: 'pkcs8', format: 'pem' }), { mode: 0o600 });
  const authority = generatePrincipalIdentity();
  const identities = Array.from({ length: buyers }, () => generateIdentity());
  const commerce = new Commerce(path);
  await commerce.inventory.createItem({
    sku: 'HTTP-100',
    name: 'HTTP inventory',
    initialQuantity: stock,
  });
  const carts = [];
  for (const identity of identities) {
    const cart = await commerce.carts.create({
      customerEmail: `${identity.aid.slice(-10)}@example.com`,
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
    const now = new Date().toISOString();
    sqliteOperations(path, [
      {
        mode: 'get',
        sql: `INSERT INTO cart_items(id,cart_id,sku,name,quantity,unit_price,total,created_at,updated_at)
        VALUES(?,?,?,?,?,?,?,?,?) RETURNING id`,
        params: [
          randomUUID(),
          cart.id,
          'HTTP-100',
          'HTTP inventory',
          2,
          '12.50',
          '25.00',
          now,
          now,
        ],
      },
      {
        mode: 'get',
        sql: "UPDATE carts SET subtotal='25.00', grand_total='25.00' WHERE id=? RETURNING id",
        params: [cart.id],
      },
    ]);
    carts.push(cart);
  }
  await commerce.provisionEconomicBudget({
    budget_id: 'http-budget',
    principal_id: 'agent:merchant',
    tenant_id: 'tenant',
    store_id: 'store',
    limit: { amount: budget, currency: 'USD' },
    valid_from: '2020-01-01T00:00:00Z',
    expires_at: '2099-01-01T00:00:00Z',
  });
  const workers = [];
  async function start({ apply = true, reference = false, ...extraEnv } = {}) {
    const env = {
      ...process.env,
      PORT: '0',
      ICP_TRUST_MODE: 'enforce',
      ICP_MERCHANT_KEY_FILE: keyFile,
      ICP_MERCHANT_AID: merchant,
      ICP_PRINCIPAL_KEYS_JSON: JSON.stringify({
        [principal]: authority.ed25519_pubkey.toString('hex'),
      }),
      ICP_AGENT_KEYS_JSON: JSON.stringify(
        Object.fromEntries(identities.map((id) => [id.aid, id.ed25519_pubkey.toString('hex')])),
      ),
      NATIVE_COMMERCE_DB: path,
      NATIVE_PROTOCOL_DB: protocol,
      NATIVE_STRIPE_DATA: stripePath,
      NATIVE_DISPATCH_DATA: dispatchPath,
      NATIVE_CART_BINDINGS: JSON.stringify(
        Object.fromEntries(identities.map((id, i) => [id.aid, carts[i].id])),
      ),
      ...extraEnv,
    };
    const proc = spawn(
      process.execPath,
      reference
        ? [referenceLauncher, '--apply', '--demo', '--db', protocol]
        : [launcher, '--config', config, ...(apply ? ['--apply'] : [])],
      { env, stdio: ['ignore', 'pipe', 'pipe'] },
    );
    workers.push(proc);
    proc.stdout.resume();
    let output = '';
    const url = await new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        proc.kill('SIGKILL');
        reject(new Error(`startup timeout: ${output}`));
      }, 15000);
      proc.once('error', (error) => {
        clearTimeout(timer);
        reject(error);
      });
      proc.once('exit', () => {
        clearTimeout(timer);
        reject(new Error(output));
      });
      proc.stderr.on('data', (chunk) => {
        output += chunk;
        const match = output.match(/listening on (http:\/\/127\.0\.0\.1:\d+)/);
        if (match) {
          clearTimeout(timer);
          resolve(match[1]);
        }
      });
    });
    return {
      proc,
      url,
      client: async (i = 0) =>
        ICPClient.create({
          handlerUrl: url,
          principal,
          identity: identities[i],
          principalIdentity: authority,
          maxPerIntent: { amount: '100', currency: 'USD' },
        }),
    };
  }
  return {
    path,
    commerce,
    protocol,
    carts,
    stripePath,
    dispatchPath,
    identities,
    start,
    async close() {
      await Promise.all(workers.map(kill));
      rmSync(dir, { recursive: true });
    },
  };
}

function purchase(client, overrides = {}) {
  return client.purchase({
    merchant,
    settler: 'settler:stateset.usdc.base-sepolia',
    items: [{ sku: 'HTTP-100', quantity: 2, unit_price: { amount: '0.01', currency: 'USD' } }],
    max_total: { amount: '30', currency: 'USD' },
    ...overrides,
  });
}

async function stripePayment(f, status = 'succeeded', index = 0) {
  const snapshot = await f.commerce.checkoutSnapshot(f.carts[index].id);
  const payment = {
    id: 'pi_native',
    object: 'payment_intent',
    amount: 2500,
    amount_received: 2500,
    amount_capturable: 0,
    currency: 'usd',
    livemode: false,
    status,
    client_secret: 'secret-must-not-reach-native-store',
    transfer_data: null,
    on_behalf_of: null,
    metadata: {
      stateset_cart_id: f.carts[index].id,
      stateset_cart_fingerprint: snapshot.fingerprint,
      stateset_buyer: f.identities[index].aid,
    },
    latest_charge: {
      id: 'ch_native',
      object: 'charge',
      payment_intent: 'pi_native',
      amount: 2500,
      amount_captured: 2500,
      amount_refunded: 0,
      currency: 'usd',
      livemode: false,
      paid: true,
      captured: true,
      refunded: false,
      disputed: false,
      status: 'succeeded',
      payment_method_details: { type: 'card' },
      transfer_data: null,
      on_behalf_of: null,
    },
  };
  writeFileSync(f.stripePath, JSON.stringify(payment));
  return payment;
}

function paymentRows(f) {
  return sqliteOperations(f.path, [{ mode: 'all', sql: 'SELECT * FROM payments' }])[0];
}

async function approveDispatch(f, orderId, { reference, quantity } = {}) {
  const receipt = await f.commerce.executeKernelCommand(
    {
      contract_version: '1.0',
      command_id: randomUUID(),
      command_type: 'orders.transition',
      idempotency_key: `prepare:${orderId}`,
      mode: 'apply',
      issued_at: new Date().toISOString(),
      principal: {
        id: 'operator',
        kind: 'agent',
        tenant_id: 'tenant',
        delegated_by: 'company',
        capabilities: ['orders.transition'],
      },
      policy_version: 'dispatch-test',
      store_id: 'store',
      payload: { order_id: orderId, status: 'processing' },
    },
    {
      version: 'dispatch-test',
      commands: { 'orders.transition': { required_capabilities: ['orders.transition'] } },
    },
  );
  assert.equal(receipt.status, 'succeeded', JSON.stringify(receipt));
  const order = await f.commerce.orders.get(orderId);
  const dispatch = {
    reference: reference ?? `dispatch:${orderId}`,
    orderId,
    orderVersion: order.version,
    trackingNumber: 'TRACK-NATIVE-100',
    items: order.items
      .filter((item) => item.shippedQuantity < item.quantity)
      .map((item) => ({
        orderItemId: item.id,
        quantity: quantity ?? item.quantity - item.shippedQuantity,
      })),
  };
  writeFileSync(f.dispatchPath, JSON.stringify(dispatch));
  return dispatch;
}

async function assertShippedOnce(f, orderId) {
  const order = await f.commerce.orders.get(orderId);
  assert.equal(order.status, 'shipped');
  assert.equal(order.items[0].shippedQuantity, 2);
  const stock = await f.commerce.inventory.getStock('HTTP-100');
  assert.equal(Number(stock.totalAllocated), 0);
  assert.equal(Number(stock.totalOnHand), 8);
  assert.equal(Number(stock.totalAvailable), 8);
  assert.equal(
    amount((await f.commerce.economicBudgetStatus('http-budget')).committed.amount),
    amount('25'),
  );
  assert.equal(
    sqliteGet(
      f.path,
      "SELECT COUNT(*) AS n FROM inventory_reservations WHERE reference_id=? AND status='fulfilled'",
      orderId,
    ).n,
    1,
  );
  assert.equal(
    sqliteGet(
      f.path,
      "SELECT COUNT(*) AS n FROM kernel_receipts WHERE command_type='orders.ship_reserved' AND status='succeeded'",
    ).n,
    1,
  );
}

test('native dispatch records shipping once across workers and recovers with resolver offline', async () => {
  const f = await fixture();
  try {
    await stripePayment(f);
    const env = { NATIVE_STRIPE: '1', NATIVE_FULFILLMENT: '1' };
    const a = await f.start(env);
    const b = await f.start(env);
    const ca = await a.client();
    const cb = await b.client();
    const { quote } = await purchase(ca);
    const accepted = await ca.accept(quote.quote_id);
    assert.equal(accepted.fulfillment.status, 'pending');
    await approveDispatch(f, accepted.order.order_id);
    const results = await Promise.all([ca.accept(quote.quote_id), cb.accept(quote.quote_id)]);
    const recorded = results.find((r) => r.fulfillment.status === 'recorded');
    assert.ok(recorded, JSON.stringify(results));
    assert.equal(recorded.order.status, 'shipped');
    assert.equal(recorded.escrow.state, 'pending');
    assert.equal(recorded.funding, null);
    await assertShippedOnce(f, accepted.order.order_id);
    await kill(a.proc);
    await kill(b.proc);
    const restarted = await f.start({
      ...env,
      NATIVE_DISPATCH_OUTAGE: '1',
      NATIVE_STRIPE_OUTAGE: '1',
    });
    assert.deepEqual(await (await restarted.client()).accept(quote.quote_id), recorded);
    await assertShippedOnce(f, accepted.order.order_id);
  } finally {
    await f.close();
  }
});

for (const fault of ['commit', 'projection']) {
  test(`native dispatch recovers after ${fault} loss`, async () => {
    const f = await fixture();
    try {
      await stripePayment(f);
      const env = { NATIVE_STRIPE: '1', NATIVE_FULFILLMENT: '1' };
      const a = await f.start(env);
      const ca = await a.client();
      const { quote } = await purchase(ca);
      const accepted = await ca.accept(quote.quote_id);
      await approveDispatch(f, accepted.order.order_id);
      await kill(a.proc);
      const broken = await f.start({
        ...env,
        ...(fault === 'commit'
          ? { NATIVE_KILL_PAYMENT_STEP: 'orders.ship_reserved' }
          : { NATIVE_FAIL_FULFILLMENT_PROJECTION: '1' }),
      });
      await assert.rejects((await broken.client()).accept(quote.quote_id));
      await kill(broken.proc);
      await assertShippedOnce(f, accepted.order.order_id);
      const recovered = await f.start({ ...env, NATIVE_DISPATCH_OUTAGE: '1' });
      const response = await (await recovered.client()).accept(quote.quote_id);
      assert.equal(response.fulfillment.status, 'recorded');
      await assertShippedOnce(f, accepted.order.order_id);
    } finally {
      await f.close();
    }
  });
}

for (const fault of [
  'unpaid',
  'version',
  'quantity',
  'partial_feature',
  'foreign',
  'policy',
  'race',
  'feature',
]) {
  test(`native dispatch refuses ${fault} evidence without consuming stock`, async () => {
    const f = await fixture();
    try {
      await stripePayment(f, fault === 'unpaid' ? 'processing' : 'succeeded');
      const a = await f.start({
        NATIVE_STRIPE: '1',
        NATIVE_FULFILLMENT: '1',
        NATIVE_DENY_SHIP: fault === 'policy' ? '1' : '0',
        NATIVE_SHIP_VERSION_RACE: fault === 'race' ? '1' : '0',
        NATIVE_LEGACY_SHIP_FEATURE: fault === 'feature' ? '1' : '0',
        NATIVE_LEGACY_PARTIAL_FEATURE: fault === 'partial_feature' ? '1' : '0',
      });
      const client = await a.client();
      const { quote } = await purchase(client);
      const accepted = await client.accept(quote.quote_id);
      const dispatch = await approveDispatch(f, accepted.order.order_id);
      if (fault === 'version') dispatch.orderVersion -= 1;
      if (fault === 'quantity') dispatch.items[0].quantity += 1;
      if (fault === 'partial_feature') dispatch.items[0].quantity = 1;
      if (fault === 'foreign') dispatch.orderId = randomUUID();
      writeFileSync(f.dispatchPath, JSON.stringify(dispatch));
      for (let i = 0; i < 2; i++) {
        const response = await client.accept(quote.quote_id);
        assert.equal(response.fulfillment.status, 'needs_attention', JSON.stringify(response));
      }
      await assertEffects(f, 1);
      assert.equal(
        (
          await f.commerce.orders.get(
            dispatch.orderId === accepted.order.order_id
              ? dispatch.orderId
              : accepted.order.order_id,
          )
        ).items[0].shippedQuantity,
        0,
      );
    } finally {
      await f.close();
    }
  });
}

test('native dispatch receipt outage holds the operation until an authoritative lookup recovers', async () => {
  const f = await fixture();
  try {
    await stripePayment(f);
    const env = { NATIVE_STRIPE: '1', NATIVE_FULFILLMENT: '1' };
    const a = await f.start({ ...env, NATIVE_SHIP_LOOKUP_UNAVAILABLE: '1' });
    const client = await a.client();
    const { quote } = await purchase(client);
    const accepted = await client.accept(quote.quote_id);
    await approveDispatch(f, accepted.order.order_id);
    const pending = await client.accept(quote.quote_id);
    assert.equal(pending.fulfillment.status, 'reconciling');
    assert.equal(
      sqliteGet(
        f.path,
        "SELECT COUNT(*) AS n FROM kernel_receipts WHERE command_type='orders.ship_reserved'",
      ).n,
      0,
    );
    await assertEffects(f, 1);
    await kill(a.proc);
    const restarted = await f.start({ ...env, NATIVE_DISPATCH_OUTAGE: '1' });
    const recovered = await (await restarted.client()).accept(quote.quote_id);
    assert.equal(recovered.fulfillment.status, 'recorded');
    await assertShippedOnce(f, accepted.order.order_id);
  } finally {
    await f.close();
  }
});

test('native dispatch reports receipt-backed payment status without a provider adapter', async () => {
  const f = await fixture();
  try {
    const worker = await f.start({ NATIVE_FULFILLMENT: '1' });
    const client = await worker.client();
    const { quote } = await purchase(client);
    const accepted = await client.accept(quote.quote_id);
    assert.equal(accepted.order.payment_status, 'pending');
    // Operator-owned payment recording outside the optional HTTP provider adapter.
    let paymentId;
    for (const type of ['payments.create', 'payments.complete']) {
      const receipt = await f.commerce.executeKernelCommand(
        {
          contract_version: '1.0',
          command_id: randomUUID(),
          command_type: type,
          idempotency_key: `${type}:${accepted.order.order_id}`,
          mode: 'apply',
          issued_at: new Date().toISOString(),
          policy_version: 'operator-test',
          store_id: 'store',
          principal: {
            id: 'operator',
            kind: 'agent',
            tenant_id: 'tenant',
            delegated_by: 'company',
            capabilities: [type],
          },
          payload:
            type === 'payments.create'
              ? {
                  order_id: accepted.order.order_id,
                  amount: '25.00',
                  currency: 'USD',
                  payment_method: 'credit_card',
                }
              : { payment_id: paymentId },
        },
        { version: 'operator-test', commands: { [type]: { required_capabilities: [type] } } },
      );
      assert.equal(receipt.status, 'succeeded', JSON.stringify(receipt));
      paymentId = receipt.result.id;
    }
    await approveDispatch(f, accepted.order.order_id);
    const recorded = await client.accept(quote.quote_id);
    assert.equal(recorded.fulfillment.status, 'recorded');
    assert.equal(recorded.fulfillment.inventory, 'reserved_stock_consumed');
    assert.equal(recorded.order.payment_status, 'paid');
    assert.equal(recorded.payment, undefined);
    await assertShippedOnce(f, accepted.order.order_id);
  } finally {
    await f.close();
  }
});

test('Stripe reconciliation records one native payment across concurrent HTTP retries', async () => {
  const f = await fixture();
  try {
    const providerPayment = await stripePayment(f, 'processing');
    const a = await f.start({ NATIVE_STRIPE: '1' });
    const b = await f.start({ NATIVE_STRIPE: '1' });
    const ca = await a.client();
    const cb = await b.client();
    const { quote } = await purchase(ca);
    const pending = await ca.accept(quote.quote_id);
    assert.equal(pending.payment.status, 'pending');
    assert.equal(pending.order.payment_status, 'pending');
    assert.equal(paymentRows(f).length, 0);
    providerPayment.status = 'succeeded';
    writeFileSync(f.stripePath, JSON.stringify(providerPayment));
    const results = await Promise.all([ca.accept(quote.quote_id), cb.accept(quote.quote_id)]);
    for (const result of results) {
      assert.equal(result.payment.status, 'recorded', JSON.stringify(result));
      assert.equal(result.order.payment_status, 'paid');
      assert.equal(result.escrow.state, 'pending'); // card capture is not escrow settlement
    }
    assert.deepEqual(results[0], results[1]);
    const rows = paymentRows(f);
    assert.equal(rows.length, 1);
    assert.equal(rows[0].status, 'completed');
    assert.doesNotMatch(JSON.stringify(rows), /secret-must-not/);
    assert.equal((await f.commerce.orders.get(results[0].order.order_id)).paymentStatus, 'paid');
    await assertEffects(f, 1);
    await kill(a.proc);
    await kill(b.proc);
    const restarted = await f.start({ NATIVE_STRIPE: '1', NATIVE_STRIPE_OUTAGE: '1' });
    assert.deepEqual(await (await restarted.client()).accept(quote.quote_id), results[0]);
  } finally {
    await f.close();
  }
});

for (const step of ['payments.create', 'payments.complete']) {
  test(`Stripe capture recovery survives SIGKILL after ${step}`, async () => {
    const f = await fixture();
    try {
      await stripePayment(f);
      const first = await f.start({ NATIVE_STRIPE: '1', NATIVE_KILL_PAYMENT_STEP: step });
      const client = await first.client();
      const { quote } = await purchase(client);
      await assert.rejects(client.accept(quote.quote_id));
      await kill(first.proc);
      assert.equal(paymentRows(f).length, 1);
      const restarted = await f.start({ NATIVE_STRIPE: '1' });
      const recoveredClient = await restarted.client();
      const recovered = await recoveredClient.accept(quote.quote_id);
      assert.equal(recovered.payment.status, 'recorded', JSON.stringify(recovered));
      assert.deepEqual(await recoveredClient.accept(quote.quote_id), recovered);
      assert.equal(paymentRows(f).length, 1);
      assert.equal(paymentRows(f)[0].status, 'completed');
      await assertEffects(f, 1);
    } finally {
      await f.close();
    }
  });
}

test('Stripe mismatches, outages and kernel denial leave orders unpaid', async () => {
  const f = await fixture();
  try {
    const payment = await stripePayment(f);
    payment.amount_received = 2499;
    writeFileSync(f.stripePath, JSON.stringify(payment));
    const first = await f.start({ NATIVE_STRIPE: '1' });
    const client = await first.client();
    const { quote } = await purchase(client);
    const mismatch = await client.accept(quote.quote_id);
    assert.equal(mismatch.payment.status, 'reconciling');
    assert.equal(paymentRows(f).length, 0);
    await kill(first.proc);
    const outage = await f.start({ NATIVE_STRIPE: '1', NATIVE_STRIPE_OUTAGE: '1' });
    assert.equal(
      (await (await outage.client()).accept(quote.quote_id)).payment.status,
      'reconciling',
    );
    assert.equal(paymentRows(f).length, 0);
    await kill(outage.proc);
    payment.amount_received = 2500;
    writeFileSync(f.stripePath, JSON.stringify(payment));
    const denied = await f.start({ NATIVE_STRIPE: '1', NATIVE_DENY_PAYMENT: '1' });
    const result = await (await denied.client()).accept(quote.quote_id);
    assert.equal(result.payment.status, 'needs_attention', JSON.stringify(result));
    assert.equal(result.order.payment_status, 'pending');
    assert.equal((await f.commerce.orders.get(result.order.order_id)).paymentStatus, 'pending');
    assert.equal(paymentRows(f).length, 1);
    assert.equal(paymentRows(f)[0].status, 'pending');
  } finally {
    await f.close();
  }
});

test('one Stripe PaymentIntent cannot satisfy two native orders', async () => {
  const f = await fixture({ buyers: 2 });
  try {
    await stripePayment(f);
    const worker = await f.start({ NATIVE_STRIPE: '1' });
    const a = await worker.client(0);
    const b = await worker.client(1);
    const first = await purchase(a);
    assert.equal((await a.accept(first.quote.quote_id)).payment.status, 'recorded');
    // Even a provider/host mapping rewritten to match the other buyer cannot
    // spend the same external payment again through another quote.
    await stripePayment(f, 'succeeded', 1);
    const second = await purchase(b);
    const refused = await b.accept(second.quote.quote_id);
    assert.equal(refused.payment.status, 'reconciling');
    assert.equal(refused.order.payment_status, 'pending');
    assert.equal(paymentRows(f).length, 1);
    await assertEffects(f, 2);
  } finally {
    await f.close();
  }
});

test('payment and dispatch preview creates no claims or native mutations', async () => {
  const f = await fixture();
  try {
    const worker = await f.start({
      apply: false,
      NATIVE_STRIPE: '1',
      NATIVE_FULFILLMENT: '1',
      NATIVE_DISPATCH_OUTAGE: '1',
    });
    const client = await worker.client();
    const { quote } = await purchase(client);
    assert.equal((await client.accept(quote.quote_id)).checkout.status, 'preview');
    assert.equal(paymentRows(f).length, 0);
    const db = new Database(f.protocol);
    try {
      const store = new SqliteProtocolStore(db);
      assert.equal(store.collection('native_payment_operations').size, 0);
      assert.equal(store.collection('native_payment_claims').size, 0);
      assert.equal(store.collection('native_fulfillment_operations').size, 0);
      assert.equal(store.collection('native_fulfillment_claims').size, 0);
    } finally {
      db.close();
    }
    await assertEffects(f, 0);
  } finally {
    await f.close();
  }
});

test('Stripe capture repairs a failed protocol checkpoint without provider access', async () => {
  const f = await fixture();
  try {
    await stripePayment(f);
    const first = await f.start({ NATIVE_STRIPE: '1', NATIVE_FAIL_PROJECTION: '1' });
    const client = await first.client();
    const { quote } = await purchase(client);
    await assert.rejects(client.accept(quote.quote_id), /internal.transaction_failed/);
    await kill(first.proc);
    assert.equal(paymentRows(f)[0].status, 'completed');
    const restarted = await f.start({ NATIVE_STRIPE: '1', NATIVE_STRIPE_OUTAGE: '1' });
    const recovered = await (await restarted.client()).accept(quote.quote_id);
    assert.equal(recovered.payment.status, 'recorded');
    assert.equal(recovered.order.payment_status, 'paid');
    assert.equal(paymentRows(f).length, 1);
    await assertEffects(f, 1);
  } finally {
    await f.close();
  }
});

async function assertEffects(f, orders) {
  assert.equal(sqliteGet(f.path, 'SELECT COUNT(*) AS n FROM orders').n, orders);
  assert.equal(
    (await f.commerce.inventory.getStock('HTTP-100')).totalAllocated,
    String(orders * 2),
  );
  assert.equal(
    amount((await f.commerce.economicBudgetStatus('http-budget')).committed.amount),
    amount(String(orders * 25)),
  );
  assert.equal(sqliteGet(f.path, 'SELECT COUNT(*) AS n FROM backorders').n, 0);
  const journal = new Database(f.protocol);
  try {
    assert.equal(new SqliteProtocolStore(journal).collection('reservations').size, 0);
  } finally {
    journal.close();
  }
}

test('HTTP native checkout defaults to preview and binds authoritative terms', async () => {
  const f = await fixture();
  try {
    const worker = await f.start({ apply: false });
    const client = await worker.client();
    const caps = await client.capabilities();
    assert.deepEqual(caps.capabilities.verbs, ['purchase.create']);
    assert.equal(caps.trust_mode, 'enforce');
    const { quote } = await purchase(client);
    assert.equal(amount(quote.total.amount), amount('25')); // caller's penny price is never used
    assert.match(quote.cart_fingerprint, /^sha256:[a-f0-9]{64}$/);
    const preview = await client.accept(quote.quote_id);
    assert.equal(preview.checkout.status, 'preview');
    assert.equal(preview.checkout.verdict, 'previewed');
    assert.equal(preview.order, null);
    await assertEffects(f, 0);
    await assert.rejects(
      purchase(client, { max_total: { amount: '24.99', currency: 'USD' } }),
      /exceeds_max_total/,
    );
    await assert.rejects(
      purchase(client, { items: [{ sku: 'OTHER', quantity: 2 }] }),
      /items_mismatch/,
    );
    await assert.rejects(purchase(client, { merchant: 'another-merchant' }), /merchant_mismatch/);
    client.maxPerIntent = { amount: '24.99', currency: 'USD' };
    await assert.rejects(purchase(client), /delegation.amount_exceeded/);
    client.maxPerIntent = { amount: '100', currency: 'USDC' };
    await assert.rejects(purchase(client), /delegation.amount_exceeded/);
  } finally {
    await f.close();
  }
});

test('HTTP native checkout rejects changed quote terms without economic effects', async () => {
  const f = await fixture();
  try {
    const worker = await f.start();
    const client = await worker.client();
    const { quote } = await purchase(client);
    sqliteGet(
      f.path,
      "UPDATE carts SET shipping_address=json_set(shipping_address,'$.line1','Changed') WHERE id=? RETURNING id",
      f.carts[0].id,
    );
    await assert.rejects(client.accept(quote.quote_id), /checkout.conflict/);
    await assertEffects(f, 0);
    const replacement = await purchase(client);
    const accepted = await client.accept(replacement.quote.quote_id);
    assert.equal(accepted.checkout.status, 'accepted');
    await assertEffects(f, 1);
  } finally {
    await f.close();
  }
});

test('an expired HTTP quote cannot start a new native checkout', async () => {
  const f = await fixture();
  try {
    const first = await f.start();
    const { quote } = await purchase(await first.client());
    await kill(first.proc);
    const restarted = await f.start({ NATIVE_NOW: String(Date.parse(quote.exp) + 1000) });
    await assert.rejects((await restarted.client()).accept(quote.quote_id), /replay.expired/);
    await assertEffects(f, 0);
  } finally {
    await f.close();
  }
});

test('two HTTP workers converge on one acceptance and refuse simulated settlement', async () => {
  const f = await fixture();
  try {
    const a = await f.start();
    const b = await f.start();
    const ca = await a.client();
    const cb = await b.client();
    const { quote } = await purchase(ca);
    const results = await Promise.all([
      ca.accept(quote.quote_id),
      cb.accept(quote.quote_id),
      ca.accept(quote.quote_id),
    ]);
    assert.equal(results[0].checkout.status, 'accepted');
    results.forEach((value) => assert.deepEqual(value, results[0]));
    const accepted = results[0];
    assert.equal(accepted.order.payment_status, 'pending');
    assert.equal(accepted.funding, null);
    const order = await f.commerce.orders.get(accepted.order.order_id);
    assert.equal(order.paymentStatus, 'pending');
    await assertEffects(f, 1);
    const stranger = await ICPClient.create({ handlerUrl: a.url, principal });
    await assert.rejects(stranger.accept(quote.quote_id), /auth.acceptance_invalid/);
    for (const path of [
      `escrows/${accepted.escrow.escrow_id}/fulfill`,
      `escrows/${accepted.escrow.escrow_id}/dispute`,
      'settlements/cosign',
    ]) {
      const response = await fetch(`${a.url}/icp/v1/${path}`, { method: 'POST', body: '{}' });
      assert.equal(response.status, 503);
    }
    await kill(a.proc);
    await kill(b.proc);
    const restarted = await f.start();
    assert.deepEqual(await (await restarted.client()).accept(quote.quote_id), accepted);
    await assert.rejects(f.start({ reference: true }), /backend does not match/);
  } finally {
    await f.close();
  }
});

for (const constraint of ['stock', 'budget']) {
  test(`competing HTTP buyers cannot exceed shared ${constraint}`, async () => {
    const f = await fixture({
      buyers: 2,
      stock: constraint === 'stock' ? 2 : 10,
      budget: constraint === 'budget' ? '25' : '100',
    });
    try {
      const a = await f.start();
      const b = await f.start();
      const clients = [await a.client(0), await b.client(1)];
      const quotes = await Promise.all(clients.map((client) => purchase(client)));
      const results = await Promise.allSettled(
        clients.map((client, i) => client.accept(quotes[i].quote.quote_id)),
      );
      assert.equal(
        results.filter((r) => r.status === 'fulfilled').length,
        1,
        JSON.stringify(results),
      );
      assert.match(
        String(results.find((r) => r.status === 'rejected').reason),
        constraint === 'stock' ? /insufficient_available/ : /budget/,
      );
      await assertEffects(f, 1);
    } finally {
      await f.close();
    }
  });
}

for (const fault of [
  'NATIVE_KILL_AFTER_COMMIT',
  'NATIVE_FAIL_PROJECTION',
  'NATIVE_LOOKUP_UNAVAILABLE',
]) {
  test(`HTTP acceptance recovers after ${fault} without duplicate effects`, async () => {
    const f = await fixture();
    try {
      const first = await f.start({ [fault]: '1' });
      const client = await first.client();
      const { quote } = await purchase(client);
      if (fault === 'NATIVE_LOOKUP_UNAVAILABLE') {
        const pending = await client.accept(quote.quote_id);
        assert.equal(pending.checkout.status, 'reconciling');
        assert.equal(pending.order, null);
      } else {
        await assert.rejects(client.accept(quote.quote_id));
      }
      await kill(first.proc);
      await assertEffects(f, fault === 'NATIVE_LOOKUP_UNAVAILABLE' ? 0 : 1);
      // A committed receipt remains recoverable after quote expiry; the
      // lookup outage case still has an undispatched, unexpired command.
      const restarted = await f.start(
        fault === 'NATIVE_LOOKUP_UNAVAILABLE'
          ? {}
          : { NATIVE_NOW: String(Date.parse(quote.exp) + 1000) },
      );
      const recoveredClient = await restarted.client();
      const recovered = await recoveredClient.accept(quote.quote_id);
      assert.equal(recovered.checkout.status, 'accepted');
      assert.deepEqual(await recoveredClient.accept(quote.quote_id), recovered);
      await assertEffects(f, 1);
      const journal = new Database(f.protocol);
      try {
        const store = new SqliteProtocolStore(journal);
        assert.equal(store.collection('escrows').size, 1);
        assert.equal(store.collection('events').values()[0].length, 1);
      } finally {
        journal.close();
      }
    } finally {
      await f.close();
    }
  });
}

test('native kernel consumes partial dispatches while replaying historical receipts unchanged', async () => {
  const f = await fixture();
  try {
    assert.ok((await f.commerce.kernelFeatures()).includes('orders.ship_reserved.partial.v1'));
    await stripePayment(f, 'succeeded');
    const worker = await f.start({ NATIVE_STRIPE: '1' });
    const client = await worker.client();
    const { quote } = await purchase(client);
    const accepted = await client.accept(quote.quote_id);
    const dispatch = await approveDispatch(f, accepted.order.order_id);
    const policy = {
      version: 'partial-test',
      commands: { 'orders.ship_reserved': { required_capabilities: ['orders.ship_reserved'] } },
    };
    const command = {
      contract_version: '1.0',
      command_id: randomUUID(),
      command_type: 'orders.ship_reserved',
      idempotency_key: `partial:${dispatch.orderId}:one`,
      mode: 'apply',
      issued_at: new Date().toISOString(),
      principal: {
        id: 'operator',
        kind: 'agent',
        tenant_id: 'tenant',
        delegated_by: 'company',
        capabilities: ['orders.ship_reserved'],
      },
      policy_version: policy.version,
      store_id: 'store',
      expected_version: dispatch.orderVersion,
      commitment: { quantity: '1' },
      payload: {
        order_id: dispatch.orderId,
        tracking_number: 'PARTIAL-ONE',
        lines: [{ order_item_id: dispatch.items[0].orderItemId, quantity: 1 }],
      },
    };
    const first = await f.commerce.executeKernelCommand(command, policy);
    assert.equal(first.status, 'succeeded', JSON.stringify(first));
    assert.equal(first.result.status, 'partially_shipped');
    assert.equal(first.result.items[0].shipped_quantity, 1);
    assert.deepEqual(
      sqliteGet(
        f.path,
        'SELECT quantity_on_hand,quantity_allocated,quantity_available FROM inventory_balances',
      ),
      {
        quantity_on_hand: '9',
        quantity_allocated: '1',
        quantity_available: '8',
      },
    );
    const second = structuredClone(command);
    second.command_id = randomUUID();
    second.idempotency_key = `partial:${dispatch.orderId}:two`;
    second.expected_version = first.version_after;
    second.payload.tracking_number = 'PARTIAL-TWO';
    const finished = await f.commerce.executeKernelCommand(second, policy);
    assert.equal(finished.status, 'succeeded', JSON.stringify(finished));
    assert.equal(finished.result.status, 'shipped');
    assert.equal(finished.result.items[0].shipped_quantity, 2);
    const replay = await f.commerce.executeKernelCommand(command, policy);
    assert.equal(replay.receipt_id, first.receipt_id);
    assert.equal(replay.result.status, 'partially_shipped');
    assert.deepEqual(
      sqliteGet(
        f.path,
        'SELECT quantity_on_hand,quantity_allocated,quantity_available FROM inventory_balances',
      ),
      {
        quantity_on_hand: '8',
        quantity_allocated: '0',
        quantity_available: '8',
      },
    );
    assert.equal(
      sqliteGet(
        f.path,
        "SELECT COUNT(*) n FROM inventory_transactions WHERE transaction_type='shipment'",
      ).n,
      2,
    );
  } finally {
    await f.close();
  }
});

function assertDispatchProgress(f, shipped, dispatches) {
  assert.deepEqual(
    sqliteGet(
      f.path,
      'SELECT quantity_on_hand,quantity_allocated,quantity_available FROM inventory_balances',
    ),
    {
      quantity_on_hand: String(10 - shipped),
      quantity_allocated: String(2 - shipped),
      quantity_available: '8',
    },
  );
  assert.equal(
    sqliteGet(
      f.path,
      "SELECT COUNT(*) n FROM kernel_receipts WHERE command_type='orders.ship_reserved' AND status='succeeded'",
    ).n,
    dispatches,
  );
  assert.equal(
    sqliteGet(
      f.path,
      "SELECT COUNT(*) n FROM inventory_transactions WHERE transaction_type='shipment'",
    ).n,
    dispatches,
  );
}

test('HTTP partial dispatches converge across workers and advance to shipped after restart', async () => {
  const f = await fixture();
  try {
    await stripePayment(f);
    const env = { NATIVE_STRIPE: '1', NATIVE_FULFILLMENT: '1' };
    const a = await f.start(env);
    const b = await f.start(env);
    const ca = await a.client();
    const cb = await b.client();
    const { quote } = await purchase(ca);
    const accepted = await ca.accept(quote.quote_id);
    const firstApproval = await approveDispatch(f, accepted.order.order_id, {
      reference: 'warehouse:first',
      quantity: 1,
    });
    const responses = await Promise.all([ca.accept(quote.quote_id), cb.accept(quote.quote_id)]);
    const partial = responses.find((r) => r.order.status === 'partially_shipped');
    assert.ok(partial, JSON.stringify(responses));
    assert.equal(partial.fulfillment.items[0].shipped_quantity, 1);
    assert.equal(partial.order.payment_status, 'paid');
    assert.equal(partial.funding, null);
    assert.equal(partial.escrow.state, 'pending');
    assertDispatchProgress(f, 1, 1);
    assert.equal(
      (await ca.accept(quote.quote_id)).fulfillment.order_version,
      partial.fulfillment.order_version,
    );
    await kill(a.proc);
    await kill(b.proc);
    const offline = await f.start({ ...env, NATIVE_DISPATCH_OUTAGE: '1' });
    const historical = await (await offline.client()).accept(quote.quote_id);
    assert.equal(historical.order.status, 'partially_shipped');
    assert.equal(historical.fulfillment.next_dispatch_status, 'reconciling');
    assertDispatchProgress(f, 1, 1);
    await kill(offline.proc);
    await approveDispatch(f, accepted.order.order_id, { reference: 'warehouse:second' });
    const c = await f.start(env);
    const d = await f.start(env);
    const cc = await c.client();
    const cd = await d.client();
    await Promise.all([cc.accept(quote.quote_id), cd.accept(quote.quote_id)]);
    const finished = await cc.accept(quote.quote_id);
    assert.equal(finished.order.status, 'shipped');
    assert.equal(finished.fulfillment.items[0].shipped_quantity, 2);
    assert.ok(finished.fulfillment.order_version > partial.fulfillment.order_version);
    assert.equal(finished.fulfillment.next_dispatch_status, undefined);
    assertDispatchProgress(f, 2, 2);
    writeFileSync(f.dispatchPath, JSON.stringify(firstApproval));
    assert.deepEqual(await cd.accept(quote.quote_id), finished);
    assertDispatchProgress(f, 2, 2);
  } finally {
    await f.close();
  }
});

for (const fault of ['commit', 'projection']) {
  test(`HTTP partial shipment recovers after ${fault} loss with resolver offline`, async () => {
    const f = await fixture();
    try {
      await stripePayment(f);
      const env = { NATIVE_STRIPE: '1', NATIVE_FULFILLMENT: '1' };
      const a = await f.start(env);
      const client = await a.client();
      const { quote } = await purchase(client);
      const accepted = await client.accept(quote.quote_id);
      await approveDispatch(f, accepted.order.order_id, {
        reference: 'warehouse:partial',
        quantity: 1,
      });
      await kill(a.proc);
      const broken = await f.start({
        ...env,
        ...(fault === 'commit'
          ? { NATIVE_KILL_PAYMENT_STEP: 'orders.ship_reserved' }
          : { NATIVE_FAIL_FULFILLMENT_PROJECTION: '1' }),
      });
      await assert.rejects((await broken.client()).accept(quote.quote_id));
      await kill(broken.proc);
      assertDispatchProgress(f, 1, 1);
      const restarted = await f.start({ ...env, NATIVE_DISPATCH_OUTAGE: '1' });
      const restored = await (await restarted.client()).accept(quote.quote_id);
      assert.equal(restored.order.status, 'partially_shipped');
      assert.equal(restored.fulfillment.items[0].shipped_quantity, 1);
      assertDispatchProgress(f, 1, 1);
      await kill(restarted.proc);
      await approveDispatch(f, accepted.order.order_id, { reference: 'warehouse:remainder' });
      const healthy = await f.start(env);
      const done = await (await healthy.client()).accept(quote.quote_id);
      assert.equal(done.order.status, 'shipped');
      assertDispatchProgress(f, 2, 2);
    } finally {
      await f.close();
    }
  });
}

test('HTTP receipt outage retains a subsequent pinned dispatch and prior progress', async () => {
  const f = await fixture();
  try {
    await stripePayment(f);
    const env = { NATIVE_STRIPE: '1', NATIVE_FULFILLMENT: '1' };
    const a = await f.start(env);
    const client = await a.client();
    const { quote } = await purchase(client);
    const accepted = await client.accept(quote.quote_id);
    await approveDispatch(f, accepted.order.order_id, { reference: 'warehouse:one', quantity: 1 });
    const first = await client.accept(quote.quote_id);
    await approveDispatch(f, accepted.order.order_id, { reference: 'warehouse:two' });
    await kill(a.proc);
    const unavailable = await f.start({ ...env, NATIVE_NEW_SHIP_LOOKUP_UNAVAILABLE: '1' });
    const held = await (await unavailable.client()).accept(quote.quote_id);
    assert.equal(held.order.status, 'partially_shipped');
    assert.equal(held.fulfillment.order_version, first.fulfillment.order_version);
    assert.equal(held.fulfillment.next_dispatch_status, 'reconciling');
    assertDispatchProgress(f, 1, 1);
    await kill(unavailable.proc);
    assert.equal(
      sqliteGet(
        f.protocol,
        "SELECT COUNT(*) n FROM _icp_records WHERE namespace='native_fulfillment_operations'",
      ).n,
      2,
    );
    const healthy = await f.start({ ...env, NATIVE_DISPATCH_OUTAGE: '1' });
    assert.equal((await (await healthy.client()).accept(quote.quote_id)).order.status, 'shipped');
    assertDispatchProgress(f, 2, 2);
  } finally {
    await f.close();
  }
});

for (const fault of ['commit', 'projection']) {
  test(`HTTP final dispatch recovers after ${fault} loss without regressing partial progress`, async () => {
    const f = await fixture();
    try {
      await stripePayment(f);
      const env = { NATIVE_STRIPE: '1', NATIVE_FULFILLMENT: '1' };
      const a = await f.start(env);
      const client = await a.client();
      const { quote } = await purchase(client);
      const accepted = await client.accept(quote.quote_id);
      await approveDispatch(f, accepted.order.order_id, {
        reference: 'warehouse:initial',
        quantity: 1,
      });
      const partial = await client.accept(quote.quote_id);
      assert.equal(partial.order.status, 'partially_shipped');
      await approveDispatch(f, accepted.order.order_id, { reference: 'warehouse:final' });
      await kill(a.proc);
      const broken = await f.start({
        ...env,
        ...(fault === 'commit'
          ? { NATIVE_KILL_PAYMENT_STEP: 'orders.ship_reserved' }
          : { NATIVE_FAIL_FULFILLMENT_PROJECTION: '1' }),
      });
      await assert.rejects((await broken.client()).accept(quote.quote_id));
      await kill(broken.proc);
      assertDispatchProgress(f, 2, 2);
      const restarted = await f.start({ ...env, NATIVE_DISPATCH_OUTAGE: '1' });
      const restored = await (await restarted.client()).accept(quote.quote_id);
      assert.equal(restored.order.status, 'shipped');
      assert.equal(restored.fulfillment.items[0].shipped_quantity, 2);
      assert.ok(restored.fulfillment.order_version > partial.fulfillment.order_version);
      assert.deepEqual(await (await restarted.client()).accept(quote.quote_id), restored);
      assertDispatchProgress(f, 2, 2);
    } finally {
      await f.close();
    }
  });
}
