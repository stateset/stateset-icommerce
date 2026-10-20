import Database from 'better-sqlite3';
import { Commerce } from '../../../bindings/node/index.js';
import { SqliteProtocolStore } from '../../../icp-handler/src/sqlite-store.mjs';
import { sqliteGet } from '../helpers/sqlite-process.js';
import { readFileSync } from 'node:fs';
import { createStripePaymentLookup } from '../../../icp-handler/src/stripe-payment.mjs';

// Operator configuration for the HTTP integration tests. Faults live in host
// callbacks, never in the handler's request surface.
export default function configure() {
  if (process.env.NATIVE_NOW) Date.now = () => Number(process.env.NATIVE_NOW);
  const commerce = new Commerce(process.env.NATIVE_COMMERCE_DB);
  const journal = new Database(process.env.NATIVE_PROTOCOL_DB);
  journal.pragma('journal_mode = WAL');
  journal.pragma('synchronous = FULL');
  journal.pragma('busy_timeout = 5000');
  const store = new SqliteProtocolStore(journal);
  if (process.env.NATIVE_FAIL_FULFILLMENT_PROJECTION === '1') {
    journal.exec(`CREATE TRIGGER IF NOT EXISTS fail_fulfillment_projection
      BEFORE UPDATE ON _icp_records WHEN NEW.namespace='escrows'
      AND json_extract(NEW.value, '$.native_acceptance.fulfillment.status')='recorded'
      BEGIN SELECT RAISE(ABORT, 'injected fulfillment projection failure'); END`);
  } else {
    journal.exec('DROP TRIGGER IF EXISTS fail_fulfillment_projection');
  }
  if (process.env.NATIVE_FAIL_PROJECTION === '1') {
    journal.exec(`CREATE TRIGGER IF NOT EXISTS fail_native_projection
      BEFORE INSERT ON _icp_records WHEN NEW.namespace='events'
      BEGIN SELECT RAISE(ABORT, 'injected projection failure'); END`);
  } else {
    journal.exec('DROP TRIGGER IF EXISTS fail_native_projection');
  }
  return {
    store,
    native: {
      commerce: {
        kernelFeatures: async () => {
          const features = await commerce.kernelFeatures();
          return features.filter(
            (feature) =>
              !(
                process.env.NATIVE_LEGACY_SHIP_FEATURE === '1' &&
                feature === 'orders.ship_reserved.v1'
              ) &&
              !(
                process.env.NATIVE_LEGACY_PARTIAL_FEATURE === '1' &&
                feature === 'orders.ship_reserved.partial.v1'
              ),
          );
        },
        checkoutSnapshot: (id) => commerce.checkoutSnapshot(id),
        orders: { get: (id) => commerce.orders.get(id) },
        async executeKernelCommand(...args) {
          if (
            args[0].command_type === 'orders.ship_reserved' &&
            process.env.NATIVE_SHIP_VERSION_RACE === '1'
          ) {
            // A competing native payment-state change increments order.version.
            sqliteGet(
              process.env.NATIVE_COMMERCE_DB,
              "UPDATE orders SET version=version+1, payment_status='refunded' WHERE id=? RETURNING id",
              args[0].payload.order_id,
            );
          }
          const result = await commerce.executeKernelCommand(...args);
          if (process.env.NATIVE_KILL_AFTER_COMMIT === '1' && result.status === 'succeeded')
            process.kill(process.pid, 'SIGKILL');
          if (
            process.env.NATIVE_KILL_PAYMENT_STEP === args[0].command_type &&
            result.status === 'succeeded'
          )
            process.kill(process.pid, 'SIGKILL');
          return result;
        },
      },
      principal: {
        id: 'agent:merchant',
        kind: 'agent',
        tenant_id: 'tenant',
        delegated_by: 'company',
        capabilities: [
          'checkout.commit',
          'payments.create',
          'payments.complete',
          'orders.ship_reserved',
        ],
      },
      policy: {
        version: 'native-http-v1',
        commands: {
          'checkout.commit': { required_capabilities: ['checkout.commit'], requires_budget: true },
          'payments.create': { required_capabilities: ['payments.create'] },
          'orders.ship_reserved': {
            required_capabilities: [
              process.env.NATIVE_DENY_SHIP === '1' ? 'orders.denied' : 'orders.ship_reserved',
            ],
          },
          'payments.complete': {
            required_capabilities: [
              process.env.NATIVE_DENY_PAYMENT === '1' ? 'payments.denied' : 'payments.complete',
            ],
          },
        },
        trusted_authority_keys: {},
      },
      storeId: 'store',
      ...(process.env.NATIVE_FULFILLMENT === '1'
        ? {
            fulfillment: {
              resolveDispatch: () => {
                if (process.env.NATIVE_DISPATCH_OUTAGE === '1')
                  throw new Error('dispatch unavailable');
                return JSON.parse(readFileSync(process.env.NATIVE_DISPATCH_DATA, 'utf8'));
              },
            },
          }
        : {}),
      ...(process.env.NATIVE_STRIPE === '1'
        ? {
            payments: {
              resolvePayment: () => 'pi_native',
              provider: createStripePaymentLookup({
                apiKey: 'sk_test_fixture',
                accountId: 'acct_native',
                apiVersion: '2024-06-20',
                fetchImpl: async (url) => {
                  if (process.env.NATIVE_STRIPE_OUTAGE === '1')
                    throw new Error('provider unavailable');
                  const payment = JSON.parse(readFileSync(process.env.NATIVE_STRIPE_DATA, 'utf8'));
                  return {
                    ok: true,
                    json: async () =>
                      url.endsWith('/account') ? { id: 'acct_native', object: 'account' } : payment,
                  };
                },
              }),
            },
          }
        : {}),
      resolveCart: (intent) => {
        const binding = JSON.parse(process.env.NATIVE_CART_BINDINGS)[intent.buyer];
        return binding ? { cartId: binding, budgetId: 'http-budget' } : null;
      },
      readReceipt: (key) => {
        if (
          process.env.NATIVE_SHIP_LOOKUP_UNAVAILABLE === '1' &&
          key.startsWith('native-fulfillment:')
        )
          throw new Error('shipment receipt ledger unavailable');
        if (process.env.NATIVE_LOOKUP_UNAVAILABLE === '1')
          throw new Error('receipt ledger unavailable');
        const row = sqliteGet(
          process.env.NATIVE_COMMERCE_DB,
          'SELECT receipt FROM kernel_receipts WHERE idempotency_key=?',
          key,
        );
        if (
          !row &&
          process.env.NATIVE_NEW_SHIP_LOOKUP_UNAVAILABLE === '1' &&
          key.startsWith('native-fulfillment:')
        )
          throw new Error('new shipment receipt lookup unavailable');
        return row ? JSON.parse(row.receipt) : null;
      },
    },
    close() {
      journal.close();
    },
  };
}
