/**
 * Offline installation smoke check. All writes are confined to a new in-memory
 * engine. Payment completion here records local state, never provider settlement.
 * Keep imports dependency-free so a broken native install can report its error.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const { version } = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8'));

export function createCheckReport() {
  return {
    schemaVersion: 1,
    package: '@stateset/embedded',
    version,
    runtime: { node: process.versions.node, platform: process.platform, arch: process.arch },
    scope: 'local-engine-only',
    database: ':memory:',
    externalSettlementVerified: false,
    ok: false,
    checks: [],
  };
}

function failure(id, error) {
  return {
    id,
    status: 'failed',
    code: typeof error?.code === 'string' ? error.code : 'EMBEDDED_CHECK_FAILED',
    message: error instanceof Error ? error.message : String(error),
    hint:
      id === 'native_binding'
        ? `Run npm install --include=optional @stateset/embedded@${version} in your project. ` +
          'Check npm install warnings and that optional dependencies are enabled. ' +
          'For supported platforms and libc requirements, see ' +
          'https://github.com/stateset/stateset-icommerce/blob/master/bindings/node/README.md#installation'
        : 'This is a local engine failure. Include this JSON report when opening an issue at ' +
          'https://github.com/stateset/stateset-icommerce/issues',
  };
}

/** The loader seam lets failure-path tests run even without a native binary. */
export async function runEmbeddedCheck({
  loadCommerce = async () => (await import('./index.js')).Commerce,
} = {}) {
  const report = createCheckReport();
  let commerce;
  let stage = 'native_binding';
  const check = async (id, operation) => {
    stage = id;
    const result = await operation();
    report.checks.push({ id, status: 'passed' });
    return result;
  };

  try {
    const Commerce = await check('native_binding', async () => {
      const loaded = await loadCommerce();
      assert.equal(typeof loaded?.open, 'function', 'Commerce.open is unavailable');
      return loaded;
    });
    commerce = await check('database', () => Commerce.open(':memory:'));
    const order = await check('order', async () => {
      const customer = await commerce.customers.create({
        email: 'embedded-check@example.invalid',
        firstName: 'Offline',
        lastName: 'Check',
      });
      const created = await commerce.orders.createExact({
        customerId: customer.id,
        currency: 'USD',
        items: [{ sku: 'EMBEDDED-CHECK', name: 'Local test item', quantity: 3, unitPrice: '0.10' }],
      });
      assert.equal(created.totalAmountExact, '0.30', 'Three 0.10 items must total exactly 0.30');
      return created;
    });
    const paymentInput = {
      orderId: order.id,
      amount: '0.30',
      currency: 'USD',
      paymentMethod: 'other',
      idempotencyKey: 'embedded-check-payment',
    };
    const payment = await check('payment', async () => {
      const created = await commerce.payments.createExact(paymentInput);
      assert.equal(created.amountExact, '0.30', 'Payment amount must round-trip exactly');
      assert.equal(created.currency, 'USD');
      assert.equal(created.status, 'pending');
      return created;
    });
    await check('payment_idempotency', async () => {
      const replay = await commerce.payments.createExact(paymentInput);
      assert.equal(replay.id, payment.id, 'Retry must return the same payment');
      await assert.rejects(
        () => commerce.payments.createExact({ ...paymentInput, amount: '0.31' }),
        (error) => error.code === 'CONFLICT',
        'Reusing a payment key for a different amount must be refused',
      );
      assert.equal(await commerce.payments.count(), 1, 'Retry must not create a second payment');
    });
    await check('payment_completion', async () => {
      assert.equal((await commerce.payments.markCompleted(payment.id)).status, 'completed');
      assert.equal((await commerce.orders.get(order.id)).paymentStatus, 'paid');
    });
    const refundInput = {
      paymentId: payment.id,
      amount: '0.30',
      reason: 'Offline installation check',
      idempotencyKey: 'embedded-check-refund',
    };
    const refund = await check('refund', async () => {
      const created = await commerce.payments.createRefundExact(refundInput);
      assert.equal(created.amountExact, '0.30', 'Refund amount must round-trip exactly');
      assert.equal(created.currency, 'USD');
      assert.equal(created.status, 'pending');
      assert.equal((await commerce.payments.get(payment.id)).amountRefundedExact, '0');
      return created;
    });
    await check('refund_idempotency', async () => {
      assert.equal((await commerce.payments.createRefundExact(refundInput)).id, refund.id);
      assert.equal((await commerce.payments.getRefunds(payment.id)).length, 1);
    });
    await check('refund_completion', async () => {
      assert.equal((await commerce.payments.completeRefund(refund.id)).status, 'completed');
      // A retry after completion must not account for the refund a second time.
      assert.equal((await commerce.payments.completeRefund(refund.id)).status, 'completed');
      const refunded = await commerce.payments.get(payment.id);
      assert.equal(refunded.amountRefundedExact, '0.30');
      assert.equal(refunded.status, 'refunded');
      assert.equal((await commerce.orders.get(order.id)).paymentStatus, 'refunded');
    });
  } catch (error) {
    report.checks.push(failure(stage, error));
  } finally {
    if (commerce) {
      try {
        await check('cleanup', async () => {
          await commerce.close();
          assert.equal(commerce.isClosed, true, 'The temporary engine must be closed');
        });
      } catch (error) {
        report.checks.push(failure('cleanup', error));
      }
    }
  }

  report.ok = report.checks.every((result) => result.status === 'passed');
  return report;
}
