// The real native engine selects hosted fields. Provider authorization and capture
// below use a fixture; the Rust integration suite exercises their HTTP contracts.
import assert from 'node:assert/strict';
import { Commerce } from '@stateset/embedded';
import { verifyMessage } from 'viem';
import { generatePrivateKey, privateKeyToAccount } from 'viem/accounts';
import { openPartnerAccount } from '../lib/partner-accounts.js';
import { partnerMessage } from '../lib/partner-signature.js';
import { publishPricing, pricingEnvironment } from '../lib/store-pricing.js';
import { importCatalog } from '../lib/merchant-onboarding.js';
import {
  startRoutedCheckout,
  routedSession,
  advanceRoutedCheckout,
  recoverRoutedRefunds,
  refundRoutedCheckout,
  routedOffer,
} from '../lib/routed-checkout.js';
import {
  reconcileStripeCheckout,
  startCheckoutUpsell,
  startStripeCheckout,
} from '../lib/stripe-checkout.js';
import {
  activeOffer,
  campaignAccounting,
  enterCampaign,
  publishCampaign,
  registerAffiliate,
} from '../lib/campaigns.js';
import { saveMerchantRecord } from '../lib/merchant-records.js';
import {
  issuePortal,
  revokePortal,
  portalPrincipal,
  portalReport,
} from '../lib/merchant-portal.js';

const engine = new Commerce(':memory:');
const env = process.env;
Object.assign(env, {
  STATESET_TAX_RATES_JSON: '{"CA":"0.1"}',
  STATESET_SHIPPING_METHODS_JSON:
    '[{"id":"ground","label":"Ground","amount":"1","countries":["US"]}]',
  STATESET_CARD_ACCOUNTS_JSON: JSON.stringify([
    {
      account: 'square-shop',
      provider: 'square',
      secretEnv: 'SQUARE_FIXTURE',
      publicKey: 'sandbox-sq0idp-fixture',
      location: 'L1',
      currencies: ['USD'],
    },
  ]),
  SQUARE_FIXTURE: 'test-secret-never-dispatch',
  NEXT_PUBLIC_STRIPE_PUBLISHABLE_KEY: 'pk_test_fixture',
  STRIPE_SECRET_KEY: 'sk_test_fixture',
  STATESET_CAMPAIGN_SECRET: 'test-only'.repeat(5),
  STATESET_CAMPAIGNS_JSON:
    '{"launch":{"variants":[{"id":"a","weight":1,"title":"Coffee","sku":"COFFEE","upsellSku":"MUG"}]}}',
  NEXT_PUBLIC_BASE_URL: 'https://shop.example.test',
});
const address = {
  firstName: 'A',
  lastName: 'B',
  line1: '1 Main',
  city: 'Oakland',
  state: 'CA',
  postalCode: '94601',
  country: 'US',
};
const origin = env.NEXT_PUBLIC_BASE_URL;
async function cartInput() {
  const cart = await engine.carts.create({ currency: 'USD' });
  const product = await engine.products.getVariantBySku('COFFEE');
  await engine.carts.addItemExact(cart.id, {
    sku: 'COFFEE',
    productId: product.productId,
    quantity: 1,
    name: 'Coffee',
    unitPrice: '10',
  });
  return {
    cartId: cart.id,
    email: 'buyer@example.test',
    shippingAddress: address,
    shippingMethodId: 'ground',
  };
}
try {
  await importCatalog(
    engine,
    [
      ['COFFEE', '10'],
      ['MUG', '3'],
    ].map(([sku, price]) => ({ sku, price, slug: sku.toLowerCase(), name: sku, stock: 8 })),
  );
  const normal = await startRoutedCheckout({ commerce: engine, env, input: await cartInput() });
  const checkout = await routedSession(engine, normal.url.split('#')[1]);
  const collect = await advanceRoutedCheckout(engine, env, checkout);
  assert.equal(collect.state, 'collect_token');
  assert.equal(collect.account.provider, 'square');
  assert.equal(collect.amount, '12');
  assert.ok(!JSON.stringify(collect).includes(env.SQUARE_FIXTURE));
  await assert.rejects(routedSession(engine, 'bad-token'), /Unknown/);
  await saveMerchantRecord(
    engine,
    'seller-roaster',
    { seller: 'roaster', account: 'acct_test' },
    'sellers',
  );
  env.STATESET_SELLER_SKUS_JSON = '{"COFFEE":{"seller":"roaster","feeBps":1000}}';
  await assert.rejects(
    startRoutedCheckout({ commerce: engine, env, input: await cartInput() }),
    /available Connect/,
  );
  assert.equal((await engine.orders.list()).length, 1);
  delete env.STATESET_SELLER_SKUS_JSON;

  let hosted;
  const stripe = {
    checkout: {
      sessions: {
        create: async (input) => {
          assert.equal(input.customer_creation, 'always');
          assert.equal(input.payment_intent_data.setup_future_usage, 'on_session');
          hosted = {
            ...input,
            id: 'cs_test_opted',
            status: 'open',
            url: 'https://checkout.stripe.com/fixture',
            currency: 'usd',
            amount_total: input.line_items[0].price_data.unit_amount,
          };
          return hosted;
        },
        retrieve: async () => hosted,
      },
    },
  };
  await registerAffiliate(engine, 'partner', 1000, true);
  const { assignment } = await enterCampaign(engine, env, {
    campaignId: 'launch',
    source: 'direct',
    affiliate: 'partner',
  });
  const primary = await startStripeCheckout({
    commerce: engine,
    stripe,
    origin,
    env,
    campaign: assignment,
    input: { ...(await cartInput()), saveCardForOffer: true },
  });
  Object.assign(hosted, {
    status: 'complete',
    payment_status: 'paid',
    customer: 'cus_buyer',
    payment_intent: {
      id: 'pi_original',
      status: 'succeeded',
      customer: 'cus_buyer',
      payment_method: 'pm_saved',
      amount_received: hosted.amount_total,
      currency: 'usd',
      metadata: hosted.metadata,
    },
  });
  const receipt = await reconcileStripeCheckout(engine, stripe, primary.sessionId);
  assert.equal(receipt.upsell.chargeTotal, '4.3');
  await assert.rejects(
    startCheckoutUpsell(
      engine,
      stripe,
      origin,
      primary.sessionId,
      assignment.visitor,
      'accept',
      '3',
      startRoutedCheckout,
    ),
    /total changed/,
  );
  const accepted = await startCheckoutUpsell(
    engine,
    stripe,
    origin,
    primary.sessionId,
    assignment.visitor,
    'accept',
    '4.30',
    startRoutedCheckout,
  );
  const saved = await routedSession(engine, accepted.url.split('#')[1]);
  let captures = 0;
  const nativePayments = engine.payments;
  const payments = new Proxy(nativePayments, {
    get(target, key) {
      if (key === 'routeCard')
        return async (op) => {
          assert.equal(op.paymentId, saved.paymentId);
          assert.equal(op.accounts[0].customer, 'cus_buyer');
          assert.equal(op.accounts[0].token, 'pm_saved');
          if (op.action === 'authorize')
            return { state: 'authorized', account: 'hosted-stripe', reference: 'pi_extra' };
          assert.equal(op.action, 'capture');
          captures++;
          await nativePayments.markCompleted(op.paymentId);
          return { state: 'captured', reference: 'pi_extra' };
        };
      const value = target[key];
      return typeof value === 'function' ? value.bind(target) : value;
    },
  });
  const shop = {
    orders: engine.orders,
    payments,
    customers: engine.customers,
    customObjects: engine.customObjects,
    carts: engine.carts,
  };
  assert.equal((await advanceRoutedCheckout(shop, env, saved)).state, 'paid');
  assert.equal((await advanceRoutedCheckout(shop, env, saved)).state, 'paid');
  assert.deepEqual(
    await startCheckoutUpsell(
      engine,
      stripe,
      origin,
      primary.sessionId,
      assignment.visitor,
      'accept',
      '4.30',
      startRoutedCheckout,
    ),
    accepted,
  );
  assert.equal(captures, 1);
  assert.equal((await engine.orders.list()).length, 3);
  const link = await issuePortal(engine, origin, 'affiliate', 'partner', 7, true);
  const principal = await portalPrincipal(engine, new URL(link.url).hash.slice(1));
  const report = await portalReport(engine, env, principal);
  assert.equal(report.balance.earned, '1.3');
  assert.ok(report.orders.every((row) => row.affiliate === 'partner'));
  assert.equal(report.referrals.length, 1);
  const launch = {
    id: 'published',
    revision: 0,
    variants: [{ id: 'a', title: 'First release', sku: 'COFFEE', weight: 1, bumpSku: 'MUG' }],
  };
  assert.equal((await publishCampaign(engine, env, launch)).products.length, 2);
  assert.equal(await enterCampaign(engine, env, { campaignId: launch.id }), null);
  await publishCampaign(engine, env, launch, true);
  const visit = await enterCampaign(engine, env, { campaignId: launch.id });
  const revision = {
    ...launch,
    revision: 1,
    variants: [{ ...launch.variants[0], title: 'Second release' }],
  };
  await publishCampaign(engine, env, revision, true);
  assert.equal((await activeOffer(engine, env, visit.assignment)).title, 'First release');
  const fresh = await enterCampaign(engine, env, { campaignId: launch.id });
  assert.equal((await activeOffer(engine, env, fresh.assignment)).title, 'Second release');
  await assert.rejects(publishCampaign(engine, env, launch, true), /changed/);
  const old = await campaignAccounting(engine, {
    from: '2000-01-01',
    to: '2000-01-31',
    includeOrders: true,
  });
  assert.deepEqual(old.orders, []);
  const scoped = await campaignAccounting(engine, {
    from: new Date().toISOString().slice(0, 10),
    includeOrders: true,
  });
  assert.equal(scoped.orders.length, 2);
  const routedPrimary = await startRoutedCheckout({
    commerce: engine,
    env,
    campaign: assignment,
    input: await cartInput(),
  });
  const original = await routedSession(engine, routedPrimary.url.split('#')[1]);
  await assert.rejects(routedOffer(engine, env, original, 'offer'), /No offer/);
  const simulateProcessor = new Proxy(engine.payments, {
    get(target, key) {
      if (key === 'routeCard')
        return async (op) => {
          if (op.action === 'authorize')
            return { state: 'authorized', account: 'square-shop', reference: 'sq_' + op.paymentId };
          assert.equal(op.action, 'capture');
          await nativePayments.markCompleted(op.paymentId);
          return { state: 'captured', reference: 'sq_' + op.paymentId };
        };
      const value = target[key];
      return typeof value === 'function' ? value.bind(target) : value;
    },
  });
  const processorShop = { ...shop, payments: simulateProcessor, products: engine.products };
  assert.equal(
    (await advanceRoutedCheckout(processorShop, env, original, { showOffer: true })).offer.name,
    'MUG',
  );
  const additional = await routedOffer(engine, env, original, 'offer');
  assert.deepEqual(await routedOffer(engine, env, original, 'offer'), additional);
  const second = await routedSession(engine, additional.url.split('#')[1]);
  assert.equal(
    (await advanceRoutedCheckout(processorShop, env, second, { showOffer: true })).offer,
    undefined,
  );
  assert.equal((await engine.payments.get(second.paymentId)).amountExact, '4.3');
  let refundCalls = 0;
  const refundPayments = new Proxy(nativePayments, {
    get(target, key) {
      if (key === 'routeCard')
        return async (op) => {
          assert.equal(op.action, 'refund');
          const [refund] = await nativePayments.getRefunds(op.paymentId);
          if (++refundCalls === 2) await nativePayments.completeRefund(refund.id);
          return { refundId: refund.id };
        };
      const value = target[key];
      return typeof value === 'function' ? value.bind(target) : value;
    },
  });
  const refundShop = { ...shop, payments: refundPayments };
  const partial = await refundRoutedCheckout(refundShop, env, {
    paymentId: second.paymentId,
    amount: '0.25',
    reason: 'Partial return',
    requestKey: 'routed-worker-retry',
  });
  assert.equal(partial.status, 'pending');
  assert.deepEqual(await recoverRoutedRefunds(refundShop, env), [
    { refundId: partial.id, state: 'preview' },
  ]);
  assert.equal(refundCalls, 1);
  assert.deepEqual(await recoverRoutedRefunds(refundShop, env, true), [
    { refundId: partial.id, state: 'completed' },
  ]);
  assert.deepEqual(await recoverRoutedRefunds(refundShop, env, true), []);
  assert.equal((await nativePayments.get(second.paymentId)).amountRefundedExact, '0.25');
  const wallet = privateKeyToAccount(generatePrivateKey());
  env.STATESET_AFFILIATE_DEFAULT_BPS = '1200';
  env.STATESET_SELLER_SIGNUP_COUNTRIES = 'US';
  async function sign(role, action, fields = {}) {
    const payload = {
      role,
      action,
      wallet: wallet.address,
      at: Date.now(),
      nonce: crypto.randomUUID(),
      ...fields,
    };
    return {
      ...payload,
      signature: await wallet.signMessage({ message: partnerMessage(origin, payload) }),
    };
  }
  const enroll = await sign('affiliate', 'portal', { bps: 1200 });
  await assert.rejects(
    openPartnerAccount(engine, env, { ...enroll, bps: 1201 }, verifyMessage),
    /Signature/,
  );
  const joined = await openPartnerAccount(engine, env, enroll, verifyMessage);
  const affiliate = await portalPrincipal(engine, new URL(joined.url).hash.slice(1));
  assert.equal(affiliate.subject, 'wallet-' + wallet.address.slice(2).toLowerCase());
  await assert.rejects(openPartnerAccount(engine, env, enroll, verifyMessage), /fresh/);
  const accounts = [];
  const sellerStripe = {
    accounts: {
      list: async function* () {
        yield* accounts;
      },
      create: async (data) => {
        const acct = { ...data, id: 'acct_self' + accounts.length };
        accounts.push(acct);
        return acct;
      },
    },
    accountLinks: {
      create: async (p) => {
        assert.ok(accounts.some((a) => a.id === p.account));
        return { url: 'https://connect.stripe.com/fixture', expires_at: 123 };
      },
    },
  };
  const sellerFields = { country: 'US', email: 'seller@example.test' };
  for (let repeat = 0; repeat < 2; repeat++)
    assert.equal(
      (
        await openPartnerAccount(
          engine,
          env,
          await sign('seller', 'onboard', sellerFields),
          verifyMessage,
          () => sellerStripe,
        )
      ).url,
      'https://connect.stripe.com/fixture',
    );
  assert.equal(accounts.length, 1);
  env.STATESET_AFFILIATE_PAYOUT_COUNTRIES = 'US';
  const affiliateFields = { country: 'US', email: 'affiliate@example.test', bps: 1200 };
  const affiliateOnboarding = await sign('affiliate', 'onboard', affiliateFields);
  await openPartnerAccount(engine, env, affiliateOnboarding, verifyMessage, () => sellerStripe);
  await assert.rejects(
    openPartnerAccount(engine, env, affiliateOnboarding, verifyMessage, () => sellerStripe),
    /fresh/,
  );
  assert.equal(accounts.length, 2);
  assert.equal(accounts[1].metadata.stateset_affiliate, affiliate.subject);
  const sellerLink = await openPartnerAccount(
    engine,
    env,
    await sign('seller', 'portal'),
    verifyMessage,
  );
  assert.equal(
    (await portalPrincipal(engine, new URL(sellerLink.url).hash.slice(1))).role,
    'seller',
  );
  const pricing = {
    revision: 0,
    rates: { CA: '0.05' },
    methods: [{ id: 'ground', label: 'Ground', amount: '2', countries: ['US'] }],
  };
  assert.equal((await publishPricing(engine, env, pricing)).mode, 'preview');
  assert.equal(
    (await pricingEnvironment(engine, env)).STATESET_TAX_RATES_JSON,
    env.STATESET_TAX_RATES_JSON,
  );
  await publishPricing(engine, env, pricing, true);
  const repriced = await startRoutedCheckout({ commerce: engine, env, input: await cartInput() });
  const checkoutWithRates = await routedSession(engine, repriced.url.split('#')[1]);
  assert.equal((await engine.payments.get(checkoutWithRates.paymentId)).amountExact, '12.5');
  await revokePortal(engine, link.id, true);
  assert.equal(await portalPrincipal(engine, new URL(link.url).hash.slice(1)), null);
  console.log(
    JSON.stringify({
      result: 'passed',
      externalProvidersVerified: false,
      checks: [
        'native Square account selection',
        'buyer consent on original payment',
        'exact saved-card total including shipping and tax',
        'one supplemental charge after retries',
        'affiliate-only reporting and revocation',
        'seller routing eligibility before stock reservation',
        'merchant campaign preview and revision publication',
        'pinned campaigns and visitor-cohort order drill-down',
        'routed primary offers and supplemental checkout replay',
        'scheduled partial refund reconciliation after original checkout completion',
        'wallet-signed self-enrollment, seller verification resume and rejection of replay',
        'published pricing changes the amount charged by the storefront',
      ],
    }),
  );
} finally {
  await engine.close();
}
