import { verifyStripeConnection } from '../lib/merchant-onboarding.js';
import { parseArgs } from 'node:util';
import { Commerce } from '@stateset/embedded';
import Stripe from 'stripe';
import { loadStoreEnvironment } from '../lib/store-environment.js';
import { getStripeConfiguration } from '../lib/stripe-config.js';
import { inspectStoreDatabase } from '../lib/store-database-check.js';
import { refundStripePayment, recoverStripeOrder } from '../lib/stripe-checkout.js';
import {
  refundRoutedCheckout,
  advanceRoutedCheckout,
  routedAccounts,
} from '../lib/routed-checkout.js';
import { merchantRecords } from '../lib/merchant-records.js';
import { decimalToUnits } from '../lib/money.js';

const { positionals, values } = parseArgs({
  allowPositionals: true,
  options: {
    apply: { type: 'boolean' },
    payment: { type: 'string' },
    order: { type: 'string' },
    amount: { type: 'string' },
    key: { type: 'string' },
    reason: { type: 'string' },
    account: { type: 'string' },
    reference: { type: 'string' },
  },
});
let commerce;
try {
  await loadStoreEnvironment();
  const client = () =>
    new Stripe(getStripeConfiguration(process.env).key, { maxNetworkRetries: 2, timeout: 12000 });
  const [command] = positionals;
  if (command === 'verify') {
    const { key, origin } = getStripeConfiguration(process.env);
    const stripe = client();
    const report = await verifyStripeConnection(
      stripe,
      origin,
      process.env.STATESET_RENEWAL_RECOVERY === 'true',
    );
    console.log(
      JSON.stringify({ ...report, mode: key.startsWith('sk_test_') ? 'test' : 'live' }, null, 2),
    );
    if (!report.checkoutWebhookRegistered || !report.chargesEnabled) process.exitCode = 1;
  } else if (['refund', 'recover'].includes(command)) {
    if (inspectStoreDatabase(process.env, { launch: true }).status !== 'pass')
      throw new Error('An existing store database is required.');
    commerce = new Commerce(process.env.STATESET_DB_PATH || './store.db');
    if (command === 'recover') {
      if (!values.order) throw new Error('Supply --order ORDER_ID');
      const order = await commerce.orders.get(values.order);
      if (order?.notes?.startsWith('Routed checkout:')) {
        let session;
        for await (const row of merchantRecords(commerce, 'routed-checkouts'))
          if (row.orderId === order.id) {
            session = row;
            break;
          }
        if (!session) throw new Error('Routed checkout session has not been bound');
        if (values.account || values.reference) {
          if (!values.account || !values.reference)
            throw new Error('Provide both --account and --reference for processor reconciliation');
          if (values.apply)
            await commerce.payments.routeCard({
              action: 'reconcile',
              paymentId: session.paymentId,
              accounts: routedAccounts(process.env, session),
              account: values.account,
              reference: values.reference,
            });
        }
        const result = values.apply
          ? await advanceRoutedCheckout(commerce, process.env, session)
          : { state: 'preview', status: order.status };
        console.log(JSON.stringify({ state: result.state, order: order.id }));
      } else
        console.log(
          JSON.stringify(await recoverStripeOrder(commerce, client(), values.order, values.apply)),
        );
    } else {
      if (
        !values.payment ||
        !values.amount ||
        !values.key ||
        !values.reason ||
        decimalToUnits(values.amount, 2) <= 0n
      ) {
        throw new Error(
          'Supply --payment ID --amount DECIMAL --key UNIQUE_REQUEST_KEY --reason TEXT',
        );
      }
      const input = {
        paymentId: values.payment,
        amount: values.amount,
        reason: values.reason,
        idempotencyKey: values.key,
      };
      const payment = await commerce.payments.get(input.paymentId);
      const routed = payment?.idempotencyKey?.startsWith('routed:');
      if (!routed && !payment?.idempotencyKey?.startsWith('stripe:pi_'))
        throw new Error('Unknown card checkout payment');
      const result = values.apply
        ? routed
          ? await refundRoutedCheckout(commerce, process.env, { ...input, requestKey: values.key })
          : await refundStripePayment(commerce, client(), input)
        : {
            mode: 'preview',
            ...input,
            currency: payment.currency,
            captured: payment.amountExact,
            refunded: payment.amountRefundedExact,
          };
      console.log(JSON.stringify(result));
    }
  } else throw new Error('Use verify, refund or recover.');
} catch (error) {
  console.error(error instanceof Error ? error.message : 'Card operation failed');
  process.exitCode = 1;
} finally {
  if (commerce) await commerce.close();
}
