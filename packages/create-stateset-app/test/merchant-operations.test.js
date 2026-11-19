import test from 'node:test';
import assert from 'node:assert/strict';
import {
  linkBillingSubscription,
  membershipExpiry,
  recordMembershipStatus,
  recordPaidRenewal,
  recordRenewalFailure,
  recoveryReport,
  retrySchedule,
  recoveryPortal,
  runRenewalRecovery,
  sendRecoveryEmail,
} from '../templates/storefront/lib/billing-recovery.js';
import {
  installIntegration,
  processIntegrations,
  recordStorefrontSale,
  recordStorefrontRefund,
} from '../templates/storefront/lib/merchant-integrations.js';

function store() {
  const rows = new Map();
  return {
    customObjects: {
      getTypeByHandle: async () => ({ id: 'type' }),
      getObjectByHandle: async (_, key) => rows.get(key),
      createObject: async (row) => {
        if (rows.has(row.handle)) throw new Error('duplicate');
        rows.set(row.handle, { ...row, id: row.handle });
        return rows.get(row.handle);
      },
      deleteObject: async (id) => rows.delete(id),
      listObjects: async (filter) =>
        [...rows.values()]
          .filter((r) => r.ownerId === filter.ownerId)
          .slice(filter.offset, filter.offset + filter.limit),
    },
    customers: { get: async () => ({ firstName: 'Ada', lastName: 'Sample' }) },
    orders: { get: async () => ({ status: 'confirmed' }) },
  };
}

test('failed renewal recovers once, retains original risk, and bounds the customer portal link', async () => {
  const commerce = store();
  const now = 1_800_000_000_000;
  const env = {
    STATESET_RECOVERY_SECRET: 'x'.repeat(40),
    NEXT_PUBLIC_BASE_URL: 'https://shop.example.com',
  };
  const invoice = {
    id: 'in_test',
    customer: 'cus_one',
    currency: 'usd',
    customer_email: 'ada@example.test',
    amount_remaining: 1499,
    amount_paid: 0,
    status: 'open',
    billing_reason: 'subscription_cycle',
    collection_method: 'charge_automatically',
    auto_advance: true,
    attempt_count: 1,
  };
  const event = {
    id: 'evt_fail',
    type: 'invoice.payment_failed',
    created: now / 1000 - 86400,
    data: { object: invoice },
  };
  await recordRenewalFailure(commerce, event);
  await recordRenewalFailure(commerce, event);
  let charges = 0,
    notice;
  const stripe = {
    invoices: {
      retrieve: async () => invoice,
      pay: async () => {
        charges++;
      },
    },
    billingPortal: {
      sessions: {
        create: async (params) => {
          assert.equal(params.customer, invoice.customer);
          return { url: 'https://billing.stripe.com/portal' };
        },
      },
    },
  };
  const options = {
    commerce,
    stripe,
    env,
    now,
    notify: async (input) => {
      notice = input;
    },
  };
  assert.equal((await runRenewalRecovery(options)).actions.length, 2);
  assert.equal(charges, 0);
  await runRenewalRecovery({ ...options, apply: true });
  await runRenewalRecovery({ ...options, apply: true });
  assert.equal(charges, 1);
  const token = new URL(notice.updateUrl).searchParams.get('token');
  assert.equal(await recoveryPortal(stripe, env, token, now), 'https://billing.stripe.com/portal');
  await assert.rejects(
    recoveryPortal(stripe, env, token.replace('in_test', 'in_other'), now),
    /invalid/,
  );
  await assert.rejects(recoveryPortal(stripe, env, token, now + 15 * 86400000), /expired/);
  invoice.amount_remaining = 0;
  invoice.amount_paid = 1499;
  invoice.status = 'paid';
  assert.deepEqual((await recoveryReport(commerce, stripe)).currencies.usd, {
    failedMinor: '1499',
    recoveredMinor: '1499',
    outstandingMinor: '0',
  });
  assert.throws(() => retrySchedule({ STATESET_RECOVERY_RETRY_DAYS: '[3,1]' }));
  await sendRecoveryEmail(
    { RESEND_API_KEY: 'test', STATESET_RECOVERY_FROM: 'shop@example.test' },
    notice,
    async (_, opts) => {
      assert.equal(opts.headers['Idempotency-Key'], notice.key);
      assert.ok(JSON.parse(opts.body).text.includes(notice.updateUrl));
      return new Response('{}');
    },
  );
});

test('one uncertain retry leaves other failed invoices collectible in the same pass', async () => {
  const commerce = store();
  const failedAt = 1_800_000_000;
  const invoices = new Map();
  for (const id of ['in_first', 'in_second']) {
    const invoice = {
      id,
      customer: 'cus_' + id,
      currency: 'usd',
      amount_paid: 0,
      amount_remaining: 900,
      status: 'open',
      billing_reason: 'subscription_cycle',
      collection_method: 'charge_automatically',
      auto_advance: true,
      attempt_count: 1,
    };
    invoices.set(id, invoice);
    await recordRenewalFailure(commerce, {
      type: 'invoice.payment_failed',
      id: 'evt_' + id,
      created: failedAt,
      data: { object: invoice },
    });
  }
  const requests = [];
  const stripe = {
    invoices: {
      retrieve: async (id) => invoices.get(id),
      pay: async (id) => {
        requests.push(id);
        invoices.get(id).attempt_count++;
        if (id === 'in_first') throw new Error('response timed out');
      },
    },
  };
  const options = { commerce, stripe, env: {}, apply: true, now: (failedAt + 86400) * 1000 };
  await assert.rejects(runRenewalRecovery(options), (error) => {
    assert.deepEqual(requests, ['in_first', 'in_second']);
    assert.deepEqual(error.report.failures, [{ invoice: 'in_first', error: 'response timed out' }]);
    return true;
  });
  await runRenewalRecovery(options);
  assert.deepEqual(requests, ['in_first', 'in_second']);
});

test('a later retry day reconciles a timed-out renewal before dispatching another charge', async () => {
  const commerce = store();
  const now = 1800000000;
  const invoice = {
    id: 'in_late',
    customer: 'cus_retry',
    currency: 'usd',
    amount_paid: 0,
    amount_remaining: 400,
    status: 'open',
    billing_reason: 'subscription_cycle',
    collection_method: 'charge_automatically',
    auto_advance: true,
    attempt_count: 1,
  };
  await recordRenewalFailure(commerce, {
    type: 'invoice.payment_failed',
    id: 'evt_late',
    created: now,
    data: { object: invoice },
  });
  let calls = 0;
  const stripe = {
    invoices: {
      retrieve: async () => invoice,
      pay: async () => {
        if (++calls === 1) throw new Error('timeout');
      },
    },
  };
  const run = (days) =>
    runRenewalRecovery({
      commerce,
      stripe,
      env: {},
      apply: true,
      now: (now + 86400 * days) * 1000,
    });
  await assert.rejects(run(1), /timeout/);
  assert.ok((await run(3)).actions.some((row) => row.action === 'review-uncertain-retry'));
  assert.equal(calls, 1);
  invoice.attempt_count++;
  await run(3);
  assert.equal(calls, 2);
});

test('verified renewal metadata links the failed invoice to the native subscriber', async () => {
  const commerce = store();
  const native = { id: 'native-subscription', customerId: 'native-customer', currency: 'USD' };
  commerce.subscriptions = { get: async (id) => (id === native.id ? native : null) };
  const metadata = { stateset_subscription_id: native.id, stateset_customer_id: native.customerId };
  const subscription = { id: 'sub_456', customer: 'cus_123', metadata, currency: 'usd' };
  const invoice = {
    id: 'in_bound',
    customer: subscription.customer,
    currency: 'usd',
    amount_paid: 0,
    amount_remaining: 600,
    status: 'open',
    auto_advance: true,
    next_payment_attempt: 1800100000,
    billing_reason: 'subscription_cycle',
    parent: { subscription_details: { subscription: subscription.id, metadata } },
  };
  const stripe = {
    invoices: { retrieve: async () => invoice },
    subscriptions: { retrieve: async () => subscription },
  };
  const event = {
    id: 'evt_bound',
    type: 'invoice.payment_failed',
    created: 1800000000,
    data: { object: invoice },
  };
  await recordRenewalFailure(commerce, event, stripe);
  const report = await recoveryReport(commerce, stripe);
  assert.equal(report.invoices[0].subscriptionId, native.id);
  assert.equal(report.invoices[0].nextRetry, new Date(1800100000000).toISOString());
  invoice.amount_paid = 150;
  invoice.amount_remaining = 450;
  assert.equal((await recoveryReport(commerce, stripe)).recoveryPercent.usd, '25.00');
  invoice.amount_paid = 600;
  invoice.amount_remaining = 0;
  invoice.status = 'paid';
  invoice.lines = {
    data: [
      {
        id: 'il_1',
        amount: 600,
        currency: 'usd',
        parent: { subscription_item_details: { subscription: subscription.id, proration: false } },
        period: { start: 1800000000, end: 1802000000 },
      },
    ],
  };
  await recordPaidRenewal(commerce, stripe, invoice.id);
  await recordPaidRenewal(commerce, stripe, invoice.id);
  assert.equal(
    await membershipExpiry(commerce, native, 1801000000000, 1800000000000),
    1802000000000,
  );
  subscription.status = 'canceled';
  subscription.ended_at = 1801500000;
  await recordMembershipStatus(commerce, stripe, {
    id: 'evt_ended',
    data: { object: subscription },
  });
  assert.equal(
    await membershipExpiry(commerce, native, 1801600000000, 1800000000000),
    1801500000000,
  );
  subscription.metadata = { ...metadata, stateset_customer_id: 'wrong' };
  await assert.rejects(
    linkBillingSubscription(commerce, stripe, native.id, subscription.id, true),
    /does not match/,
  );
});

test('configured merchant integrations deliver once and replay a failed delivery by key', async () => {
  const commerce = store();
  const env = {
    SHIPSTATION_API_KEY: 'key',
    SHIPSTATION_API_SECRET: 'secret',
    KLAVIYO_PRIVATE_KEY: 'key',
    GORGIAS_DOMAIN: 'demo',
    GORGIAS_EMAIL: 'agent@example.test',
    GORGIAS_API_KEY: 'key',
  };
  let shipAttempts = 0,
    tickets = 0;
  const events = new Map();
  const fetchImpl = async (url, options) => {
    if (url.includes('/orders?')) return Response.json({ page: 1, pages: 0, orders: [] });
    if (url.includes('/orders/createorder')) {
      shipAttempts++;
      assert.ok(options.body.includes('"unitPrice":10.50'));
      return Response.json(
        { ...JSON.parse(options.body), orderId: 42 },
        { status: shipAttempts === 1 ? 429 : 200 },
      );
    }
    if (url.endsWith('/api/events/')) {
      const data = JSON.parse(options.body).data.attributes;
      events.set(data.unique_id, data);
      return new Response('', { status: 202 });
    }
    if (url.includes('/api/profiles/?'))
      return Response.json({
        data: [{ id: 'p', type: 'profile', attributes: { email: 'ada@example.test' } }],
      });
    if (url.includes('/api/events/?'))
      return Response.json({
        data: [...events.values()].map((e, i) => ({
          type: 'event',
          id: 'e' + i,
          attributes: { event_properties: { $event_id: e.unique_id, ...e.properties } },
          relationships: { profile: { data: { id: 'p' } }, metric: { data: { id: 'm' + i } } },
        })),
        included: [...events.values()].map((e, i) => ({
          type: 'metric',
          id: 'm' + i,
          attributes: { name: e.metric.data.attributes.name },
        })),
      });
    if (url.includes('tickets?')) return Response.json({ data: [] });
    if (url.endsWith('tickets')) {
      tickets++;
      return Response.json({ id: 17, external_id: JSON.parse(options.body).external_id });
    }
    return Response.json({});
  };
  for (const name of ['shipstation', 'klaviyo', 'gorgias'])
    await installIntegration(commerce, env, name, true, fetchImpl);
  const at = new Date(Date.now() + 1000).toISOString();
  const order = {
    id: 'order1',
    customerId: 'cust',
    orderNumber: '1',
    currency: 'USD',
    totalAmountExact: '10.5',
    createdAt: at,
    shippingAddress: {
      line1: '1st St',
      city: 'City',
      state: 'NY',
      postalCode: '10001',
      country: 'US',
    },
    items: [{ sku: 'SKU', name: 'Item', quantity: 1, unitPriceExact: '10.50' }],
  };
  await recordStorefrontSale(commerce, order, 'ada@example.test');
  await recordStorefrontRefund(commerce, order.id, {
    id: 'refund1',
    status: 'completed',
    amountExact: '1.25',
    createdAt: at,
  });
  const first = await processIntegrations(commerce, env, { apply: true, fetchImpl });
  assert.equal(
    first.filter((row) => row.status === 'accepted' && row.provider === 'klaviyo').length,
    2,
  );
  const failed = first.find((row) => row.status === 'failed');
  assert.equal(failed.provider, 'shipstation');
  assert.equal(
    (await processIntegrations(commerce, env, { onlyKey: failed.key }))[0].status,
    'failed',
  );
  await processIntegrations(commerce, env, { apply: true, onlyKey: failed.key, fetchImpl });
  assert.ok(
    (await processIntegrations(commerce, env, { apply: true, fetchImpl })).every(
      (row) => row.status === 'delivered',
    ),
  );
  assert.equal(tickets, 1);
  assert.equal(shipAttempts, 2);
  assert.equal(events.size, 2);
});

import {
  campaignAccounting,
  enterCampaign,
  decodeCampaign,
  registerAffiliate,
  recordCampaignStep,
  affiliateExport,
} from '../templates/storefront/lib/campaigns.js';
import { saveMerchantRecord } from '../templates/storefront/lib/merchant-records.js';
import {
  findGorgiasTicket,
  findShipStationOrder,
} from '../templates/storefront/lib/integration-receipts.js';

test('provider lookup rejects ambiguous tickets and ignores adjacent order-number matches', async () => {
  const key = 'delivery-original';
  await assert.rejects(
    findGorgiasTicket(
      async () => ({
        data: [
          { id: 3, external_id: key },
          { id: 4, external_id: key },
        ],
      }),
      key,
    ),
    /Multiple/,
  );
  await assert.rejects(
    findGorgiasTicket(async () => ({ data: [{ id: 3, external_id: 'wrong' }] }), key),
    /differs/,
  );
  const event = { orderId: 'native', number: '12' };
  assert.equal(
    await findShipStationOrder(
      async () => ({
        orders: [{ orderKey: 'other', orderNumber: '123', orderId: 3 }],
        page: 1,
        pages: 1,
      }),
      event,
    ),
    undefined,
  );
  await assert.rejects(
    findShipStationOrder(
      async () => ({
        orders: [{ orderKey: 'native', orderNumber: '12', orderId: null }],
        page: 1,
        pages: 1,
      }),
      event,
    ),
    /invalid resource ID/,
  );
});

test('fully refunded orders are skipped and historical acknowledgements need matching provider evidence', async () => {
  const commerce = store(),
    env = { SHIPSTATION_API_KEY: 'example', SHIPSTATION_API_SECRET: 'example' };
  const fetchImpl = async (url, opts) => {
    assert.equal(opts.method, 'GET');
    return Response.json(url.includes('/stores') ? [] : { orders: [], page: 1, pages: 0 });
  };
  await installIntegration(commerce, env, 'shipstation', true, fetchImpl);
  const order = {
    id: 'refunded',
    customerId: 'customer',
    orderNumber: 'SS1',
    currency: 'USD',
    totalAmountExact: '7',
    items: [{ name: 'Coffee', sku: 'COFFEE', unitPriceExact: '7', quantity: 1 }],
    createdAt: new Date().toISOString(),
  };
  await recordStorefrontSale(commerce, order, 'a@store.test');
  commerce.orders.get = async () => ({
    status: 'confirmed',
    fulfillmentStatus: 'unfulfilled',
    paymentStatus: 'refunded',
  });
  const [skipped] = await processIntegrations(commerce, env, { apply: true, fetchImpl });
  assert.deepEqual([skipped.status, skipped.message], ['skipped', 'refunded']);
  assert.equal((await processIntegrations(commerce, env))[0].status, 'skipped');
  await recordStorefrontSale(
    commerce,
    { ...order, id: 'legacy', orderNumber: 'SS2' },
    'b@store.test',
  );
  const { key } = (await processIntegrations(commerce, env)).find((r) => r.orderId === 'legacy');
  await saveMerchantRecord(commerce, key, { externalId: '123' });
  const [missing] = await processIntegrations(commerce, env, {
    apply: true,
    onlyKey: key,
    fetchImpl,
  });
  assert.equal(missing.status, 'accepted');
  assert.equal(missing.externalId, undefined);
  const [recovered] = await processIntegrations(commerce, env, {
    apply: true,
    onlyKey: key,
    fetchImpl: async (_, opts) => {
      assert.equal(opts.method, 'GET');
      return Response.json({
        orders: [{ orderId: 123, orderKey: 'legacy', orderNumber: 'SS2' }],
        page: 1,
        pages: 1,
      });
    },
  });
  assert.equal(recovered.externalId, '123');
  assert.equal(recovered.history.length, 2);
});
import { resolveRenewalSeller } from '../templates/storefront/lib/seller-payouts.js';
import { renewalSellerTerms } from '../templates/storefront/lib/renewal-payments.js';

test('historical renewals preserve provable splits and require explicit allocation for mixed orders', async () => {
  const commerce = store();
  const split = { seller: 'studio', account: 'acct_a', gross: '20', fee: '2', payable: '18' };
  const item = { sku: 'CLUB', quantity: 1, unitPrice: '20' };
  await saveMerchantRecord(commerce, 'seller-studio', { account: split.account });
  for (const key of ['single', 'mixed']) {
    await saveMerchantRecord(commerce, 'provider-enrollment-' + key, { orderId: key });
    await saveMerchantRecord(commerce, 'stripe-membership-' + key, { sku: 'CLUB' });
    await saveMerchantRecord(commerce, 'seller-order-' + key, { allocations: [split] });
    await saveMerchantRecord(commerce, 'merchandise-' + key, {
      items: key === 'single' ? [item] : [item, { sku: 'POSTER', quantity: 1, unitPrice: '5' }],
    });
  }
  assert.deepEqual((await renewalSellerTerms(commerce, 'single')).allocations, [split]);
  assert.equal((await renewalSellerTerms(commerce, 'mixed')).status, 'needs review');
  await assert.rejects(
    resolveRenewalSeller(commerce, 'mixed', { seller: 'unrelated', bps: '1500', apply: true }),
    /original purchase/,
  );
  const selection = { seller: 'studio', bps: '1500' };
  const preview = await resolveRenewalSeller(commerce, 'mixed', selection);
  assert.equal(preview.allocations[0].payable, '17');
  assert.equal((await renewalSellerTerms(commerce, 'mixed')).status, 'needs review');
  await resolveRenewalSeller(commerce, 'mixed', { ...selection, apply: true });
  assert.deepEqual((await renewalSellerTerms(commerce, 'mixed')).allocations, preview.allocations);
});

test('first-touch assignment and net revenue per visitor survive refreshes and partial refunds', async () => {
  const commerce = store();
  const env = {
    STATESET_CAMPAIGN_SECRET: 'y'.repeat(40),
    STATESET_CAMPAIGNS_JSON: JSON.stringify({
      launch: {
        variants: [
          { id: 'a', weight: 1, title: 'A', sku: 'coffee' },
          { id: 'b', weight: 1, title: 'B', sku: 'coffee' },
        ],
      },
    }),
  };
  await registerAffiliate(commerce, 'partner', 1000, true);
  const visit = await enterCampaign(commerce, env, {
    campaignId: 'launch',
    source: 'newsletter',
    affiliate: 'partner',
  });
  const revisit = await enterCampaign(commerce, env, {
    campaignId: 'launch',
    token: visit.token,
    source: 'other',
  });
  assert.deepEqual(visit.assignment, revisit.assignment);
  assert.equal(decodeCampaign(env, visit.token).affiliate, 'partner');
  assert.equal(decodeCampaign(env, visit.token + 'x'), null);
  await recordCampaignStep(commerce, visit.assignment, 'checkout', 'order-1');
  await saveMerchantRecord(commerce, 'campaign-order-order-1', { ...visit.assignment, bump: true });
  await recordStorefrontSale(
    commerce,
    {
      id: 'order-1',
      currency: 'USD',
      totalAmountExact: '12',
      items: [{ sku: 'coffee', unitPriceExact: '10', quantity: 1 }],
      createdAt: '2026-11-01T00:00:00Z',
    },
    'buyer@example.test',
  );
  for (const [id, amount] of [
    ['r1', '1.25'],
    ['r2', '1.75'],
  ]) {
    await recordStorefrontRefund(commerce, 'order-1', {
      id,
      amountExact: amount,
      status: 'completed',
      createdAt: '2026-11-02T00:00:00Z',
    });
  }
  const report = await campaignAccounting(commerce);
  assert.equal(report.variants.length, 1);
  assert.equal(report.variants[0].visitors, 1);
  assert.equal(report.variants[0].netRevenuePerVisitor, '7.5');
  assert.equal(report.commissions[0].earned, '0.75');
  assert.equal(report.commissions[0].reversal, '0.25');
  assert.ok(
    (await affiliateExport(commerce)).includes('"partner","order-1","USD","1","0.25","0.75"'),
  );
});

test('routed account checks distinguish connected sandbox locations, disabled cards and settlement', async () => {
  const { verifyCardAccount } = await import('../templates/storefront/lib/card-accounts.js');
  const account = {
    account: 'west',
    provider: 'square',
    secret: 'private-fixture',
    sandbox: true,
    location: 'west-store',
  };
  const location = {
    id: 'west-store',
    merchant_id: 'merchant',
    currency: 'USD',
    status: 'ACTIVE',
    capabilities: ['CREDIT_CARD_PROCESSING'],
  };
  const fetcher = (body) => (url, init) => {
    assert.equal(url, 'https://connect.squareupsandbox.com/v2/locations/west-store');
    assert.equal(init.redirect, 'error');
    assert.equal(init.headers.Authorization, 'Bearer private-fixture');
    return Response.json({ location: body });
  };
  const status = await verifyCardAccount(account, fetcher(location));
  assert.equal(status.identity, 'merchant/west-store');
  assert.equal(status.settlementVerified, false);
  assert.equal(status.chargesEnabled, true);
  assert.equal(status.mode, 'sandbox');
  assert.ok(!JSON.stringify(status).includes(account.secret));
  assert.equal(
    (await verifyCardAccount(account, fetcher({ ...location, capabilities: [] }))).chargesEnabled,
    false,
  );
  await assert.rejects(
    verifyCardAccount(account, fetcher({ ...location, id: 'elsewhere' })),
    /different/,
  );
  const stripe = await verifyCardAccount(
    { account: 'cards', provider: 'stripe', secret: 'sk_test_fixture' },
    (url) => {
      assert.equal(url, 'https://api.stripe.com/v1/account');
      return Response.json({ id: 'acct_merchant', charges_enabled: false });
    },
  );
  assert.equal(stripe.chargesEnabled, false);
  assert.equal(stripe.identity, 'acct_merchant');
});

test('campaign drop-offs conserve visits and compare net revenue within the same revision and source', async () => {
  const commerce = store();
  const { campaignAccounting, recordCampaignStep } =
    await import('../templates/storefront/lib/campaigns.js');
  async function visitor(visitor, variant, source = 'email', revision = 1) {
    const terms = {
      campaignId: 'launch',
      revision,
      variant,
      source,
      visitor,
      at: '2026-01-04T12:00:00Z',
    };
    await saveMerchantRecord(commerce, 'visit-' + visitor, terms, 'campaign-visits');
    return terms;
  }
  async function bought(terms, id, amount) {
    await saveMerchantRecord(commerce, 'campaign-order-' + id, terms);
    await saveMerchantRecord(
      commerce,
      'sale-' + id,
      {
        type: 'sale',
        orderId: id,
        currency: 'USD',
        total: amount,
        items: [{ unitPrice: amount, quantity: 1 }],
      },
      'merchant-events',
    );
  }
  await visitor('landing', 'a');
  const checkout = await visitor('checkout', 'a'),
    pay = await visitor('payment', 'a'),
    customer = await visitor('buy', 'a');
  await recordCampaignStep(commerce, checkout, 'checkout');
  await recordCampaignStep(commerce, pay, 'payment-start', 'order-1');
  await recordCampaignStep(commerce, pay, 'payment-start', 'order-2');
  await bought(customer, 'alpha', '8');
  const b = await visitor('b-buyer', 'b');
  await bought(b, 'beta', '10');
  await saveMerchantRecord(
    commerce,
    'refund-beta',
    { orderId: 'beta', type: 'refund', amount: '4' },
    'merchant-events',
  );
  await visitor('b-empty', 'b');
  await visitor('independent', 'b', 'search');
  await visitor('new-revision', 'b', 'email', 2);
  const report = await campaignAccounting(commerce, {
    bySource: true,
    control: 'a',
    includeOrders: true,
  });
  const a = report.variants.find((r) => r.variant === 'a');
  assert.deepEqual(
    [a.beforeCheckout, a.checkoutWithoutPayment, a.paymentWithoutPurchase, a.purchasingVisitors],
    [1, 1, 1, 1],
  );
  const treatment = report.variants.find(
    (r) => r.variant === 'b' && r.source === 'email' && r.revision === 1,
  );
  assert.equal(treatment.netRevenuePerVisitor, '3');
  assert.equal(treatment.comparisonVariant, 'a');
  assert.equal(treatment.netRevenuePerVisitorDifference, '1');
  assert.equal(treatment.netRevenuePerVisitorChangePercent, '50');
  assert.equal(
    report.variants.find((r) => r.source === 'search').netRevenuePerVisitorChangePercent,
    null,
  );
  assert.equal(report.variants.find((r) => r.revision === 2).comparisonVariant, null);
  assert.equal(report.orders.find((r) => r.orderId === 'beta').net, '6');
  const reversed = await campaignAccounting(commerce, { bySource: true, control: 'b' });
  assert.equal(
    reversed.variants.find((r) => r.variant === 'a').netRevenuePerVisitorChangePercent,
    '-33.33',
  );
  assert.equal(
    reversed.variants.find((r) => r.source === 'search').netRevenuePerVisitorChangePercent,
    null,
  );
});

test('Connect payouts can use the platform account independently of the buyer checkout', async () => {
  const { getStripeConfiguration, getStripePlatformConfiguration } =
    await import('../templates/storefront/lib/stripe-config.js');
  const env = {
    STATESET_CHECKOUT_PROVIDER: 'base',
    STRIPE_SECRET_KEY: 'sk_test_example',
    NEXT_PUBLIC_BASE_URL: 'https://store.example.test',
  };
  assert.deepEqual(getStripePlatformConfiguration(env), {
    key: 'sk_test_example',
    origin: 'https://store.example.test',
  });
  assert.throws(() => getStripeConfiguration(env), /disabled/);
  assert.throws(
    () => getStripeConfiguration({ ...env, STATESET_CHECKOUT_PROVIDER: 'stripe' }),
    /webhook/,
  );
  assert.throws(
    () =>
      getStripePlatformConfiguration({
        ...env,
        NEXT_PUBLIC_BASE_URL: 'https://user@store.example.test',
      }),
    /origin/,
  );
});
