// Durable provider double: exercises process death, not live chain settlement.
import Database from 'better-sqlite3';
import {
  createAgentCommerce,
  SqlitePurchaseStore,
} from '../../../bindings/node/purchase-runtime.mjs';

const [directory, mode] = process.argv.slice(2);
const db = new Database(`${directory}/buyer.db`);
const provider = new Database(`${directory}/provider.db`);
for (const connection of [db, provider]) {
  connection.pragma('journal_mode = WAL');
  connection.pragma('synchronous = FULL');
  connection.pragma('busy_timeout = 5000');
}
provider.exec(`
  CREATE TABLE IF NOT EXISTS effects (id TEXT PRIMARY KEY, step TEXT NOT NULL, evidence TEXT NOT NULL);
  CREATE TABLE IF NOT EXISTS stock (id INTEGER PRIMARY KEY, available INTEGER NOT NULL);
  INSERT OR IGNORE INTO stock VALUES (1, 10);
`);
const asset = `eip155:31337/erc20:0x${'1'.repeat(40)}`;
const payer = `0x${'2'.repeat(40)}`;
const payee = `0x${'3'.repeat(40)}`;
const evidence = {
  reserve_inventory: { reservation_id: 'reservation:one' },
  pay: { transaction_id: 'local-provider:one', amount: '40', asset, payer, payee },
  create_order: { order_id: 'order:one' },
  confirm_inventory: { reservation_id: 'reservation:one' },
  release_inventory: { reservation_id: 'reservation:one' },
};
const adapters = Object.fromEntries(
  Object.entries(evidence).map(([step, value]) => [
    step,
    {
      async lookup({ idempotencyKey }) {
        const row = provider.prepare('SELECT evidence FROM effects WHERE id=?').get(idempotencyKey);
        return row
          ? { status: 'succeeded', evidence: JSON.parse(row.evidence) }
          : { status: 'not_found' };
      },
      async execute({ idempotencyKey }) {
        provider
          .transaction(() => {
            const result = provider
              .prepare('INSERT OR IGNORE INTO effects VALUES (?,?,?)')
              .run(idempotencyKey, step, JSON.stringify(value));
            if (result.changes && step === 'reserve_inventory')
              provider.prepare('UPDATE stock SET available=available-2 WHERE id=1').run();
          })
          .immediate();
        if (step === 'pay' && mode === 'crash') {
          process.send({ event: 'payment_committed' });
          await new Promise(() => {}); // Parent kills us before evidence is checkpointed.
        }
        return { status: 'succeeded', evidence: value };
      },
    },
  ]),
);
const store = new SqlitePurchaseStore(db);
const commerce = createAgentCommerce({
  store,
  identity: { agentId: 'buyer', principalId: 'company', tenantId: 'tenant', storeId: 'store' },
  policyVersion: 'v1',
  currencies: { USDC: { asset, payer, budgetId: 'monthly', decimals: 6 } },
  resolveQuote: async () => ({
    id: 'quote:one',
    counterpartyId: 'merchant',
    amount: '40',
    asset,
    payee,
    expiresAt: '2099-01-01T00:00:00Z',
  }),
  authorize: async () => ({ allowed: true, decisionId: 'test-policy' }),
  adapters,
  allowApply: true,
  // Advance past the dead worker's lease without making the test sleep.
  clock: () => 1_800_000_000_000 + (mode === 'crash' ? 0 : 60_000),
});
store.provisionBudget(commerce.scope, {
  id: 'monthly',
  asset,
  limit: '100',
  expiresAt: '2099-01-01T00:00:00Z',
});
const request = {
  quoteId: 'quote:one',
  idempotencyKey: 'purchase:one',
  maxTotal: { amount: '50', currency: 'USDC' },
};
if (mode === 'crash') await commerce.buy(request);
else {
  const before = store.budget(commerce.scope, 'monthly');
  const recovery = await commerce.recover();
  const replay = await commerce.buy(request);
  const counts = provider
    .prepare('SELECT step, COUNT(*) AS count FROM effects GROUP BY step')
    .all();
  const stock = provider.prepare('SELECT available FROM stock WHERE id=1').get();
  process.send({
    event: 'recovered',
    before,
    recovery,
    replay,
    counts,
    stock,
    budget: store.budget(commerce.scope, 'monthly'),
  });
}
db.close();
provider.close();
process.disconnect();
