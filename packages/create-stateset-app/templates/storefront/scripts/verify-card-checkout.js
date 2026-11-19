// Exercise native stock, order and refund persistence against a processor fixture.
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Commerce } from '@stateset/embedded';
import {
  startStripeCheckout,
  reconcileStripeCheckout,
  refundStripePayment,
  reconcileStripeRefund,
} from '../lib/stripe-checkout.js';
import { getTaxProvider } from '../lib/tax.js';
import { getShippingProvider } from '../lib/shipping.js';

const directory = mkdtempSync(join(tmpdir(), 'stateset-card-checkout-'));
const filename = join(directory, 'store.db');
let commerce = new Commerce(filename);
try {
  const sku = 'COFFEE';
  const product = await commerce.products.create({
    name: 'Coffee',
    variants: [{ sku, priceExact: '12.99', isDefault: true }],
  });
  await commerce.products.activate(product.id);
  await commerce.inventory.createItem({ sku, name: 'Coffee', initialQuantity: 4 });
  const cart = await commerce.carts.create({ currency: 'USD' });
  await commerce.carts.addItemExact(cart.id, {
    productId: product.id,
    sku,
    name: 'Coffee',
    quantity: 2,
    unitPrice: '12.99',
  });
  let session;
  let remoteRefund;
  const stripe = {
    checkout: {
      sessions: {
        create: async (parameters) => {
          assert.equal(parameters.line_items[0].price_data.unit_amount, 3286);
          session = {
            ...parameters,
            id: 'cs_test_demo',
            status: 'open',
            currency: 'usd',
            amount_total: 3286,
            url: 'https://checkout.stripe.com/example',
          };
          return session;
        },
        retrieve: async () => session,
        list: async () => ({ data: [session] }),
      },
    },
    refunds: {
      list: async function* () {
        if (remoteRefund) yield remoteRefund;
      },
      create: async (input) => {
        assert.equal(remoteRefund, undefined);
        remoteRefund = { ...input, currency: 'usd', id: 're_demo', status: 'succeeded' };
        return remoteRefund;
      },
      retrieve: async () => remoteRefund,
    },
  };
  const input = {
    cartId: cart.id,
    email: 'buyer@example.test',
    shippingMethodId: 'ground',
    shippingAddress: {
      firstName: 'Ada',
      lastName: 'L',
      line1: '1 Main St',
      city: 'Oakland',
      state: 'CA',
      postalCode: '94601',
      country: 'US',
    },
  };
  const options = {
    commerce,
    stripe,
    origin: 'https://store.example.test',
    input,
    shippingProvider: getShippingProvider({
      STATESET_SHIPPING_METHODS_JSON:
        '[{"id":"ground","label":"Ground","amount":"5","countries":["US"]}]',
    }),
    taxProvider: getTaxProvider({ STATESET_TAX_RATES_JSON: '{"CA":"0.0725"}' }),
  };
  assert.deepEqual(await startStripeCheckout(options), await startStripeCheckout(options));
  await assert.rejects(
    startStripeCheckout({ ...options, input: { ...input, email: 'other@example.test' } }),
    /different/,
  );
  assert.equal((await reconcileStripeCheckout(commerce, stripe, session.id)).state, 'pending');
  session.status = 'complete';
  session.payment_status = 'paid';
  session.payment_intent = {
    id: 'pi_demo',
    currency: 'usd',
    status: 'succeeded',
    amount_received: 3285,
    metadata: session.metadata,
  };
  await assert.rejects(
    reconcileStripeCheckout(commerce, stripe, session.id),
    /settlement does not match/,
  );
  assert.equal((await commerce.payments.list()).length, 0);
  session.payment_intent.amount_received++;
  const result = await reconcileStripeCheckout(commerce, stripe, session.id);
  assert.equal(result.state, 'paid');
  const refundInput = {
    paymentId: result.paymentId,
    amount: '1.25',
    reason: 'Buyer request',
    idempotencyKey: 'fixture-refund',
  };
  assert.equal((await refundStripePayment(commerce, stripe, refundInput)).status, 'completed');
  await commerce.close();
  commerce = new Commerce(filename);
  await reconcileStripeCheckout(commerce, stripe, session.id);
  await reconcileStripeRefund(commerce, stripe, remoteRefund.id);
  await refundStripePayment(commerce, stripe, refundInput);
  const payment = await commerce.payments.get(result.paymentId);
  assert.equal(payment.amountRefundedExact, '1.25');
  assert.equal((await commerce.payments.getRefunds(payment.id)).length, 1);
  assert.equal((await commerce.orders.list()).length, 1);
  assert.equal((await commerce.payments.list()).length, 1);
  assert.equal((await commerce.inventory.getStock(sku)).totalAvailable, '2');
  console.log(
    JSON.stringify({
      result: 'passed',
      provider: 'fixture',
      externalProvidersVerified: false,
      checks: [
        'stock reservation',
        'immutable checkout',
        'exact provider settlement',
        'replay after reopen',
        'partial refund reconciliation',
      ],
    }),
  );
} finally {
  await commerce.close();
  rmSync(directory, { recursive: true, force: true });
}
