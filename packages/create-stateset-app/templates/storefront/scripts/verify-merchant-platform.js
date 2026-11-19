// Dispose of the native store on exit. All provider operations use the local fixture.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Commerce } from '@stateset/embedded';
import {
  enterCampaign,
  registerAffiliate,
  campaignAccounting,
  payoutExport,
  affiliateBalances,
} from '../lib/campaigns.js';
import { checkDigitalAccess, licensesForOrder, linkMembership } from '../lib/digital-access.js';
import { importCatalog, runLaunchDrill, shipTestPurchase } from '../lib/merchant-onboarding.js';
import {
  reconcileStripeCheckout,
  startCheckoutUpsell,
  startStripeCheckout,
  refundStripePayment,
} from '../lib/stripe-checkout.js';
import { portalPrincipal } from '../lib/merchant-portal.js';
import { readMerchantRecord, saveMerchantRecord } from '../lib/merchant-records.js';
import {
  sellerStatement,
  sellerBalances,
  settleSellers,
  sellerBankPayouts,
} from '../lib/seller-payouts.js';

const directory = mkdtempSync(join(tmpdir(), 'stateset-platform-check-'));
const commerce = new Commerce(join(directory, 'fixture.db'));
Object.assign(process.env, {
  STATESET_LICENSE_SECRET: 'digital'.repeat(8),
  STATESET_CAMPAIGN_SECRET: 'campaign'.repeat(8),
  STATESET_CAMPAIGNS_JSON: JSON.stringify({
    launch: {
      variants: [
        { id: 'a', weight: 1, title: 'Coffee', sku: 'COFFEE', bumpSku: 'GUIDE', upsellSku: 'CUP' },
      ],
    },
  }),
  STATESET_TAX_RATES_JSON: '{"CA":"0"}',
  STATESET_SHIPPING_METHODS_JSON:
    '[{"id":"ground","label":"Ground","amount":"0","countries":["US"]}]',
  STATESET_SELLER_SKUS_JSON: '{"COFFEE":{"seller":"roaster","feeBps":1000}}',
  STATESET_DIGITAL_PRODUCTS_JSON:
    '{"GUIDE":{"resource":"brew-guide","roles":["barista"],"validDays":14}}',
});
const origin = 'https://store.example.test';
const address = {
  firstName: 'A',
  lastName: 'B',
  line1: '1 Main',
  city: 'Oakland',
  state: 'CA',
  country: 'US',
  postalCode: '94601',
};
const sessions = new Map(),
  refunds = [],
  transfers = [],
  reversals = [];
const stripe = {
  accounts: {
    retrieve: async () => ({ capabilities: { transfers: 'active' }, payouts_enabled: true }),
  },
  checkout: {
    sessions: {
      create: async (params, options) => {
        const prior = [...sessions.values()].find((s) => s.key === options.idempotencyKey);
        if (prior) return prior;
        const id = 'cs_test_order' + sessions.size;
        const session = {
          ...params,
          id,
          key: options.idempotencyKey,
          url: 'https://checkout.stripe.com/example',
          status: 'open',
          currency: 'usd',
          amount_total: params.line_items[0].price_data.unit_amount,
        };
        sessions.set(id, session);
        return session;
      },
      retrieve: async (id) => sessions.get(id),
    },
  },
  refunds: {
    list: async function* () {
      yield* refunds;
    },
    retrieve: async (id) => refunds.find((r) => r.id === id),
    create: async (input) => {
      const refund = {
        ...input,
        id: 're_fixture' + refunds.length,
        status: 'succeeded',
        currency: 'usd',
      };
      refunds.push(refund);
      return refund;
    },
  },
  transfers: {
    list: async function* () {
      yield* transfers;
    },
    create: async (input) => {
      const record = {
        ...input,
        id: 'tr_fixture',
        destination_payment: 'py_destination',
        amount_reversed: 0,
        created: Math.floor(Date.now() / 1000),
      };
      transfers.push(record);
      return record;
    },
    retrieve: async (id) => transfers.find((t) => t.id === id),
    createReversal: async (id, input) => {
      const t = transfers.find((t) => t.id === id);
      t.amount_reversed += input.amount;
      reversals.push({
        ...input,
        id: 'rv_fixture',
        destination_payment_refund: 'pyr_destination',
        created: t.created,
      });
    },
    listReversals: async function* () {
      yield* reversals;
    },
  },
};
function completeSession(sessionId) {
  const s = sessions.get(sessionId);
  s.status = 'complete';
  s.payment_status = 'paid';
  s.payment_intent = {
    id: 'pi_' + s.id.replaceAll('_', ''),
    currency: 'usd',
    amount_received: s.amount_total,
    status: 'succeeded',
    metadata: s.metadata,
    latest_charge: 'ch_fixture',
  };
  return s;
}
async function pay(sessionId) {
  completeSession(sessionId);
  return reconcileStripeCheckout(commerce, stripe, sessionId);
}

async function checkWalletDrills() {
  const env = {
    ...process.env,
    STATESET_CHECKOUT_PROVIDER: 'stripe',
    STRIPE_SECRET_KEY: 'sk_test_fixture',
    STRIPE_WEBHOOK_SECRET: 'whsec_fixture',
    NEXT_PUBLIC_BASE_URL: origin,
  };
  const run = (values) => runLaunchDrill(commerce, stripe, env, values);
  for (const wallet of ['apple_pay', 'google_pay']) {
    const key = 'wallet-' + wallet;
    const otherWallet = wallet === 'apple_pay' ? 'google_pay' : 'apple_pay';
    const purchase = {
      key,
      purchase: 'CUP',
      email: 'buyer@example.test',
      address,
      shipping: 'ground',
      wallet,
      apply: true,
    };
    const { testPurchase } = await run(purchase);
    assert.equal(testPurchase.expectedWallet, wallet);
    await assert.rejects(run({ ...purchase, wallet: otherWallet }), /different wallet/);
    const finish = { key, finish: testPurchase.sessionId };
    const s = completeSession(finish.finish);
    s.livemode = false;
    s.payment_intent.livemode = false;
    const chargeId = 'ch_' + s.payment_intent.id.slice(3);
    s.payment_intent.latest_charge = chargeId;
    const charge = {
      id: chargeId,
      payment_intent: s.payment_intent.id,
      status: 'succeeded',
      livemode: false,
      paid: true,
      captured: true,
      amount_captured: s.amount_total,
      currency: 'usd',
      payment_method_details: { type: 'card', card: { wallet: null } },
    };
    stripe.charges = {
      retrieve: async (id) => {
        assert.equal(id, chargeId);
        return charge;
      },
    };
    const refundCount = refunds.length;
    await assert.rejects(run({ ...finish, apply: true }), /used card/);
    assert.equal(refunds.length, refundCount);
    charge.payment_method_details.card.wallet = { type: otherWallet };
    await assert.rejects(run(finish), new RegExp('used ' + otherWallet));
    charge.payment_method_details.card.wallet.type = wallet;
    for (const override of [
      { livemode: true },
      { captured: false },
      { payment_intent: 'pi_other' },
      { amount_captured: 1 },
    ]) {
      const original = { ...charge };
      Object.assign(charge, override);
      await assert.rejects(run(finish), /does not match/);
      Object.assign(charge, original);
    }
    const preview = await run(finish);
    assert.equal(preview.wallet.method, wallet);
    assert.equal(preview.wallet.chargeId, chargeId);
    assert.equal(refunds.length, refundCount);
    assert.equal(await readMerchantRecord(commerce, 'test-purchase-' + key + '-complete'), null);
    const applied = await run({ ...finish, apply: true });
    assert.equal(applied.settlementVerified, true);
    await run({ ...finish, apply: true });
    assert.equal(refunds.length, refundCount + 1);
    const completed = await readMerchantRecord(commerce, 'test-purchase-' + key + '-complete');
    assert.deepEqual(completed.wallet, preview.wallet);
    await assert.rejects(run({ ...finish, wallet: otherWallet }), /different wallet/);
  }
}
try {
  const setupDb = join(directory, 'new-store.db');
  const setup = fileURLToPath(new URL('./setup.js', import.meta.url));
  const setupEnv = {
    ...process.env,
    STATESET_DB_PATH: setupDb,
    NEXT_PUBLIC_BASE_URL: 'http://localhost:3000',
  };
  const startSetup = (args = []) =>
    execFileSync(process.execPath, [setup, ...args], {
      cwd: directory,
      env: setupEnv,
      encoding: 'utf8',
    });
  assert.match(startSetup(), /--apply/);
  assert.equal(existsSync(setupDb), false);
  const invitation = startSetup(['--apply'])
    .split('\n')
    .find((line) => line.startsWith('http'));
  const newStore = new Commerce(setupDb);
  try {
    const token = new URL(invitation).hash.slice(1);
    const grant = await portalPrincipal(newStore, token);
    assert.equal(grant?.role, 'merchant');
    assert.equal(grant.subject, 'shop');
    assert.equal(await portalPrincipal(newStore, token, Date.now() + 86400000), null);
    assert.equal((await newStore.products.list()).length, 0);
  } finally {
    await newStore.close();
  }
  const plan = await commerce.subscriptions.createPlan({
    name: 'Coffee club',
    billingInterval: 'monthly',
    price: 15,
  });
  await commerce.subscriptions.activatePlan(plan.id);
  process.env.STATESET_DIGITAL_PRODUCTS_JSON = JSON.stringify({
    ...JSON.parse(process.env.STATESET_DIGITAL_PRODUCTS_JSON),
    COFFEE: { resource: 'coffee-club', membershipPlanId: plan.id, roles: ['member'] },
  });
  const catalog = [
    ['COFFEE', '15.00'],
    ['GUIDE', '5.00'],
    ['CUP', '3.00'],
  ].map(([sku, price]) => ({
    name: sku,
    slug: sku.toLowerCase(),
    sku,
    price,
    stock: 4,
  }));
  assert.equal((await importCatalog(commerce, catalog)).inserted, 3);
  assert.equal((await importCatalog(commerce, catalog)).inserted, 0);
  await saveMerchantRecord(
    commerce,
    'seller-roaster',
    { seller: 'roaster', account: 'acct_fixture' },
    'sellers',
  );
  await registerAffiliate(commerce, 'partner', 1000, true);
  const emptyBatch = await payoutExport(commerce, 'before-first-sale', true);
  const { assignment } = await enterCampaign(commerce, process.env, {
    campaignId: 'launch',
    source: 'email',
    affiliate: 'partner',
  });
  const cart = await commerce.carts.create({ currency: 'USD' });
  const base = await commerce.products.getVariantBySku('COFFEE');
  await commerce.carts.addItemExact(cart.id, {
    productId: base.productId,
    sku: base.sku,
    name: 'Coffee',
    quantity: 1,
    unitPrice: base.priceExact,
  });
  const purchase = await startStripeCheckout({
    commerce,
    stripe,
    origin,
    campaign: assignment,
    input: {
      cartId: cart.id,
      email: 'buyer@example.test',
      shippingAddress: address,
      shippingMethodId: 'ground',
      bump: true,
    },
  });
  process.env.STATESET_DIGITAL_PRODUCTS_JSON = '{"GUIDE":{"resource":"later-guide"}}';
  const reserved = await commerce.orders.get(sessions.get(purchase.sessionId).metadata.order_id);
  const subscription = await commerce.subscriptions.subscribe({
    customerId: reserved.customerId,
    planId: plan.id,
  });
  const result = await pay(purchase.sessionId);
  assert.equal(sessions.get(purchase.sessionId).amount_total, 2000);
  assert.equal(result.upsell.name, 'CUP');
  const licenses = await licensesForOrder(commerce, process.env, result.orderId);
  const grant = licenses.find((l) => l.resource === 'brew-guide');
  const club = licenses.find((l) => l.resource === 'coffee-club');
  assert.equal(club.active, true);
  assert.equal(club.expires, Date.parse(subscription.currentPeriodEnd));
  assert.equal(
    (await linkMembership(commerce, result.orderId, 'coffee-club', subscription.id, true)).mode,
    'linked',
  );
  const outsider = await commerce.customers.create({
    email: 'another@example.test',
    firstName: 'Another',
    lastName: 'Buyer',
  });
  const otherMembership = await commerce.subscriptions.subscribe({
    customerId: outsider.id,
    planId: plan.id,
  });
  await assert.rejects(
    linkMembership(commerce, result.orderId, 'coffee-club', otherMembership.id, true),
    /belong/,
  );
  const [cycle] = await commerce.subscriptions.listBillingCycles({
    subscriptionId: subscription.id,
  });
  await commerce.subscriptions.markCyclePaid(cycle.id);
  const access = (now) =>
    checkDigitalAccess(commerce, process.env, club.licenseKey, club.resource, now);
  assert.equal((await access(Date.now())).allowed, true);
  const renewed = await access(club.expires + 1000);
  assert.ok(renewed.allowed && renewed.expires > club.expires);
  await commerce.subscriptions.cancel(subscription.id, { immediate: false });
  assert.equal((await access(renewed.expires - 1000)).allowed, true);
  assert.equal((await access(renewed.expires)).allowed, false);
  assert.deepEqual(
    (await checkDigitalAccess(commerce, process.env, grant.licenseKey, 'brew-guide')).roles,
    ['barista'],
  );
  assert.equal(
    (
      await checkDigitalAccess(
        commerce,
        process.env,
        grant.licenseKey,
        'brew-guide',
        grant.expires + 1,
      )
    ).allowed,
    false,
  );
  await assert.rejects(
    startCheckoutUpsell(commerce, stripe, origin, purchase.sessionId, 'wrong-visitor'),
  );
  const extra = await startCheckoutUpsell(
    commerce,
    stripe,
    origin,
    purchase.sessionId,
    assignment.visitor,
  );
  assert.equal(
    (await startCheckoutUpsell(commerce, stripe, origin, purchase.sessionId, assignment.visitor))
      .sessionId,
    extra.sessionId,
  );
  await pay(extra.sessionId);
  assert.equal((await commerce.orders.list()).length, 2);
  const shipment = await shipTestPurchase(commerce, result.orderId);
  assert.deepEqual(await shipTestPurchase(commerce, result.orderId), shipment);
  assert.equal((await commerce.shipments.list({ orderId: result.orderId })).length, 1);
  assert.equal((await campaignAccounting(commerce)).variants[0].netRevenuePerVisitor, '23');
  assert.ok(
    (await payoutExport(commerce, 'batch-one', true)).includes('batch-one,partner,USD,2.3'),
  );
  assert.equal((await payoutExport(commerce, 'duplicate-batch', true)).split('\r\n').length, 1);
  assert.equal(await payoutExport(commerce, 'before-first-sale', true), emptyBatch);

  const gl = commerce.generalLedger;
  const period = await gl.createPeriod({
    periodName: 'fixture',
    fiscalYear: new Date().getFullYear(),
    periodNumber: 1,
    startDate: '2020-01-01',
    endDate: '2099-12-31',
  });
  await gl.openPeriod(period.id);
  const accounts = {};
  for (const [key, accountNumber, accountType] of [
    ['sellerPayable', '2041', 'liability'],
    ['salesClearing', '4041', 'revenue'],
    ['platformFees', '4042', 'revenue'],
    ['processorClearing', '1041', 'asset'],
  ]) {
    accounts[key] = (await gl.createAccount({ accountNumber, accountType, name: key })).id;
  }
  process.env.STATESET_SELLER_GL_JSON = JSON.stringify(accounts);
  const [first] = await settleSellers(commerce, stripe, process.env, true);
  assert.equal(first.state, 'reconciled', first.error);
  assert.equal(first.transferred, '13.5');
  await refundStripePayment(commerce, stripe, {
    paymentId: result.paymentId,
    amount: '4',
    reason: 'Partial return',
    idempotencyKey: 'platform-test-refund',
  });
  assert.equal(
    (await checkDigitalAccess(commerce, process.env, grant.licenseKey, 'brew-guide')).allowed,
    false,
  );
  const [statement] = await sellerStatement(commerce);
  assert.equal(statement.payable, '10.8');
  assert.equal(statement.outstanding, '-2.7');
  stripe.accounts.retrieve = async () => ({
    payouts_enabled: false,
    capabilities: { transfers: 'inactive' },
  });
  const [second] = await settleSellers(commerce, stripe, process.env, true);
  assert.equal(second.state, 'reconciled', second.error);
  assert.equal(second.transferred, '10.8');
  await settleSellers(commerce, stripe, process.env, true);
  assert.equal(transfers.length, 1);
  assert.equal(reversals.length, 1);
  let bankStatus = 'pending';
  stripe.payouts = {
    list: async function* (_, options) {
      assert.equal(options.stripeAccount, 'acct_fixture');
      yield {
        id: 'po_fixture',
        amount: 1080,
        currency: 'usd',
        status: bankStatus,
        automatic: true,
        reconciliation_status: bankStatus === 'paid' ? 'completed' : 'in_progress',
        arrival_date: 1800000000,
      };
    },
  };
  stripe.balanceTransactions = {
    list: async function* (criteria, options) {
      assert.equal(criteria.payout, 'po_fixture');
      assert.equal(options.stripeAccount, 'acct_fixture');
      yield {
        id: 'txn_sale',
        source: 'py_destination',
        currency: 'usd',
        type: 'payment',
        amount: 1350,
        fee: 0,
        net: 1350,
      };
      yield {
        id: 'txn_refund',
        source: 'pyr_destination',
        currency: 'usd',
        type: 'payment_refund',
        amount: -270,
        fee: 0,
        net: -270,
      };
    },
  };
  await sellerBankPayouts(commerce, stripe, 'roaster', true);
  bankStatus = 'paid';
  await sellerBankPayouts(commerce, stripe, 'roaster', true);
  const [balance] = await sellerBalances(commerce, 'roaster');
  assert.equal(balance.outstanding, '0');
  assert.equal(balance.transferred, '10.8');
  assert.deepEqual(
    balance.bankPayouts.map((p) => p.status),
    ['paid'],
  );
  const bankSheet = balance.bankPayouts[0].reconciliation;
  assert.equal(bankSheet.differenceMinor, '0');
  assert.deepEqual(
    bankSheet.transactions.map((t) => t.orderId),
    [result.orderId, result.orderId],
  );
  assert.equal((await gl.listJournalEntries({ sourceDocumentType: 'merchant_event' })).length, 4);
  assert.equal((await campaignAccounting(commerce)).variants[0].netRevenuePerVisitor, '19');
  assert.equal((await affiliateBalances(commerce)).balances[0].available, '-0.4');
  const promotion = await commerce.promotions.create({
    name: 'Launch coupon',
    promotionType: 'fixed_amount_off',
    trigger: 'coupon_code',
    fixedAmountOff: 2,
    target: 'order',
  });
  await commerce.promotions.activate(promotion.id);
  await commerce.promotions.createCoupon({
    promotionId: promotion.id,
    code: 'LAUNCH2',
    usageLimit: 1,
  });
  const campaigns = JSON.parse(process.env.STATESET_CAMPAIGNS_JSON);
  campaigns.launch.variants[0].couponCode = 'LAUNCH2';
  process.env.STATESET_CAMPAIGNS_JSON = JSON.stringify(campaigns);
  async function discountedInput() {
    const cart = await commerce.carts.create({ currency: 'USD' });
    await commerce.carts.addItemExact(cart.id, {
      productId: base.productId,
      sku: base.sku,
      name: 'Coffee',
      quantity: 1,
      unitPrice: base.priceExact,
    });
    return {
      commerce,
      stripe,
      origin,
      campaign: assignment,
      input: {
        cartId: cart.id,
        email: 'buyer@example.test',
        shippingAddress: address,
        shippingMethodId: 'ground',
        bump: true,
      },
    };
  }
  const options = await discountedInput();
  const quoted = await startStripeCheckout(options);
  assert.equal(sessions.get(quoted.sessionId).amount_total, 1800);
  assert.equal((await startStripeCheckout(options)).sessionId, quoted.sessionId);
  const discounted = await pay(quoted.sessionId);
  assert.equal((await commerce.orders.get(discounted.orderId)).discountAmountExact, '2');
  assert.equal((await commerce.promotions.listUsage({ orderId: discounted.orderId })).length, 1);
  await assert.rejects(startStripeCheckout(await discountedInput()), /limit|exhausted/i);
  assert.equal((await commerce.orders.list()).length, 3);
  await startCheckoutUpsell(commerce, stripe, origin, quoted.sessionId, assignment.visitor, 'skip');
  const accounting = (await campaignAccounting(commerce)).variants[0];
  assert.equal(accounting.netRevenuePerVisitor, '37');
  assert.equal(accounting.upsellSkips, 1);
  await checkWalletDrills();
  console.log(
    JSON.stringify({
      result: 'passed',
      provider: 'fixture',
      externalProvidersVerified: false,
      checks: [
        'new-store setup preview and expiring private merchant invitation',
        'catalog import and retry',
        'attributed purchase and bump',
        'single upsell checkout',
        'access expiry and refund revocation',
        'commission reserve and refund reversal',
        'seller transfer and reversal',
        'four idempotent native ledger postings',
        'automatic membership linking, owner check, renewal and end-of-period cancellation',
        'one governed test shipment across retries',
        'bank payout history and outstanding seller balance after account restriction',
        'native coupon redemption and usage limit',
        'net revenue after promotions and refunds',
        'upsell decline tracking',
        'Apple Pay and Google Pay charge evidence, read-only verification, wrong-wallet rejection and replay',
      ],
    }),
  );
} finally {
  await commerce.close();
  rmSync(directory, { recursive: true, force: true });
}
