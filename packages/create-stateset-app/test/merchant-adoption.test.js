import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import test from 'node:test';
import { adoptionReport, enrollStore } from '../templates/storefront/lib/merchant-adoption.js';
import { saveMerchantRecord } from '../templates/storefront/lib/merchant-records.js';
import { groupAdoptionReports } from '../templates/storefront/scripts/adoption-cohorts.js';

const day = 86400000,
  start = Date.UTC(2026, 0, 1);
const iso = (offset) => new Date(start + offset * day).toISOString();

function fixture() {
  const records = new Map(),
    orders = new Map(),
    charges = new Map(),
    sessions = new Map();
  const commerce = {
    customObjects: {
      getTypeByHandle: async () => ({}),
      getObjectByHandle: async (_, key) => records.get(key),
      createObject: async (row) => {
        if (records.has(row.handle)) throw new Error('duplicate');
        records.set(row.handle, row);
      },
      listObjects: async ({ ownerId, offset, limit }) =>
        [...records.values()]
          .filter((row) => row.ownerId === ownerId)
          .slice(offset, offset + limit),
    },
    orders: { get: async (id) => orders.get(id) },
    payments: { list: async ({ orderId }) => [charges.get(orderId)] },
  };
  async function sale(id, date, options = {}) {
    const order = {
      id,
      createdAt: iso(date),
      totalAmountExact: '12.50',
      currency: 'USD',
      customerId: 'private-customer',
    };
    orders.set(id, order);
    charges.set(id, {
      id: 'payment-' + id,
      orderId: id,
      customerId: order.customerId,
      status: 'completed',
      amountExact: '12.5',
      currency: 'USD',
      idempotencyKey: 'stripe:pi_' + id,
    });
    await saveMerchantRecord(
      commerce,
      'sale-' + id,
      {
        type: 'sale',
        orderId: id,
        placedAt: order.createdAt,
        at: iso(date + 0.25),
        currency: 'USD',
        total: '12.50',
        email: 'private@shop.example',
      },
      'merchant-events',
    );
    if (options.routed) return;
    const sessionId = 'cs_' + (options.test ? 'test_' : 'live_') + id;
    await saveMerchantRecord(commerce, 'stripe-session-' + id, { sessionId });
    const metadata = { order_id: id };
    sessions.set(sessionId, {
      id: sessionId,
      metadata,
      client_reference_id: id,
      status: 'complete',
      payment_status: 'paid',
      mode: 'payment',
      currency: 'usd',
      amount_total: 1250,
      livemode: true,
      payment_intent: {
        id: 'pi_' + id,
        metadata,
        amount_received: 1250,
        status: 'succeeded',
        currency: 'usd',
        livemode: true,
      },
    });
  }
  const check = {
    now: start + 31 * day,
    check: true,
    env: {},
    stripeFor: () => ({
      checkout: {
        sessions: {
          retrieve: async (id) => {
            if (!sessions.has(id)) throw new Error('missing');
            return sessions.get(id);
          },
        },
      },
    }),
  };
  return { commerce, orders, charges, sessions, sale, check };
}

test('local enrollment previews, binds the original channel and survives replays', async () => {
  const { commerce } = fixture();
  assert.equal((await adoptionReport(commerce)).status, 'unenrolled');
  assert.deepEqual(await enrollStore(commerce, 'partner', false, start), {
    mode: 'preview',
    source: 'partner',
  });
  assert.equal((await adoptionReport(commerce)).status, 'unenrolled');
  const [left, right] = await Promise.all([
    enrollStore(commerce, 'partner', true, start),
    enrollStore(commerce, 'partner', true, start + 2000),
  ]);
  assert.deepEqual(left, right);
  await assert.rejects(enrollStore(commerce, 'other', true, start), /different/);
  await assert.rejects(enrollStore(commerce, 'buyer@email', true, start), /acquisition channel/);
});

test('only verified independent live card orders qualify for activation and week-later reuse', async () => {
  const { commerce, sale, check, sessions, orders, charges } = fixture();
  await enrollStore(commerce, 'organic', true, start);
  for (const [id, date] of [
    ['old', -1],
    ['first', 1],
    ['same', 2],
    ['week', 10],
    ['upsell', 9],
    ['wrong', 6],
    ['failure', 4],
    ['base', 3],
    ['future', 32],
    ['unpaid', 3],
  ])
    await sale(id, date);
  await sale('test', 0, { test: true });
  await saveMerchantRecord(commerce, 'campaign-order-upsell', { parentOrderId: 'first' });
  orders.get('base').notes = 'Verified Base settlement: fixture; Cart: fixture';
  sessions.get('cs_live_wrong').payment_intent.amount_received = 1249;
  sessions.delete('cs_live_failure');
  charges.get('unpaid').status = 'processing';
  const preview = await adoptionReport(commerce, { now: check.now });
  assert.equal(preview.coverage, 'partial');
  assert.equal(preview.counts.unchecked, 7);
  const report = await adoptionReport(commerce, check);
  assert.deepEqual(report.counts, {
    live: 3,
    sandbox: 1,
    unchecked: 0,
    unavailable: 1,
    mismatch: 2,
    otherRails: 1,
    supplemental: 1,
  });
  assert.equal(report.coverage, 'partial');
  assert.equal(report.firstVerifiedSaleAt, iso(1.25));
  assert.equal(report.hoursToFirstVerifiedSale, 30);
  assert.equal(report.repeatVerifiedSaleAt, iso(10.25));
  assert.doesNotMatch(JSON.stringify(report), /private|cs_live_|pi_/);
});

test('checks the original routed provider payment, location, amount and environment', async () => {
  const { commerce, sale, charges, check } = fixture();
  await enrollStore(commerce, 'referral', true, start);
  await sale('square', 1, { routed: true });
  charges.get('square').idempotencyKey = 'routed:square';
  const token = 'x'.repeat(43);
  await saveMerchantRecord(commerce, 'routed-access-square', { token });
  await saveMerchantRecord(
    commerce,
    'routed-session-' + createHash('sha256').update(token).digest('hex'),
    {
      orderId: 'square',
      paymentId: 'payment-square',
      accountIds: ['retail'],
    },
  );
  const binding = { provider: 'square', account: 'retail', reference: 'remote-id' };
  await saveMerchantRecord(commerce, 'routed-authorization-square', binding);
  commerce.payments.routeCard = async ({ action, paymentId }) => {
    assert.equal(action, 'selected');
    assert.equal(paymentId, 'payment-square');
    return { state: 'selected', ...binding };
  };
  const entry = {
    provider: 'square',
    account: 'retail',
    publicKey: 'sq0idp-setup',
    location: 'location',
    secretEnv: 'SQUARE_TOKEN',
    sandbox: false,
    currencies: ['USD'],
  };
  check.env = { SQUARE_TOKEN: 'test-only', STATESET_CARD_ACCOUNTS_JSON: JSON.stringify([entry]) };
  const remote = {
    id: 'remote-id',
    status: 'COMPLETED',
    location_id: 'location',
    amount_money: { amount: 1250, currency: 'USD' },
  };
  let url;
  check.fetcher = async (target) => {
    url = target;
    return new Response(JSON.stringify({ payment: remote }));
  };
  assert.equal((await adoptionReport(commerce, check)).counts.live, 1);
  assert.equal(url, 'https://connect.squareup.com/v2/payments/remote-id');
  check.env.STATESET_CARD_ACCOUNTS_JSON = JSON.stringify([
    { ...entry, publicKey: 'sandbox-sq0idp-setup', sandbox: true },
  ]);
  assert.equal((await adoptionReport(commerce, check)).counts.sandbox, 1);
  assert.match(url, /connect.squareupsandbox.com/);
  remote.location_id = 'different';
  assert.equal((await adoptionReport(commerce, check)).counts.mismatch, 1);
  check.fetcher = async () => new Response(null, { status: 403 });
  assert.equal((await adoptionReport(commerce, check)).counts.unavailable, 1);
});

test('cohorts deduplicate reports and honor observation windows, incomplete checks, and zero-sale stores', async () => {
  const { commerce, sale, check } = fixture();
  await enrollStore(commerce, 'cohort-a', true, start);
  const earlier = await adoptionReport(commerce, { ...check, now: start + day });
  await sale('first', 0);
  await sale('next', 9);
  const full = await adoptionReport(commerce, check);
  const newReport = (i, data) => ({
    ...full,
    installationId: '00000000-0000-4000-8000-' + String(i).padStart(12, '0'),
    ...data,
  });
  const rows = [
    earlier,
    full,
    full,
    newReport(1, { firstVerifiedSaleAt: null, repeatVerifiedSaleAt: null }),
    newReport(2, { coverage: 'partial' }),
    newReport(3, { observedAt: iso(9), repeatVerifiedSaleAt: null }),
    newReport(4, { firstVerifiedSaleAt: iso(7), repeatVerifiedSaleAt: iso(30) }),
  ];
  assert.deepEqual(groupAdoptionReports(rows), [
    {
      source: 'cohort-a',
      submittedStores: 5,
      incompleteStores: 1,
      activation7d: { eligible: 4, achieved: 2 },
      repeat30d: { eligible: 3, achieved: 1 },
    },
  ]);
  assert.throws(
    () => groupAdoptionReports([full, { ...full, source: 'different' }]),
    /Conflicting/,
  );
  assert.throws(() => groupAdoptionReports([{ ...full, repeatVerifiedSaleAt: iso(1) }]), /Invalid/);
});

test('partial and preview results stay retryable; verified historical receipts can be retained for offline reporting', async () => {
  const { commerce, sale, check } = fixture();
  await enrollStore(commerce, 'partner', true, start);
  await sale('stored', 1);
  const offline = () => {
    throw new Error('no credentials');
  };
  assert.equal(
    (await adoptionReport(commerce, { ...check, stripeFor: offline, apply: true })).counts
      .unavailable,
    1,
  );
  assert.equal((await adoptionReport(commerce, check)).counts.live, 1);
  assert.equal((await adoptionReport(commerce, { ...check, stripeFor: offline })).counts.live, 0);
  const retained = await adoptionReport(commerce, { ...check, apply: true });
  assert.equal(retained.counts.live, 1);
  assert.deepEqual(await adoptionReport(commerce, { now: check.now }), retained);
  await assert.rejects(adoptionReport(commerce, { apply: true }), /--check/);
});

test('membership activation requires a paid signup invoice bound to the original native payment', async () => {
  const { commerce, check, sale, sessions } = fixture();
  await enrollStore(commerce, 'partner', true, start);
  await sale('member', 1);
  const session = sessions.get('cs_live_member');
  session.mode = 'subscription';
  session.invoice = 'in_signup';
  const intent = { ...session.payment_intent, customer: 'cus_checkout' };
  delete session.payment_intent;
  const invoice = {
    id: 'in_signup',
    customer: 'cus_checkout',
    currency: 'usd',
    livemode: true,
    status: 'paid',
    billing_reason: 'subscription_create',
    amount_paid: 1250,
  };
  const tx = {
    invoice: invoice.id,
    status: 'paid',
    currency: 'usd',
    amount_paid: 1250,
    payment: { type: 'payment_intent', payment_intent: intent },
  };
  check.stripeFor = () => ({
    checkout: { sessions: { retrieve: async (id) => sessions.get(id) } },
    invoices: {
      retrieve: async (id) => {
        assert.equal(id, invoice.id);
        return invoice;
      },
    },
    invoicePayments: { list: async () => ({ has_more: false, data: [tx] }) },
  });
  assert.equal((await adoptionReport(commerce, check)).counts.live, 1);
  tx.payment.payment_intent = { ...intent, id: 'pi_other' };
  assert.equal((await adoptionReport(commerce, check)).counts.mismatch, 1);
  tx.payment.payment_intent = intent;
  invoice.billing_reason = 'subscription_cycle';
  assert.equal((await adoptionReport(commerce, check)).counts.mismatch, 1);
});

test('routed Stripe honors the original selected account and processor-reported mode', async () => {
  const { commerce, sale, charges, check } = fixture();
  await enrollStore(commerce, 'direct', true, start);
  await sale('card', 1, { routed: true });
  charges.get('card').idempotencyKey = 'routed:card';
  const token = 't'.repeat(43),
    binding = { account: 'online', provider: 'stripe', reference: 'pi_card' };
  await saveMerchantRecord(commerce, 'routed-access-card', { token });
  await saveMerchantRecord(
    commerce,
    'routed-session-' + createHash('sha256').update(token).digest('hex'),
    {
      orderId: 'card',
      paymentId: 'payment-card',
      accountIds: ['online'],
    },
  );
  await saveMerchantRecord(commerce, 'routed-authorization-card', binding);
  const native = { state: 'selected', ...binding };
  commerce.payments.routeCard = async () => native;
  check.env = {
    CARD_KEY: 'sk_test_fixture',
    STATESET_CARD_ACCOUNTS_JSON: JSON.stringify([
      {
        provider: 'stripe',
        account: 'online',
        secretEnv: 'CARD_KEY',
        publicKey: 'pk_test_fixture',
        currencies: ['USD'],
      },
    ]),
  };
  const intent = {
    id: 'pi_card',
    status: 'succeeded',
    currency: 'usd',
    amount_received: 1250,
    livemode: false,
  };
  check.stripeFor = (key) => {
    assert.equal(key, check.env.CARD_KEY);
    return { paymentIntents: { retrieve: async () => intent } };
  };
  assert.equal((await adoptionReport(commerce, check)).counts.sandbox, 1);
  native.reference = 'pi_other';
  assert.equal((await adoptionReport(commerce, check)).counts.mismatch, 1);
});
