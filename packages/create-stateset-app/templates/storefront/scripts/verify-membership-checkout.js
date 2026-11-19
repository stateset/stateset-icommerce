import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { Commerce } from '@stateset/embedded';
import { importCatalog } from '../lib/merchant-onboarding.js';
import { enterCampaign } from '../lib/campaigns.js';
import { settleSellers } from '../lib/seller-payouts.js';
import { portalReport } from '../lib/merchant-portal.js';
import {
  startStripeCheckout,
  startCheckoutUpsell,
  reconcileStripeCheckout,
  reconcileStripeRefund,
  refundStripePayment,
} from '../lib/stripe-checkout.js';
import {
  quoteMembership,
  subscriptionCheckoutParameters,
  membershipManagement,
} from '../lib/membership-checkout.js';
import { verifyStripeRecovery } from '../lib/stripe-recovery-proof.js';
import { checkDigitalAccess, licensesForOrder } from '../lib/digital-access.js';
import {
  membershipExpiry,
  reconcileRenewals,
  recordMembershipStatus,
  recordPaidRenewal,
  recordRenewalFailure,
  runRenewalRecovery,
  renewalStatement,
} from '../lib/billing-recovery.js';
import { readMerchantRecord, saveMerchantRecord } from '../lib/merchant-records.js';
const dir = mkdtempSync(join(tmpdir(), 'stateset-membership-'));
const db = join(dir, 'store.db'),
  origin = 'https://store.example.test';
let commerce = new Commerce(db);
const now = Math.floor(Date.now() / 1000),
  end = now + 28 * 86400,
  next = end + 30 * 86400;
try {
  const env = process.env;
  Object.assign(env, {
    STATESET_LICENSE_SECRET: 'a'.repeat(48),
    STATESET_CAMPAIGN_SECRET: 'c'.repeat(48),
    STATESET_SELLER_SKUS_JSON: '{"STUDIO":{"seller":"studio","feeBps":1000}}',
    NEXT_PUBLIC_STRIPE_PUBLISHABLE_KEY: 'pk_test_membership',
    STATESET_CAMPAIGNS_JSON: JSON.stringify({
      launch: {
        variants: [{ id: 'a', weight: 1, title: 'Studio', sku: 'STUDIO', upsellSku: 'BOOK' }],
      },
    }),
    STATESET_TAX_RATES_JSON: '{"CA":"0.10"}',
    STATESET_SHIPPING_METHODS_JSON:
      '[{"id":"ground","label":"Ground","amount":"2.00","countries":["US"]}]',
  });
  const plan = await commerce.subscriptions.createPlan({
    name: 'Studio membership',
    billingInterval: 'monthly',
    price: 20,
    currency: 'USD',
  });
  await commerce.subscriptions.activatePlan(plan.id);
  await importCatalog(commerce, [
    { name: 'Studio', slug: 'studio', sku: 'STUDIO', price: '20', stock: 4 },
    { name: 'Workbook', slug: 'workbook', sku: 'BOOK', price: '5', stock: 4 },
  ]);
  const policy = {
    resource: 'studio',
    membershipPlanId: plan.id,
    roles: ['member'],
    recurring: true,
    refundPolicy: 'full',
  };
  env.STATESET_DIGITAL_PRODUCTS_JSON = JSON.stringify({ STUDIO: policy });
  await saveMerchantRecord(
    commerce,
    'seller-studio',
    { seller: 'studio', account: 'acct_studio' },
    'sellers',
  );
  let session, sub, charge, refund;
  const transfers = new Map(),
    reversals = [];
  const invoices = new Map();
  let portalParams;
  const intent = {
    id: 'pi_first',
    status: 'succeeded',
    currency: 'usd',
    customer: 'cus_buyer',
    payment_method: 'pm_studio',
    latest_charge: 'ch_studio',
    amount_received: 2400,
  };
  const renewalIntent = {
    ...intent,
    id: 'pi_renewal',
    livemode: false,
    amount_received: 2200,
    latest_charge: 'ch_later',
  };
  const refunds = new Map();
  const stripe = {
    accounts: {
      retrieve: async () => ({ capabilities: { transfers: 'active' }, payouts_enabled: true }),
    },
    transfers: {
      list: async function* (params) {
        for (const transfer of transfers.values())
          if (transfer.transfer_group === params.transfer_group) yield transfer;
      },
      retrieve: async (id) => transfers.get(id),
      create: async (params) => {
        assert.equal(params.destination, 'acct_studio');
        const renewal = params.metadata.invoice_id === 'in_renewal';
        assert.equal(params.source_transaction, renewal ? 'ch_later' : 'ch_studio');
        assert.equal(params.amount, renewal ? 1637 : 1800);
        const id = renewal ? 'tr_renewal' : 'tr_studio';
        assert.ok(!transfers.has(id));
        const transfer = { ...params, id, created: now, amount_reversed: 0 };
        transfers.set(id, transfer);
        return transfer;
      },
      createReversal: async (id, params) => {
        assert.equal(id, 'tr_renewal');
        assert.equal(params.amount, 1637);
        assert.equal(reversals.length, 0);
        transfers.get(id).amount_reversed += params.amount;
        const reversal = { id: 'trr_renewal', ...params, created: now };
        reversals.push(reversal);
        return reversal;
      },
      listReversals: async function* () {
        yield* reversals;
      },
    },
    checkout: {
      sessions: {
        create: async (params) => {
          assert.equal(params.mode, 'subscription');
          assert.equal(params.payment_intent_data, undefined);
          assert.equal(params.line_items[0].price_data.unit_amount, 2200);
          assert.equal(params.line_items[1].price_data.unit_amount, 200);
          assert.deepEqual(params.line_items[0].price_data.recurring, {
            interval: 'month',
            interval_count: 1,
          });
          sub = {
            id: 'sub_studio',
            status: 'active',
            customer: 'cus_buyer',
            currency: 'usd',
            metadata: { ...params.subscription_data.metadata },
          };
          session = {
            ...params,
            id: 'cs_test_studio',
            url: 'https://checkout.example.test',
            status: 'open',
            currency: 'usd',
            amount_total: 2400,
            payment_status: 'unpaid',
            customer: sub.customer,
            subscription: sub.id,
            invoice: 'in_first',
          };
          return session;
        },
        retrieve: async () => session,
        list: async (params) => ({ data: params.subscription === sub.id ? [session] : [] }),
      },
    },
    subscriptions: {
      retrieve: async () => sub,
      update: async (_, params) => {
        Object.assign(sub.metadata, params.metadata);
        return sub;
      },
    },
    invoices: {
      retrieve: async (id) => invoices.get(id),
      list: async function* () {
        yield* invoices.values();
      },
    },
    prices: {
      retrieve: async (id) => ({
        id,
        currency: 'usd',
        unit_amount: 2200,
        recurring: { interval: 'month', interval_count: 1 },
      }),
    },
    invoicePayments: {
      list: async (params) => {
        const invoice =
          params.invoice ||
          (params.payment?.payment_intent === 'pi_renewal' ? 'in_renewal' : 'in_first');
        return {
          has_more: false,
          data: [
            {
              invoice,
              status: 'paid',
              amount_paid: invoice === 'in_first' ? 2400 : 2200,
              currency: 'usd',
              payment: {
                type: 'payment_intent',
                payment_intent: invoice === 'in_first' ? intent : renewalIntent,
              },
            },
          ],
        };
      },
    },
    charges: { retrieve: async () => charge },
    refunds: {
      retrieve: async (id) => refunds.get(id) ?? refund,
      create: async (params) => {
        assert.equal(params.payment_intent, 'pi_renewal');
        assert.equal(params.amount, 2000);
        assert.ok(params.metadata.stateset_refund_id);
        const remote = {
          ...params,
          id: 're_rest',
          currency: 'usd',
          charge: charge.id,
          status: 'succeeded',
        };
        refunds.set(remote.id, remote);
        charge.amount_refunded += remote.amount;
        return remote;
      },
      list: async function* () {
        yield* refunds.values();
      },
    },
    billingPortal: {
      sessions: {
        create: async (params) => {
          portalParams = params;
          return { url: 'https://billing.example.test' };
        },
      },
    },
  };
  const item = await commerce.products.getVariantBySku('STUDIO');
  const cart = await commerce.carts.create({ currency: 'USD' });
  await commerce.carts.addItemExact(cart.id, {
    productId: item.productId,
    sku: 'STUDIO',
    name: 'Studio',
    quantity: 1,
    unitPrice: '20',
  });
  const input = {
    cartId: cart.id,
    email: 'new@example.test',
    shippingMethodId: 'ground',
    shippingAddress: {
      firstName: 'New',
      lastName: 'Member',
      line1: '1 Main',
      city: 'Oakland',
      state: 'CA',
      postalCode: '94601',
      country: 'US',
    },
  };
  const { assignment } = await enterCampaign(commerce, env, { campaignId: 'launch' });
  const checkout = await startStripeCheckout({
    commerce,
    env,
    stripe,
    origin,
    campaign: assignment,
    input: { ...input, saveCardForOffer: true },
  });
  const invoice = (id, reason, start, stop, amount) => ({
    id,
    status: 'paid',
    currency: 'usd',
    customer: sub.customer,
    billing_reason: reason,
    amount_paid: amount,
    amount_remaining: 0,
    parent: { subscription_details: { subscription: sub.id, metadata: sub.metadata } },
    lines: {
      data: [
        {
          id: 'il_' + id,
          quantity: 1,
          pricing: { price_details: { price: 'price_studio' } },
          amount: 2200,
          currency: 'usd',
          period: { start, end: stop },
          parent: { subscription_item_details: { subscription: sub.id, proration: false } },
        },
      ],
    },
  });
  invoices.set('in_first', invoice('in_first', 'subscription_create', now, end, 2400));
  assert.equal(
    (await reconcileStripeCheckout(commerce, stripe, checkout.sessionId)).state,
    'pending',
  );
  assert.equal((await commerce.subscriptions.list()).length, 0);
  Object.assign(session, { status: 'complete', payment_status: 'paid' });
  intent.amount_received = 2399;
  await assert.rejects(
    reconcileStripeCheckout(commerce, stripe, checkout.sessionId),
    /invoice does not match/,
  );
  assert.equal((await commerce.payments.list()).length, 0);
  intent.amount_received = 2400;
  const settled = await reconcileStripeCheckout(commerce, stripe, checkout.sessionId);
  assert.ok(settled.renewingMembership);
  assert.equal(settled.upsell.chargeTotal, '7.5');
  assert.equal(settled.savedCard, undefined);
  await startCheckoutUpsell(
    commerce,
    stripe,
    origin,
    checkout.sessionId,
    assignment.visitor,
    'accept',
    '7.50',
    async ({ input }, saved) => {
      assert.equal(input.expectedTotal, '7.50');
      assert.deepEqual(saved, { customer: 'cus_buyer', method: 'pm_studio' });
      return { url: '/checkout/card-payment#test' };
    },
  );
  const [native] = await commerce.subscriptions.list();
  assert.equal(native.id, sub.metadata.stateset_subscription_id);
  assert.ok(!native.nextBillingDate);
  assert.equal(native.purchaseOrderId, settled.orderId);
  const [license] = await licensesForOrder(commerce, env, settled.orderId);
  const access = (timestamp) =>
    checkDigitalAccess(commerce, env, license.licenseKey, 'studio', timestamp);
  assert.deepEqual((await access(Date.now() + 1000)).roles, ['member']);
  assert.equal(license.expires, end * 1000);
  assert.equal((await access(end * 1000)).allowed, false);
  await commerce.close();
  commerce = new Commerce(db);
  await Promise.all(
    Array.from({ length: 3 }, () => reconcileStripeCheckout(commerce, stripe, session.id)),
  );
  assert.equal((await commerce.subscriptions.list()).length, 1);
  assert.equal(
    (await commerce.subscriptions.listBillingCycles({ subscriptionId: native.id })).length,
    1,
  );
  const [paidCycle] = await commerce.subscriptions.listBillingCycles({ subscriptionId: native.id });
  assert.equal(paidCycle.status, 'paid');
  assert.equal(paidCycle.paymentId, settled.paymentId);
  const ledger = commerce.generalLedger;
  const period = await ledger.createPeriod({
    periodName: 'Membership signup',
    fiscalYear: new Date().getFullYear(),
    periodNumber: 1,
    startDate: '2020-01-01',
    endDate: '2099-12-31',
  });
  await ledger.openPeriod(period.id);
  const accounts = {};
  for (const [key, accountNumber, accountType] of [
    ['sellerPayable', '2041', 'liability'],
    ['salesClearing', '4041', 'revenue'],
    ['platformFees', '4042', 'revenue'],
    ['processorClearing', '1041', 'asset'],
  ]) {
    accounts[key] = (await ledger.createAccount({ accountNumber, accountType, name: key })).id;
  }
  env.STATESET_SELLER_GL_JSON = JSON.stringify(accounts);
  for (let attempt = 0; attempt < 2; attempt++) {
    const [payout] = await settleSellers(commerce, stripe, env, true);
    assert.equal(payout.state, 'reconciled', payout.error);
    assert.equal(payout.transferred, '18');
  }
  const receipt = await membershipManagement(commerce, stripe, session.id, origin);
  assert.equal(receipt.url, 'https://billing.example.test');
  assert.equal(portalParams.flow_data.subscription_cancel.subscription, sub.id);
  assert.equal(portalParams.customer, 'cus_buyer');
  await assert.rejects(
    membershipManagement(commerce, stripe, 'cs_test_another', origin),
    /invoice does not match/,
  );
  invoices.set('in_renewal', invoice('in_renewal', 'subscription_cycle', end, next, 2200));
  const renewal = invoices.get('in_renewal');
  Object.assign(renewal, {
    object: 'invoice',
    livemode: false,
    collection_method: 'charge_automatically',
  });
  const failure = {
    type: 'invoice.payment_failed',
    id: 'evt_renewal',
    created: end,
    livemode: false,
    data: {
      object: {
        ...renewal,
        status: 'open',
        amount_paid: 0,
        amount_remaining: 2200,
        attempt_count: 1,
        auto_advance: true,
      },
    },
  };
  invoices.set(renewal.id, failure.data.object);
  stripe.events = { retrieve: async () => failure };
  const verifyRecovery = () => verifyStripeRecovery(commerce, stripe, failure.id);
  assert.equal((await verifyRecovery()).state, 'missing_failure_webhook');
  await recordRenewalFailure(commerce, failure, stripe);
  assert.equal((await verifyRecovery()).state, 'awaiting_payment');
  stripe.invoices.pay = async (invoiceId, _, options) => {
    assert.equal(invoiceId, renewal.id);
    assert.equal(options.idempotencyKey, 'recovery-retry-in_renewal-0');
    invoices.set(renewal.id, renewal);
    return renewal;
  };
  const retried = await runRenewalRecovery({
    commerce,
    stripe,
    env: { STATESET_RECOVERY_RETRY_DAYS: '[0]' },
    apply: true,
    now: end * 1000,
  });
  assert.deepEqual(retried.actions, [{ invoice: renewal.id, action: 'retry', stage: '0' }]);
  renewal.lines.data[0].pricing.price_details.price = 'price_another';
  await recordPaidRenewal(commerce, stripe, 'in_renewal');
  assert.equal((await access((end + 1) * 1000)).allowed, false);
  assert.deepEqual(await renewalStatement(commerce), []);
  renewal.lines.data[0].pricing.price_details.price = 'price_studio';
  renewalIntent.amount_received = 2199;
  await assert.rejects(recordPaidRenewal(commerce, stripe, renewal.id), /does not match/);
  renewalIntent.amount_received = 2200;
  assert.deepEqual(await renewalStatement(commerce), []);
  assert.equal((await reconcileRenewals(commerce, stripe, native.id)).invoices[0].state, 'preview');
  assert.equal((await commerce.payments.list()).length, 1);
  sub.cancel_at = next - 3600;
  await recordMembershipStatus(
    commerce,
    stripe,
    { id: 'evt_cancel', data: { object: sub } },
    Date.now() + 60000,
  );
  charge = {
    id: 'ch_later',
    livemode: false,
    status: 'succeeded',
    paid: true,
    captured: true,
    disputed: false,
    payment_intent: 'pi_renewal',
    customer: sub.customer,
    currency: 'usd',
    amount_captured: 2200,
    amount_refunded: 200,
  };
  assert.equal((await verifyRecovery()).state, 'missing_native_renewal');
  refund = {
    id: 're_later',
    payment_intent: 'pi_renewal',
    charge: charge.id,
    currency: 'usd',
    amount: 200,
    status: 'succeeded',
  };
  refunds.set(refund.id, { ...refund });
  // A refund can arrive before invoice.paid and the webhook can be redelivered.
  await reconcileStripeRefund(commerce, stripe, refund.id);
  await Promise.all([
    reconcileStripeRefund(commerce, stripe, refund.id),
    recordPaidRenewal(commerce, stripe, renewal.id),
    recordPaidRenewal(commerce, stripe, renewal.id),
  ]);
  const [sale] = await renewalStatement(commerce);
  const proof = await verifyRecovery();
  assert.equal(proof.state, 'recovered');
  assert.equal(proof.providerConfirmed, true);
  assert.deepEqual(
    [proof.recoveredMinor, proof.refundedMinor, proof.netRetainedMinor],
    ['2200', '200', '2000'],
  );
  assert.deepEqual([proof.cycleId, proof.paymentId], [sale.cycle, sale.payment]);
  assert.deepEqual(await verifyRecovery(), proof);
  charge.amount_captured -= 1;
  await assert.rejects(verifyRecovery, /captured charge/);
  charge.amount_captured += 1;
  failure.livemode = true;
  await assert.rejects(verifyRecovery, /sandbox payment failure/);
  failure.livemode = false;
  charge.amount_refunded += 1;
  await assert.rejects(verifyRecovery, /paid membership cycle/);
  charge.amount_refunded -= 1;
  assert.equal(await membershipExpiry(commerce, native, (end + 1) * 1000, end * 1000, 'any'), 0);
  // Reconfiguring the catalog after purchase cannot change the agreed split.
  const sellerConfig = env.STATESET_SELLER_SKUS_JSON;
  env.STATESET_SELLER_SKUS_JSON = '{}';
  try {
    for (let attempt = 0; attempt < 2; attempt++) {
      const payout = (await settleSellers(commerce, stripe, env, true)).find(
        (row) => row.invoiceId === renewal.id,
      );
      assert.equal(payout?.state, 'reconciled', payout?.error);
      assert.deepEqual([payout.originalPayable, payout.transferred], ['18', '16.37']);
    }
  } finally {
    env.STATESET_SELLER_SKUS_JSON = sellerConfig;
  }
  assert.deepEqual(
    [sale.collected, sale.tax, sale.refunded, sale.netCollected],
    ['22', '2', '2', '20'],
  );
  assert.equal(sale.subscription, native.id);
  const posting = await commerce.payments.get(sale.payment);
  assert.equal(posting.customerId, native.customerId);
  assert.ok(!posting.orderId);
  const cycle = (
    await commerce.subscriptions.listBillingCycles({ subscriptionId: native.id })
  ).find((c) => c.id === sale.cycle);
  assert.equal(cycle.paymentId, posting.id);
  assert.deepEqual(
    [cycle.status, cycle.cycleNumber, cycle.subtotalExact, cycle.taxExact, cycle.totalExact],
    ['paid', 2, '20', '2', '22'],
  );
  assert.equal(Date.parse(cycle.periodStart), end * 1000);
  assert.equal(Date.parse(cycle.periodEnd), next * 1000);
  assert.equal((await commerce.subscriptions.get(native.id)).billingCycleCount, 2);
  assert.equal((await commerce.payments.getRefunds(sale.payment)).length, 1);
  assert.equal((await access((next - 3599) * 1000)).allowed, false);
  assert.ok((await access((next - 3601) * 1000)).allowed);
  assert.ok((await access((end + 1) * 1000)).allowed);
  const request = {
    paymentId: sale.payment,
    amount: '20.00',
    reason: 'Member request',
    idempotencyKey: 'membership-balance',
  };
  const sendRefund = stripe.refunds.create;
  stripe.refunds.create = async (...args) => {
    const remote = await sendRefund(...args);
    // A separate operator can post the native refund before this webhook arrives.
    await commerce.payments.completeRefund(remote.metadata.stateset_refund_id);
    assert.equal(await readMerchantRecord(commerce, 'period-revoked-in_renewal'), null);
    assert.equal((await access((end + 1) * 1000)).allowed, false);
    assert.ok((await access(Date.now() + 1000)).allowed);
    return remote;
  };
  const manual = await refundStripePayment(commerce, stripe, request);
  assert.equal(manual.status, 'completed');
  assert.equal((await reconcileStripeRefund(commerce, stripe, 're_rest')).id, manual.id);
  assert.equal((await refundStripePayment(commerce, stripe, request)).id, manual.id);
  await commerce.close();
  commerce = new Commerce(db);
  assert.equal(
    (await reconcileRenewals(commerce, stripe, native.id, true)).invoices[0].payment,
    sale.payment,
  );
  const [replayed] = await renewalStatement(commerce);
  for (let attempt = 0; attempt < 2; attempt++) {
    const payout = (await settleSellers(commerce, stripe, env, true)).find(
      (row) => row.invoiceId === renewal.id,
    );
    assert.equal(payout?.state, 'reconciled', payout?.error);
    assert.equal(payout.transferred, '0');
  }
  assert.equal(reversals.length, 1);
  assert.equal(transfers.size, 2);
  assert.deepEqual(
    (await portalReport(commerce, env, { role: 'merchant', subject: 'studio' })).renewalReceipts,
    [replayed],
  );
  assert.equal(
    (await portalReport(commerce, env, { role: 'seller', subject: 'studio' })).renewalReceipts,
    undefined,
  );
  assert.deepEqual(
    [replayed.status, replayed.refunded, replayed.netCollected],
    ['refunded', '22', '0'],
  );
  const history = await commerce.subscriptions.listBillingCycles({ subscriptionId: native.id });
  assert.equal(history.length, 2);
  assert.equal(history.find((c) => c.id === sale.cycle).status, 'refunded');
  assert.equal((await verifyRecovery()).netRetainedMinor, '0');
  assert.equal((await commerce.payments.getRefunds(sale.payment)).length, 2);
  assert.equal((await access((end + 1) * 1000)).allowed, false);
  assert.ok((await access(Date.now() + 1000)).allowed);
  assert.equal(
    (await readMerchantRecord(commerce, 'period-revoked-in_renewal')).providerId,
    sub.id,
  );
  const quote = {
    totals: {
      rate: '0.10',
      total: '18.50',
      lines: [{ sku: 'STUDIO', quantity: 1, unitPrice: '20', discount: '5', tax: '1.50' }],
    },
  };
  const priced = await quoteMembership(commerce, { STUDIO: policy }, quote, 'Stripe');
  let coupon;
  const params = await subscriptionCheckoutParameters(
    {
      coupons: {
        create: async (p) => {
          coupon = p;
          return { id: 'first' };
        },
      },
    },
    priced,
    { id: settled.orderId, orderNumber: 'test', customerId: native.customerId },
    {},
  );
  assert.equal(params.line_items[0].price_data.unit_amount, 2200);
  assert.equal(params.line_items[1].price_data.unit_amount, 200);
  assert.equal(coupon.amount_off, 550);
  assert.equal(coupon.duration, 'once');
  await assert.rejects(
    quoteMembership(commerce, { STUDIO: policy }, quote, 'Routed'),
    /Stripe Checkout/,
  );

  env.STATESET_DIGITAL_PRODUCTS_JSON = JSON.stringify({ STUDIO: { ...policy, recurring: false } });
  const prepaidCart = await commerce.carts.create({ currency: 'USD' });
  await commerce.carts.addItemExact(prepaidCart.id, {
    productId: item.productId,
    sku: 'STUDIO',
    name: 'Studio',
    quantity: 1,
    unitPrice: '20',
  });
  let prepaid;
  const ordinaryStripe = {
    checkout: {
      sessions: {
        create: async (params) => {
          assert.equal(params.mode, 'payment');
          assert.equal(params.payment_intent_data.setup_future_usage, undefined);
          assert.equal(params.subscription_data, undefined);
          prepaid = {
            ...params,
            id: 'cs_test_prepaid',
            url: 'https://checkout.example.test',
            currency: 'usd',
            amount_total: 2400,
            status: 'open',
            customer: 'cus_prepaid',
            payment_status: 'unpaid',
            payment_intent: {
              id: 'pi_prepaid',
              customer: 'cus_prepaid',
              payment_method: 'pm_prepaid',
              amount_received: 2400,
              currency: 'usd',
              status: 'succeeded',
              metadata: params.metadata,
            },
          };
          return prepaid;
        },
        retrieve: async () => prepaid,
        list: async ({ payment_intent }) => ({
          data: payment_intent === 'pi_prepaid' ? [prepaid] : [],
        }),
      },
    },
    refunds: {
      retrieve: async () => ({
        id: 're_prepaid',
        currency: 'usd',
        amount: 2400,
        payment_intent: 'pi_prepaid',
        status: 'succeeded',
      }),
    },
  };
  const prepaidCheckout = await startStripeCheckout({
    commerce,
    env,
    stripe: ordinaryStripe,
    origin,
    campaign: assignment,
    input: { ...input, cartId: prepaidCart.id, email: 'prepaid@example.test' },
  });
  Object.assign(prepaid, { status: 'complete', payment_status: 'paid' });
  const prepaidReceipt = await reconcileStripeCheckout(
    commerce,
    ordinaryStripe,
    prepaidCheckout.sessionId,
  );
  assert.equal(prepaidReceipt.renewingMembership, undefined);
  assert.equal(prepaidReceipt.upsell.chargeTotal, undefined);
  const prepaidSubs = await commerce.subscriptions.list();
  assert.equal(prepaidSubs.length, 2);
  const prepaidSub = prepaidSubs.find((row) => row.purchaseOrderId === prepaidReceipt.orderId);
  assert.ok(
    prepaidSub && !prepaidSub.nextBillingDate && prepaidSub.customerId !== native.customerId,
  );
  const [prepaidCycle] = await commerce.subscriptions.listBillingCycles({
    subscriptionId: prepaidSub.id,
  });
  assert.equal(prepaidCycle.paymentId, prepaidReceipt.paymentId);
  assert.equal(prepaidCycle.status, 'paid');
  const [prepaidLicense] = await licensesForOrder(commerce, env, prepaidReceipt.orderId);
  assert.ok(prepaidLicense.active);
  await reconcileStripeRefund(commerce, ordinaryStripe, 're_prepaid');
  await reconcileStripeRefund(commerce, ordinaryStripe, 're_prepaid');
  const prepaidHistory = await commerce.subscriptions.listBillingCycles({
    subscriptionId: prepaidSub.id,
  });
  assert.deepEqual(
    prepaidHistory.map((cycle) => [cycle.id, cycle.status]),
    [[prepaidCycle.id, 'refunded']],
  );
  assert.equal((await licensesForOrder(commerce, env, prepaidReceipt.orderId))[0].active, false);
  assert.ok((await access(Date.now() + 1000)).allowed);
  console.log(
    JSON.stringify({
      result: 'passed',
      externalProvidersVerified: false,
      checks: [
        'new buyer enrollment from paid initial invoice',
        'exact Stripe payments and one-time coupon',
        'one native membership after reopen and replay',
        'provider period expiry and paid renewal',
        'failed renewal recovery matched to sandbox-shaped event, captured charge, payment, cycle and later refunds',
        'native renewal billing cycle, tax, payment and refund statement with historical replay',
        'buyer cancellation and renewal refund policy',
        'optional offer uses the verified invoice card only with buyer consent',
        'single seller transfer from the signup invoice charge',
        'renewal seller split, partial refund and full transfer reversal on the renewal charge',
        'prepaid buyer enrollment, no automatic rebill, and full refund replay',
      ],
    }),
  );
} finally {
  await commerce.close();
  rmSync(dir, { recursive: true, force: true });
}
