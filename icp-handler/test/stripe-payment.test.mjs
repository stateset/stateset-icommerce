import test from 'node:test';
import assert from 'node:assert/strict';
import { createStripePaymentLookup } from '../src/stripe-payment.mjs';

const terms = {
  amount: '25.00',
  currency: 'USD',
  cartId: 'cart-1',
  cartFingerprint: `sha256:${'a'.repeat(64)}`,
  buyer: 'buyer-1',
};
function intent() {
  return {
    object: 'payment_intent',
    id: 'pi_test',
    amount: 2500,
    amount_received: 2500,
    amount_capturable: 0,
    currency: 'usd',
    livemode: false,
    status: 'succeeded',
    transfer_data: null,
    on_behalf_of: null,
    client_secret: 'never-persist-this',
    metadata: {
      stateset_cart_id: terms.cartId,
      stateset_cart_fingerprint: terms.cartFingerprint,
      stateset_buyer: terms.buyer,
    },
    latest_charge: {
      id: 'ch_test',
      object: 'charge',
      payment_intent: 'pi_test',
      currency: 'usd',
      amount: 2500,
      amount_captured: 2500,
      amount_refunded: 0,
      livemode: false,
      paid: true,
      captured: true,
      refunded: false,
      disputed: false,
      status: 'succeeded',
      payment_method_details: { type: 'card' },
      transfer_data: null,
      on_behalf_of: null,
    },
  };
}
function fixture({
  payment = intent(),
  account = 'acct_test',
  ok = true,
  connectedAccount = false,
} = {}) {
  const requests = [];
  const provider = createStripePaymentLookup({
    apiKey: 'secret-fixture-key',
    accountId: 'acct_test',
    apiVersion: '2024-06-20',
    connectedAccount,
    fetchImpl: async (url, options) => {
      requests.push({ url, options });
      return {
        ok,
        json: async () => (url.endsWith('/account') ? { id: account, object: 'account' } : payment),
      };
    },
  });
  return { requests, provider };
}

test('Stripe lookup verifies direct USD card capture and returns only allowed evidence', async () => {
  const { provider, requests } = fixture({ connectedAccount: true });
  const result = await provider.lookup('pi_test', terms);
  assert.equal(result.status, 'succeeded');
  assert.equal(result.evidence.provider, provider.identity);
  assert.equal(result.evidence.transactionId, 'ch_test');
  assert.equal(result.evidence.amount, '25.00');
  assert.doesNotMatch(JSON.stringify(result), /secret|payment_method_details|2500/);
  assert.equal(requests.length, 2);
  for (const { url, options } of requests) {
    assert.match(url, /^https:\/\/api\.stripe\.com\/v1\//);
    assert.equal(options.method, 'GET');
    assert.equal(options.redirect, 'error');
    assert.equal(options.headers['Stripe-Account'], 'acct_test');
    assert.equal(options.headers['Stripe-Version'], '2024-06-20');
  }
});

for (const [label, mutate] of [
  [
    'amount',
    (p) => {
      p.amount = 2499;
    },
  ],
  [
    'received amount',
    (p) => {
      p.amount_received = 2499;
    },
  ],
  [
    'currency',
    (p) => {
      p.currency = 'eur';
    },
  ],
  [
    'live mode',
    (p) => {
      p.livemode = true;
    },
  ],
  [
    'cart',
    (p) => {
      p.metadata.stateset_cart_id = 'other';
    },
  ],
  [
    'fingerprint',
    (p) => {
      p.metadata.stateset_cart_fingerprint = 'other';
    },
  ],
  [
    'buyer',
    (p) => {
      p.metadata.stateset_buyer = 'other';
    },
  ],
  [
    'recipient transfer',
    (p) => {
      p.transfer_data = { destination: 'acct_other' };
    },
  ],
  [
    'on behalf of',
    (p) => {
      p.on_behalf_of = 'acct_other';
    },
  ],
  [
    'unexpanded charge',
    (p) => {
      p.latest_charge = 'ch_test';
    },
  ],
  [
    'wrong charge',
    (p) => {
      p.latest_charge.payment_intent = 'pi_other';
    },
  ],
  [
    'partial capture',
    (p) => {
      p.latest_charge.amount_captured = 2000;
    },
  ],
  [
    'refund',
    (p) => {
      p.latest_charge.amount_refunded = 1;
    },
  ],
  [
    'dispute',
    (p) => {
      p.latest_charge.disputed = true;
    },
  ],
  [
    'authorization only',
    (p) => {
      p.latest_charge.captured = false;
    },
  ],
  [
    'different payment method',
    (p) => {
      p.latest_charge.payment_method_details.type = 'us_bank_account';
    },
  ],
]) {
  test(`Stripe lookup refuses ${label}`, async () => {
    const payment = intent();
    mutate(payment);
    await assert.rejects(fixture({ payment }).provider.lookup('pi_test', terms));
  });
}

test('unknown provider outcomes never become successful payments', async () => {
  for (const status of [
    'processing',
    'requires_action',
    'requires_capture',
    'canceled',
    'unexpected',
  ]) {
    const payment = intent();
    payment.status = status;
    assert.notEqual(
      (await fixture({ payment }).provider.lookup('pi_test', terms)).status,
      'succeeded',
    );
  }
  await assert.rejects(
    fixture({ account: 'acct_other' }).provider.lookup('pi_test', terms),
    /account mismatch/,
  );
  await assert.rejects(
    fixture({ ok: false }).provider.lookup('pi_test', terms),
    /lookup unavailable/,
  );
});

test('invalid references and fractional cents are refused before HTTP', async () => {
  const f = fixture();
  for (const value of ['25.001', '0.00', '1000000', '1e2', 25])
    await assert.rejects(f.provider.lookup('pi_test', { ...terms, amount: value }));
  await assert.rejects(f.provider.lookup('../account', terms));
  await assert.rejects(f.provider.lookup('pi_test', { ...terms, currency: 'USDC' }));
  await assert.rejects(f.provider.lookup('pi_test', { ...terms, buyer: undefined }));
  await assert.rejects(f.provider.lookup('pi_test', { ...terms, cartFingerprint: 'unbound' }));
  assert.equal(f.requests.length, 0);
});
