#!/usr/bin/env node
// Local recovery demonstration. Agent clients and economic providers are
// simulated. No network, model calls, wallets or real money are involved.
import assert from 'node:assert/strict';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import Database from 'better-sqlite3';
import { PurchaseRuntime, SqlitePurchaseStore } from '../../bindings/node/purchase-runtime.mjs';

const apply = process.argv.includes('--apply');
const path = join(mkdtempSync(join(tmpdir(), 'stateset-purchase-demo-')), 'demo.db');
let db;
let losePaymentResponse = true;

function open() {
  db = new Database(path);
  db.pragma('journal_mode = WAL');
  db.pragma('busy_timeout = 5000');
  db.exec(`CREATE TABLE IF NOT EXISTS demo_provider_effects (
    idempotency_key TEXT PRIMARY KEY, step TEXT NOT NULL, result TEXT NOT NULL
  )`);
}

const evidence = {
  reserve_inventory: { reservation_id: 'demo:reservation:50-units' },
  pay: { transaction_id: 'SIMULATED:payment:4650', amount: '4650', asset: 'USDC' },
  create_order: { order_id: 'SIMULATED:order:50-units' },
  confirm_inventory: { reservation_id: 'demo:reservation:50-units' },
  release_inventory: { reservation_id: 'demo:reservation:50-units' },
};

// This durable table simulates a provider's idempotency and lookup service.
// Production adapters must get these guarantees from the actual provider.
const adapters = Object.fromEntries(
  Object.entries(evidence).map(([step, value]) => [
    step,
    {
      async lookup({ idempotencyKey }) {
        const row = db
          .prepare('SELECT result FROM demo_provider_effects WHERE idempotency_key=?')
          .get(idempotencyKey);
        return row ? JSON.parse(row.result) : { status: 'not_found' };
      },
      async execute(context) {
        const result = { status: 'succeeded', evidence: value };
        db.prepare('INSERT OR IGNORE INTO demo_provider_effects VALUES(?,?,?)').run(
          context.idempotencyKey,
          step,
          JSON.stringify(result),
        );
        if (step === 'pay' && losePaymentResponse) {
          losePaymentResponse = false;
          throw new Error('DEMO: provider committed payment, response was lost');
        }
        return this.lookup(context);
      },
    },
  ]),
);

function agent(agentId) {
  return new PurchaseRuntime({
    store: new SqlitePurchaseStore(db),
    identity: { agentId, principalId: 'acme', tenantId: 'demo', storeId: 'demo' },
    policyVersion: 'demo-procurement-v1',
    adapters,
    allowApply: apply,
    resolveQuote: async (id) => ({
      id,
      counterpartyId: 'demo-merchant',
      amount: '4650',
      asset: 'USDC',
      expiresAt: '2099-01-01T00:00:00Z',
      items: [{ sku: 'SKU-100', quantity: 50 }],
    }),
    authorize: async () => ({ allowed: true, decisionId: 'DEMO:fixed-policy' }),
  });
}

open();
try {
  console.log(`SIMULATED providers; no real funds. Database retained at ${path}`);
  let buyer = agent('buyer:one');
  buyer.store.provisionBudget(buyer.scope, {
    id: 'shared',
    limit: '5000',
    asset: 'USDC',
    expiresAt: '2099-01-01T00:00:00Z',
  });
  const request = {
    idempotencyKey: 'buy-50',
    quoteId: 'quote:50',
    budgetId: 'shared',
    maxAmount: '5000',
    asset: 'USDC',
  };
  const first = await buyer.buy(request);
  console.log(`Buyer one: ${first.status}`);
  if (!apply) {
    assert.equal(first.status, 'preview');
    assert.equal(db.prepare('SELECT COUNT(*) AS count FROM demo_provider_effects').get().count, 0);
    console.log('Preview only. Run with --apply to exercise local simulated recovery.');
  } else {
    assert.equal(first.status, 'reconciling');
    const competitors = await Promise.allSettled(
      ['buyer:two', 'buyer:three'].map((id) => agent(id).buy(request)),
    );
    for (const contender of competitors) {
      assert.equal(contender.status, 'rejected');
      assert.match(contender.reason.message, /budget exceeded/);
    }
    console.log(
      'Two competing buyers rejected: the shared budget retains the pending payment hold.',
    );
    db.close();
    open(); // Discard the coordinator and reconnect to the persisted checkpoint.
    buyer = agent('buyer:one');
    // The worker rediscovers unfinished work; it does not need an in-memory ID.
    const batch = await buyer.recover({ limit: 10 });
    assert.equal(batch.results.length, 1);
    const recovered = batch.results[0].operation;
    assert.equal(recovered.status, 'completed');
    assert.equal(
      db.prepare("SELECT COUNT(*) AS count FROM demo_provider_effects WHERE step='pay'").get()
        .count,
      1,
    );
    assert.equal(buyer.store.budget(buyer.scope, 'shared').spent, '4650');
    console.log(
      'Reopened SQLite: purchase completed, one simulated payment, shared balance 350 USDC.',
    );
    console.log(JSON.stringify(recovered.receipt, null, 2));
    console.log('Receipt above is a local digest summary, not a signed settlement receipt.');
  }
} finally {
  db.close();
}
