import { parseArgs } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';
import { Commerce } from '@stateset/embedded';
import { verifyCardAccount } from '../lib/card-accounts.js';
import { loadStoreEnvironment } from '../lib/store-environment.js';
import { saveMerchantRecord } from '../lib/merchant-records.js';
import { decimalToUnits } from '../lib/money.js';

const { values } = parseArgs({
  options: {
    apply: { type: 'boolean' },
    provider: { type: 'string', default: 'both' },
    key: { type: 'string' },
    db: { type: 'string' },
    'wait-seconds': { type: 'string', default: '30' },
  },
});
const providers = values.provider === 'both' ? ['stripe', 'square'] : [values.provider];
let commerce;
try {
  await loadStoreEnvironment();
  if (!providers.every((p) => ['stripe', 'square'].includes(p)))
    throw new Error('Choose stripe, square or both');
  const wait = Number(values['wait-seconds']);
  if (!Number.isSafeInteger(wait) || wait < 0 || wait > 300)
    throw new Error('Choose a wait time from 0 to 300 seconds');
  if (!values.apply) {
    console.log(
      JSON.stringify({
        mode: 'preview',
        providers,
        charge: '1.00',
        partialRefund: '0.25',
        currency: 'USD',
        providerCalls: false,
        apply:
          'Supply --apply --db PATH_TO_DRILL_STORE --key STABLE_DRILL_ID. Rerun the same command for pending settlements.',
      }),
    );
  } else {
    if (!/^[a-z0-9_-]{1,48}$/.test(values.key || '') || !values.db || values.db === ':memory:')
      throw new Error('Supply a durable drill database and stable drill key');
    const env = process.env;
    const stripeKey = env.STRIPE_TEST_KEY || env.STRIPE_SECRET_KEY;
    if (providers.includes('stripe') && !/^sk_test_[A-Za-z0-9]+$/.test(stripeKey || ''))
      throw new Error('Configure a Stripe test-mode secret key');
    if (
      providers.includes('square') &&
      (!env.SQUARE_SANDBOX_ACCESS_TOKEN ||
        !/^[A-Za-z0-9_-]+$/.test(env.SQUARE_SANDBOX_LOCATION || ''))
    )
      throw new Error('Configure SQUARE_SANDBOX_ACCESS_TOKEN and SQUARE_SANDBOX_LOCATION');
    commerce = new Commerce(values.db);
    for (const provider of providers) {
      const common = {
        account: provider,
        provider,
        available: true,
        currencies: ['USD'],
        methods: ['credit_card'],
      };
      const account =
        provider === 'stripe'
          ? { ...common, secret: stripeKey, token: 'pm_card_visa' }
          : {
              ...common,
              secret: env.SQUARE_SANDBOX_ACCESS_TOKEN,
              sandbox: true,
              location: env.SQUARE_SANDBOX_LOCATION,
              token: 'cnon:card-nonce-ok',
            };
      const { identity, mode, currency } = await verifyCardAccount(account);
      if (mode !== 'sandbox' || (provider === 'square' && currency !== 'USD'))
        throw new Error('Use a test account with a USD location for this drill');
      const key = 'provider-drill-' + provider + '-' + values.key;
      await saveMerchantRecord(commerce, key, { provider, identity, currency: 'USD', amount: '1' });
      const payment = await commerce.payments.createExact({
        amount: '1',
        currency: 'USD',
        paymentMethod: 'credit_card',
        idempotencyKey: key,
      });
      const route = (action, extra = {}) =>
        commerce.payments.routeCard({
          paymentId: payment.id,
          accounts: [account],
          action,
          ...extra,
        });
      const deadline = Date.now() + wait * 1000;
      let result;
      if (!['completed', 'partially_refunded'].includes(payment.status)) {
        do {
          result = await route('authorize');
          if (result.state === 'authorized') result = await route('capture');
          if (result.state !== 'pending' || Date.now() >= deadline) break;
          await delay(Math.min(2000, deadline - Date.now()));
        } while (true);
        if (result.state !== 'captured') {
          console.log(
            JSON.stringify({
              provider,
              paymentId: payment.id,
              state: result.state,
              providerConfirmed: false,
            }),
          );
          process.exitCode = 2;
          continue;
        }
      }
      const binding = await route('selected');
      if (binding.state !== 'selected' || binding.account !== provider || !binding.reference)
        throw new Error('Captured payment is missing its processor reference');
      do {
        result = await route('refund', {
          amount: '0.25',
          requestKey: key + '-refund',
          reason: 'Sandbox verification',
        });
        if (!['pending', 'processing'].includes(result.state) || Date.now() >= deadline) break;
        await delay(Math.min(2000, deadline - Date.now()));
      } while (true);
      const stored = await commerce.payments.get(payment.id);
      const confirmed =
        result.state === 'completed' &&
        stored.status === 'partially_refunded' &&
        decimalToUnits(stored.amountRefundedExact, 2) === 25n;
      console.log(
        JSON.stringify({
          provider,
          mode: 'sandbox',
          paymentId: payment.id,
          providerReference: binding.reference,
          refundId: result.refundId,
          captured: stored.amountExact,
          refunded: stored.amountRefundedExact,
          state: result.state,
          providerConfirmed: confirmed,
        }),
      );
      if (!confirmed) process.exitCode = 2;
    }
  }
} catch (error) {
  let message = error instanceof Error ? error.message : 'Provider drill failed';
  for (const key of ['STRIPE_TEST_KEY', 'STRIPE_SECRET_KEY', 'SQUARE_SANDBOX_ACCESS_TOKEN']) {
    if (process.env[key]) message = message.replaceAll(process.env[key], '[redacted]');
  }
  console.error(message.replace(/(?:sk|rk)_(?:test|live)_[A-Za-z0-9*]+/g, '[redacted]'));
  process.exitCode = 1;
} finally {
  if (commerce) await commerce.close();
}
