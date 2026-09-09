// createKernelPurchaseAdapter builds the kernel command a purchase step
// executes. It carried no economic commitment, so the kernel had nothing to
// evaluate money policy against: every rule with a `max_amount` rejected the
// command outright with `policy.commitment_amount_required`, whatever the
// quote said. A governed kernel could therefore not execute any purchase.
//
// Runs against the real native kernel, like native-merchant-checkout.test.js.
import assert from 'node:assert/strict';
import test from 'node:test';
import { randomUUID } from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import Database from 'better-sqlite3';
import { Commerce } from '../../../bindings/node/index.js';
import {
  createKernelPurchaseAdapter,
  quoteCommitment,
} from '../../../bindings/node/purchase-runtime.mjs';

/** A commerce database with one stocked SKU and one $25.00 cart. */
async function fixture() {
  const dir = mkdtempSync(join(tmpdir(), 'stateset-purchase-commitment-'));
  const path = join(dir, 'commerce.db');
  const commerce = new Commerce(path);
  await commerce.inventory.createItem({ sku: 'SKU-500', name: 'Widget', initialQuantity: 500 });
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
  db.prepare(
    `INSERT INTO cart_items(id,cart_id,sku,name,quantity,unit_price,total,created_at,updated_at)
     VALUES(?,?,?,?,?,?,?,?,?)`,
  ).run(randomUUID(), cart.id, 'SKU-500', 'Widget', 2, '12.50', '25.00', now, now);
  db.prepare("UPDATE carts SET subtotal='25.00', grand_total='25.00' WHERE id=?").run(cart.id);

  const readReceipt = async (key) => {
    const row = db.prepare('SELECT receipt FROM kernel_receipts WHERE idempotency_key=?').get(key);
    return row ? JSON.parse(row.receipt) : null;
  };
  const adapterFor = (commandType, rule, buildPayload, evidence) =>
    createKernelPurchaseAdapter({
      commerce,
      policy: {
        version: 'commitment-v1',
        commands: { [commandType]: { required_capabilities: [commandType], ...rule } },
        trusted_authority_keys: {},
      },
      principal: {
        id: 'buyer',
        kind: 'agent',
        tenant_id: 'tenant',
        delegated_by: 'company',
        capabilities: [commandType],
      },
      storeId: 'store',
      commandType,
      buildPayload,
      readReceipt,
      evidence,
    });
  return {
    cartId: cart.id,
    adapterFor,
    close() {
      db.close();
      rmSync(dir, { recursive: true });
    },
  };
}

function operation(amount, overrides = {}) {
  return {
    createdAt: new Date().toISOString(),
    request: { budgetId: 'monthly' },
    quote: {
      id: 'quote:commitment',
      counterpartyId: 'merchant:acme',
      amount,
      currency: 'USD',
      expiresAt: '2099-01-01T00:00:00Z',
      ...overrides,
    },
  };
}

test('a quote under the policy cap executes; over the cap the kernel rejects it', async () => {
  const f = await fixture();
  const adapter = f.adapterFor(
    'checkout.commit',
    { max_amount: { amount: '50.00', currency: 'USD' } },
    () => ({ cart_id: f.cartId }),
    (receipt) => ({ order_id: receipt.result.order_id }),
  );
  try {
    // Above the cap: rejected on policy, before the cart is touched.
    const over = await adapter.execute({
      operation: operation('75.00'),
      idempotencyKey: 'purchase:over',
    });
    assert.equal(over.status, 'failed');
    assert.match(over.reason, /policy\.max_amount_exceeded/);
    assert.doesNotMatch(over.reason, /commitment_amount_required/);

    // Under the cap: the quoted amount is what the kernel authorizes.
    const under = await adapter.execute({
      operation: operation('25.00'),
      idempotencyKey: 'purchase:under',
    });
    assert.equal(under.status, 'succeeded', JSON.stringify(under));
    const receipt = under.evidence.kernel_receipt;
    assert.equal(receipt.status, 'succeeded');
    assert.deepEqual(receipt.economic_context.commitment.amount, {
      amount: '25.00',
      currency: 'USD',
    });
    assert.ok(under.evidence.order_id);
  } finally {
    f.close();
  }
});

test('a quantity cap is enforced against the quantity the quote committed', async () => {
  const f = await fixture();
  const adapter = f.adapterFor(
    'inventory.reserve',
    { max_quantity: '10' },
    (op) => ({
      sku: 'SKU-500',
      quantity: op.quote.quantity,
      reference_type: 'purchase',
      reference_id: 'commitment-test',
    }),
    (receipt) => ({ reservation_id: receipt.aggregate_id }),
  );
  try {
    const allowed = await adapter.execute({
      operation: operation('25.00', { quantity: '4' }),
      idempotencyKey: 'purchase:allowed',
    });
    assert.equal(allowed.status, 'succeeded', JSON.stringify(allowed));

    const bulk = await adapter.execute({
      operation: operation('25.00', { quantity: '40' }),
      idempotencyKey: 'purchase:bulk',
    });
    assert.equal(bulk.status, 'failed');
    assert.match(bulk.reason, /policy\.max_quantity_exceeded/);
    assert.doesNotMatch(bulk.reason, /commitment_quantity_required/);
  } finally {
    f.close();
  }
});

test('the derived commitment mirrors the accepted quote', () => {
  assert.deepEqual(quoteCommitment(operation('25.00', { quantity: '3' })), {
    // `request.budgetId` is the runtime's own token ledger, NOT a kernel
    // economic budget: naming it would fail with kernel.budget_not_found.
    budget_id: null,
    amount: { amount: '25.00', currency: 'USD' },
    asset_amount: null,
    counterparty_id: 'merchant:acme',
    quantity: '3',
    evidence: ['quote:commitment'],
  });
  assert.equal(
    quoteCommitment(operation('25.00', { budgetId: 'kernel-budget' })).budget_id,
    'kernel-budget',
  );

  // A token-denominated quote commits the asset, and the kernel refuses a
  // fiat budget alongside one.
  const token = operation('40', {
    currency: undefined,
    asset: 'eip155:1/erc20:0x0000000000000000000000000000000000000001',
  });
  assert.deepEqual(quoteCommitment(token), {
    budget_id: null,
    amount: null,
    asset_amount: {
      amount: '40',
      asset: 'eip155:1/erc20:0x0000000000000000000000000000000000000001',
    },
    counterparty_id: 'merchant:acme',
    quantity: null,
    evidence: ['quote:commitment'],
  });

  // No quote (a host driving the adapter directly) commits nothing.
  assert.equal(quoteCommitment({ createdAt: 'now' }), null);
  assert.equal(quoteCommitment(undefined), null);
  // A quote whose amount is not an exact decimal is never smuggled through.
  assert.throws(() => quoteCommitment(operation(12.5)), /exact nonnegative decimal/);
  assert.throws(() => quoteCommitment(operation('-1')), /exact nonnegative decimal/);
});

test('a host-supplied mandate rides on the same envelope', async () => {
  const captured = [];
  const adapter = createKernelPurchaseAdapter({
    commerce: {
      async executeKernelCommand(command) {
        captured.push(command);
        return {
          command_type: command.command_type,
          idempotency_key: command.idempotency_key,
          status: 'succeeded',
          aggregate_id: 'res:1',
        };
      },
    },
    policy: { version: 'v1', commands: {}, trusted_authority_keys: {} },
    principal: { id: 'buyer', kind: 'agent', tenant_id: 't', capabilities: [] },
    storeId: 'store',
    commandType: 'inventory.reserve',
    buildPayload: () => ({}),
    readReceipt: async () => null,
    evidence: () => ({ reservation_id: 'res:1' }),
    mandate: (op) => ({ mandate_id: `mandate:${op.quote.id}` }),
  });
  const result = await adapter.execute({
    operation: operation('25.00'),
    idempotencyKey: 'purchase:mandate',
  });
  assert.equal(result.status, 'succeeded');
  assert.deepEqual(captured[0].mandate, { mandate_id: 'mandate:quote:commitment' });
  assert.equal(captured[0].commitment.amount.amount, '25.00');
});
