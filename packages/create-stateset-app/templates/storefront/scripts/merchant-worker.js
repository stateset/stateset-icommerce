import { parseArgs } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';
import Stripe from 'stripe';
import { Commerce } from '@stateset/embedded';
import { loadStoreEnvironment } from '../lib/store-environment.js';
import { getStripeConfiguration, getStripePlatformConfiguration } from '../lib/stripe-config.js';
import { inspectStoreDatabase } from '../lib/store-database-check.js';
import { processIntegrations, sendKlaviyoEvent } from '../lib/merchant-integrations.js';
import {
  RenewalRecoveryError,
  runRenewalRecovery,
  sendRecoveryEmail,
} from '../lib/billing-recovery.js';
import { recoverStripeOrder } from '../lib/stripe-checkout.js';
import { settleSellers, sellerBankPayouts, affiliateBankPayouts } from '../lib/seller-payouts.js';
import { merchantRecords, readMerchantRecord } from '../lib/merchant-records.js';
import { advanceRoutedCheckout, recoverRoutedRefunds } from '../lib/routed-checkout.js';

const { values } = parseArgs({
  options: {
    apply: { type: 'boolean' },
    watch: { type: 'boolean' },
    interval: { type: 'string', default: '60' },
  },
});
const interval = Number(values.interval);
if (!Number.isSafeInteger(interval) || interval < 1 || interval > 86400)
  throw new Error('Interval must be 1 to 86400 seconds');
await loadStoreEnvironment();
const env = process.env;
if (inspectStoreDatabase(env, { launch: true }).status !== 'pass')
  throw new Error('Configure an existing store');
const commerce = new Commerce(env.STATESET_DB_PATH || './store.db');
const stop = new AbortController();
process.on('SIGTERM', () => stop.abort());
process.on('SIGINT', () => stop.abort());
const stripe =
  env.STATESET_CHECKOUT_PROVIDER === 'stripe' ||
  (env.STATESET_CHECKOUT_PROVIDER === 'routed' &&
    env.STRIPE_SECRET_KEY &&
    env.STRIPE_WEBHOOK_SECRET)
    ? new Stripe(getStripeConfiguration(env).key, { maxNetworkRetries: 2, timeout: 12000 })
    : null;

const payoutClient =
  stripe ??
  (env.STATESET_SYNC_BANK_PAYOUTS === 'true' || env.STATESET_AUTOMATIC_SELLER_TRANSFERS === 'true'
    ? new Stripe(getStripePlatformConfiguration(env).key, { maxNetworkRetries: 2, timeout: 12000 })
    : null);

async function run() {
  const results = {};
  const failed = [];
  async function task(name, fn) {
    try {
      results[name] = await fn();
    } catch (error) {
      if (error instanceof RenewalRecoveryError) {
        results[name] = error.report;
        failed.push(...error.report.failures.map((row) => row.invoice));
      } else {
        failed.push(name);
        results[name] = { error: error.message };
      }
    }
  }
  if (stripe)
    await task('checkout', async () => {
      const orders = [];
      for (let offset = 0; ; offset += 100) {
        const page = await commerce.orders.list({ status: 'pending', limit: 100, offset });
        orders.push(...page.filter((order) => order.notes?.startsWith('Stripe checkout:')));
        if (page.length < 100) break;
      }
      const outcomes = [];
      for (const order of orders) {
        try {
          outcomes.push(await recoverStripeOrder(commerce, stripe, order.id, values.apply));
        } catch (error) {
          failed.push(order.id);
          outcomes.push({ orderId: order.id, error: error.message });
        }
      }
      return outcomes;
    });
  if (env.STATESET_CARD_ACCOUNTS_JSON || stripe)
    await task('routed', async () => {
      const outcomes = [];
      for await (const session of merchantRecords(commerce, 'routed-checkouts')) {
        if (await readMerchantRecord(commerce, 'routed-finished-' + session.orderId)) continue;
        try {
          const state = values.apply
            ? (await advanceRoutedCheckout(commerce, env, session)).state
            : 'preview';
          outcomes.push({ orderId: session.orderId, state });
        } catch (error) {
          failed.push(session.orderId);
          outcomes.push({ orderId: session.orderId, error: error.message });
        }
      }
      return outcomes;
    });
  if (env.STATESET_CARD_ACCOUNTS_JSON || stripe)
    await task('routedRefunds', async () => {
      const rows = await recoverRoutedRefunds(commerce, env, values.apply);
      failed.push(
        ...rows
          .filter((row) => row.state === 'review' || row.state === 'failed')
          .map((row) => row.refundId),
      );
      return rows;
    });
  if (stripe && env.STATESET_RENEWAL_RECOVERY === 'true')
    await task('renewals', () =>
      runRenewalRecovery({
        commerce,
        stripe,
        env,
        apply: values.apply,
        notify: (notice) =>
          env.RESEND_API_KEY
            ? sendRecoveryEmail(env, notice)
            : sendKlaviyoEvent(env, {
                key: notice.key,
                email: notice.email,
                metric: 'StateSet Renewal Recovery',
                properties: {
                  invoice_id: notice.invoiceId,
                  update_url: notice.updateUrl,
                  payment_url: notice.paymentUrl,
                },
              }),
      }),
    );
  await task('integrations', async () => {
    const rows = await processIntegrations(commerce, env, { apply: values.apply });
    const needsAttention = rows.filter((r) => ['failed', 'review'].includes(r.status));
    failed.push(...needsAttention.map((r) => r.key));
    return {
      total: rows.length,
      accepted: rows.filter((r) => r.status === 'accepted').length,
      failed: needsAttention,
    };
  });
  if (payoutClient && env.STATESET_AUTOMATIC_SELLER_TRANSFERS === 'true')
    await task('sellers', async () => {
      const rows = await settleSellers(commerce, payoutClient, env, values.apply);
      failed.push(...rows.filter((r) => r.state === 'failed').map((r) => r.orderId));
      return rows;
    });
  if (payoutClient && env.STATESET_SYNC_BANK_PAYOUTS === 'true')
    await task('bankPayouts', async () => {
      const synced = [];
      for await (const seller of merchantRecords(commerce, 'sellers')) {
        try {
          const rows = await sellerBankPayouts(commerce, payoutClient, seller.seller, values.apply);
          synced.push({ seller: seller.seller, payouts: rows.length });
        } catch (error) {
          failed.push(seller.seller);
          synced.push({ seller: seller.seller, error: error.message });
        }
      }
      for await (const partner of merchantRecords(commerce, 'affiliate-connect-accounts')) {
        try {
          const rows = await affiliateBankPayouts(
            commerce,
            payoutClient,
            partner.affiliate,
            values.apply,
          );
          synced.push({ affiliate: partner.affiliate, payouts: rows.length });
        } catch (error) {
          failed.push(partner.affiliate);
          synced.push({ affiliate: partner.affiliate, error: error.message });
        }
      }
      return synced;
    });
  console.log(
    JSON.stringify({
      at: new Date().toISOString(),
      mode: values.apply ? 'apply' : 'preview',
      results,
      failed,
    }),
  );
  if (!values.watch && failed.length) process.exitCode = 1;
}
try {
  do {
    await run();
    if (values.watch && !stop.signal.aborted)
      await delay(interval * 1000, undefined, { signal: stop.signal }).catch((error) => {
        if (error.name !== 'AbortError') throw error;
      });
  } while (values.watch && !stop.signal.aborted);
} finally {
  await commerce.close();
}
