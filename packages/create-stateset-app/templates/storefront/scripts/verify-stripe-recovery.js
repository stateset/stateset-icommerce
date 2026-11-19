import { stat } from 'node:fs/promises';
import { parseArgs } from 'node:util';
import { loadStoreEnvironment } from '../lib/store-environment.js';
import { verifyStripeRecovery } from '../lib/stripe-recovery-proof.js';

const { values } = parseArgs({
  options: {
    check: { type: 'boolean' },
    event: { type: 'string' },
    db: { type: 'string' },
  },
});
let commerce;
try {
  if (!values.check) {
    console.log(
      JSON.stringify({
        mode: 'preview',
        providerCalls: false,
        check:
          'Pass --check --event evt_... --db ./store.db with a Stripe sandbox key to verify a failed membership renewal and its recovery.',
      }),
    );
  } else {
    if (!/^evt_[a-zA-Z0-9]+$/.test(values.event || '')) throw new Error('Supply --event evt_...');
    if (!values.db || !(await stat(values.db).catch(() => null))?.isFile())
      throw new Error('Supply --db with the existing sandbox storefront database');
    await loadStoreEnvironment();
    const key = process.env.STRIPE_TEST_KEY || process.env.STRIPE_SECRET_KEY;
    if (!/^sk_test_[a-zA-Z0-9]+$/.test(key || ''))
      throw new Error('Configure a Stripe sandbox secret key');
    const [{ default: Stripe }, { Commerce }] = await Promise.all([
      import('stripe'),
      import('@stateset/embedded'),
    ]);
    commerce = new Commerce(values.db);
    const stripe = new Stripe(key, { maxNetworkRetries: 2, timeout: 12000 });
    const report = await verifyStripeRecovery(commerce, stripe, values.event);
    console.log(JSON.stringify(report));
    if (!report.providerConfirmed) process.exitCode = report.state === 'closed_unpaid' ? 1 : 2;
  }
} catch (error) {
  const message = error instanceof Error ? error.message : 'Stripe recovery verification failed';
  console.error(message.replace(/(?:sk|rk)_(?:test|live)_[A-Za-z0-9*]+/g, '[redacted]'));
  process.exitCode = 1;
} finally {
  if (commerce) await commerce.close();
}
