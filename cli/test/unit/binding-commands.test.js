/**
 * The `executeCommand` modules that used to call binding methods that never
 * existed (`commerce.listSubscriptionPlans`, `commerce.fraud.assessOrder`,
 * `commerce.reviews.approve`, `commerce.promotions()`, ...), run against a
 * recording mock of the binding. The mock refuses any method the binding's
 * `index.d.ts` does not declare, and each test asserts the exact call made.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { executeCommand, commands } from '../../src/commands/index.js';
import { bindingClassMethods } from '../helpers/binding-class-methods.js';

const API_CLASSES = {
  subscriptions: 'Subscriptions',
  fraud: 'Fraud',
  loyalty: 'Loyalty',
  reviews: 'Reviews',
  segments: 'Segments',
  inventory: 'Inventory',
  promotions: 'Promotions',
  wishlists: 'Wishlists',
  products: 'Products',
  carts: 'Carts',
};

const UUID = '11111111-2222-4333-8444-555555555555';

/**
 * Recording binding mock: `{ commerce, calls }`, `calls` as
 * `['api.method', ...args]`. Every API is a plain object property, as on the
 * raw binding -- never a callable -- and holds only declared methods.
 */
function makeCommerce(apis) {
  const calls = [];
  const commerce = {};
  for (const [api, impls] of Object.entries(apis)) {
    const declared = bindingClassMethods(API_CLASSES[api]);
    commerce[api] = {};
    for (const [name, fn] of Object.entries(impls)) {
      assert.ok(declared.has(name), `mock defines ${api}.${name}, which the binding lacks`);
      commerce[api][name] = async (...args) => {
        calls.push([`${api}.${name}`, ...args]);
        return fn(...args);
      };
    }
  }
  return { commerce, calls };
}

const output = { table: (rows) => `table:${rows.length}` };
const run = (commerce, resource, action, args, jsonOutput = true, extra = {}) =>
  executeCommand(resource, action, args, { commerce, output, jsonOutput, ...extra });

describe('subscriptions command', () => {
  const plan = { id: UUID, code: 'BOX', name: 'Box', priceExact: '19.99', currency: 'USD' };
  const sub = { id: UUID, subscriptionNumber: 'SUB-1', priceExact: '19.99', currency: 'USD' };

  it('routes plan lookups by UUID and by code', async () => {
    const { commerce, calls } = makeCommerce({
      subscriptions: { getPlan: () => plan, getPlanByCode: () => plan },
    });
    await run(commerce, 'subscriptions', 'plan', [UUID]);
    await run(commerce, 'subscriptions', 'plan', ['BOX']);
    assert.deepEqual(calls, [
      ['subscriptions.getPlan', UUID],
      ['subscriptions.getPlanByCode', 'BOX'],
    ]);
  });

  it('routes subscription lookups by UUID and by number', async () => {
    const { commerce, calls } = makeCommerce({
      subscriptions: { get: () => sub, getByNumber: () => sub },
    });
    await run(commerce, 'subscriptions', 'get', [UUID]);
    await run(commerce, 'subscriptions', 'get', ['SUB-1']);
    assert.deepEqual(calls, [
      ['subscriptions.get', UUID],
      ['subscriptions.getByNumber', 'SUB-1'],
    ]);
  });

  it('lists plans, subscriptions and cycles through the Subscriptions API', async () => {
    const { commerce, calls } = makeCommerce({
      subscriptions: {
        listPlans: () => [plan],
        list: () => [sub],
        listBillingCycles: () => [{ id: 'c1', totalExact: '19.99' }],
      },
    });
    const plans = await run(commerce, 'subscriptions', 'plans', ['active', 'monthly'], false);
    await run(commerce, 'subscriptions', 'list', ['cust-1', 'active']);
    await run(commerce, 'subscriptions', 'cycles', [UUID, 'scheduled']);
    assert.equal(plans.formatted, 'table:1');
    assert.deepEqual(calls, [
      ['subscriptions.listPlans', { status: 'active', billingInterval: 'monthly' }],
      ['subscriptions.list', { customerId: 'cust-1', status: 'active' }],
      ['subscriptions.listBillingCycles', { subscriptionId: UUID, status: 'scheduled' }],
    ]);
  });

  it('subscribes, pauses, resumes and cancels', async () => {
    const { commerce, calls } = makeCommerce({
      subscriptions: {
        subscribe: () => sub,
        pause: () => sub,
        resume: () => sub,
        cancel: () => sub,
      },
    });
    await run(commerce, 'subscriptions', 'create', ['cust-1', UUID]);
    await run(commerce, 'subscriptions', 'pause', [UUID, 'on', 'vacation']);
    await run(commerce, 'subscriptions', 'resume', [UUID]);
    const cancelled = await run(commerce, 'subscriptions', 'cancel', [UUID, 'immediate']);
    assert.match(cancelled.formatted, /immediately/);
    assert.deepEqual(calls, [
      ['subscriptions.subscribe', { customerId: 'cust-1', planId: UUID }],
      ['subscriptions.pause', UUID, { reason: 'on vacation' }],
      ['subscriptions.resume', UUID],
      ['subscriptions.cancel', UUID, { immediate: true }],
    ]);
  });

  it('limits events client-side: getEvents takes only the subscription ID', async () => {
    const events = [{ id: 'e1' }, { id: 'e2' }, { id: 'e3' }];
    const { commerce, calls } = makeCommerce({ subscriptions: { getEvents: () => events } });
    const result = await run(commerce, 'subscriptions', 'events', [UUID, '2']);
    assert.deepEqual(result, events.slice(0, 2));
    assert.deepEqual(calls, [['subscriptions.getEvents', UUID]]);
  });
});

describe('fraud command', () => {
  const assessment = {
    orderId: 'ord-1',
    riskScore: 0.9,
    signals: [{ orderId: 'ord-1', signalType: 'proxy_vpn', score: 0.9, details: 'VPN' }],
    decision: 'review',
    needsReview: true,
  };

  it('records an assessment from caller-supplied signals', async () => {
    const { commerce, calls } = makeCommerce({ fraud: { createAssessment: () => assessment } });
    const signals = [{ signalType: 'proxy_vpn', score: '0.9', details: 'VPN' }];
    await run(commerce, 'fraud', 'assess', ['ord-1', JSON.stringify(signals)]);
    assert.deepEqual(calls, [
      [
        'fraud.createAssessment',
        { orderId: 'ord-1', signals: [{ signalType: 'proxy_vpn', score: 0.9, details: 'VPN' }] },
      ],
    ]);
  });

  it('lists signals from one order or from assessments above a risk score', async () => {
    const { commerce, calls } = makeCommerce({
      fraud: { getAssessment: () => assessment, listAssessments: () => [assessment] },
    });
    const one = await run(commerce, 'fraud', 'signals', ['ord-1']);
    const all = await run(commerce, 'fraud', 'signals', ['', '0.5', '1']);
    assert.deepEqual(one, assessment.signals);
    assert.equal(all.length, 1);
    assert.deepEqual(calls, [
      ['fraud.getAssessment', 'ord-1'],
      ['fraud.listAssessments', { minRiskScore: 0.5 }],
    ]);
  });

  it('reviews by order ID with a reviewer and notes', async () => {
    const { commerce, calls } = makeCommerce({ fraud: { reviewAssessment: () => assessment } });
    await run(commerce, 'fraud', 'review', ['ord-1', 'reject', 'ops', 'confirmed', 'VPN']);
    assert.deepEqual(calls, [
      ['fraud.reviewAssessment', 'ord-1', 'reject', 'ops', 'confirmed VPN'],
    ]);
  });
});

describe('loyalty command', () => {
  const account = { id: 'acct-1', customerId: 'c1', programId: 'p1', pointsBalance: 100 };
  const tx = (points, transactionType) => ({
    id: 't1',
    accountId: 'acct-1',
    points,
    transactionType,
  });

  it('enrolls with the EnrollCustomerInput shape', async () => {
    const { commerce, calls } = makeCommerce({ loyalty: { enroll: () => account } });
    await run(commerce, 'loyalty', 'enroll', ['p1', 'c1']);
    assert.deepEqual(calls, [['loyalty.enroll', { customerId: 'c1', programId: 'p1' }]]);
  });

  it('earns and redeems as adjustPoints on the customer account', async () => {
    const reward = { id: 'r1', programId: 'p1', name: '5 off', isActive: true };
    const { commerce, calls } = makeCommerce({
      loyalty: {
        getAccountByCustomer: () => account,
        getReward: () => reward,
        adjustPoints: (input) => tx(input.points, input.transactionType),
      },
    });
    await run(commerce, 'loyalty', 'earn', ['p1', 'c1', '100', 'purchase', 'ord-1', 'first']);
    await run(commerce, 'loyalty', 'redeem', ['p1', 'c1', '40', 'r1']);
    assert.deepEqual(calls, [
      ['loyalty.getAccountByCustomer', 'c1', 'p1'],
      [
        'loyalty.adjustPoints',
        {
          accountId: 'acct-1',
          points: 100,
          transactionType: 'earn',
          referenceId: 'ord-1',
          description: 'purchase: first',
        },
      ],
      ['loyalty.getAccountByCustomer', 'c1', 'p1'],
      ['loyalty.getReward', 'r1'],
      [
        'loyalty.adjustPoints',
        {
          accountId: 'acct-1',
          points: -40,
          transactionType: 'redeem',
          referenceId: 'r1',
          description: 'reward r1 (5 off)',
        },
      ],
    ]);
  });

  it('refuses to earn for a customer who is not enrolled', async () => {
    const { commerce, calls } = makeCommerce({ loyalty: { getAccountByCustomer: () => null } });
    await assert.rejects(run(commerce, 'loyalty', 'earn', ['p1', 'c1', '5']), /not enrolled/);
    assert.equal(calls.length, 1);
  });

  it('reads accounts by customer and rewards by filter', async () => {
    const { commerce, calls } = makeCommerce({
      loyalty: { getAccountByCustomer: () => account, listRewards: () => [] },
    });
    await run(commerce, 'loyalty', 'account', ['p1', 'c1']);
    await run(commerce, 'loyalty', 'rewards', ['p1', 'discount']);
    assert.deepEqual(calls, [
      ['loyalty.getAccountByCustomer', 'c1', 'p1'],
      ['loyalty.listRewards', { programId: 'p1', rewardType: 'discount' }],
    ]);
  });

  it('creates rewards with an exact decimal value', async () => {
    const { commerce, calls } = makeCommerce({
      loyalty: { createReward: (i) => ({ id: 'r1', ...i }) },
    });
    await run(commerce, 'loyalty', 'create-reward', ['p1', '5 off', '40', 'discount', '5.00']);
    assert.deepEqual(calls, [
      [
        'loyalty.createReward',
        {
          programId: 'p1',
          name: '5 off',
          description: undefined,
          pointsCost: 40,
          rewardType: 'discount',
          value: '5.00',
        },
      ],
    ]);
    await assert.rejects(
      run(commerce, 'loyalty', 'create-reward', ['p1', 'x', '1', 'discount', '5.0.0']),
      /decimal string/,
    );
  });
});

describe('reviews command', () => {
  const review = (overrides = {}) => ({ id: 'rev-1', rating: 4, reportedCount: 0, ...overrides });

  it('moderates through update, and flags through markReported + update', async () => {
    const { commerce, calls } = makeCommerce({
      reviews: { update: (_id, input) => review(input), markReported: () => undefined },
    });
    await run(commerce, 'reviews', 'approve', ['rev-1']);
    const rejected = await run(commerce, 'reviews', 'reject', ['rev-1', 'off', 'topic']);
    const flagged = await run(commerce, 'reviews', 'flag', ['rev-1', 'spam', 'fake']);
    assert.equal(rejected.reasonPersisted, false);
    assert.equal(flagged.reason, 'spam');
    assert.deepEqual(calls, [
      ['reviews.update', 'rev-1', { status: 'approved' }],
      ['reviews.update', 'rev-1', { status: 'rejected' }],
      ['reviews.markReported', 'rev-1'],
      ['reviews.update', 'rev-1', { status: 'flagged' }],
    ]);
  });

  it('counts by paging list and applies maxRating client-side', async () => {
    const page = [review({ rating: 5 }), review({ rating: 2 })];
    const { commerce, calls } = makeCommerce({ reviews: { list: () => page } });
    const result = await run(commerce, 'reviews', 'count', ['prod-1', '', 'approved', '', '3']);
    assert.equal(result.count, 1);
    assert.deepEqual(calls, [
      [
        'reviews.list',
        {
          productId: 'prod-1',
          customerId: undefined,
          status: 'approved',
          minRating: undefined,
          limit: 1000,
          offset: 0,
        },
      ],
    ]);
  });
});

describe('segments command', () => {
  it('checks stored membership with isMember', async () => {
    const { commerce, calls } = makeCommerce({ segments: { isMember: () => true } });
    const result = await run(commerce, 'segments', 'evaluate', ['seg-1', 'c1']);
    assert.deepEqual(result, {
      segmentId: 'seg-1',
      customerId: 'c1',
      isMember: true,
      basis: 'stored_membership',
    });
    assert.deepEqual(calls, [['segments.isMember', 'seg-1', 'c1']]);
  });

  it('counts by paging list with the segmentType filter', async () => {
    const { commerce, calls } = makeCommerce({ segments: { list: () => [{ id: 's1' }] } });
    const result = await run(commerce, 'segments', 'count', ['static']);
    assert.equal(result.count, 1);
    assert.deepEqual(calls, [['segments.list', { segmentType: 'static', limit: 1000, offset: 0 }]]);
  });

  it('no longer offers rebuild: nothing in the engine evaluates segment rules', async () => {
    assert.equal(commands.segments.metadata.actions.rebuild, undefined);
    const { commerce } = makeCommerce({ segments: {} });
    await assert.rejects(run(commerce, 'segments', 'rebuild', ['seg-1']), /Unknown action/);
  });
});

describe('inventory command', () => {
  it('reserves against an order reference and releases by reservation ID', async () => {
    const { commerce, calls } = makeCommerce({
      inventory: {
        reserve: () => ({ id: 'res-1', itemId: 1, quantity: '3', status: 'pending' }),
        releaseReservation: () => undefined,
      },
    });
    await run(commerce, 'inventory', 'reserve', ['W-1', '3', 'ord-1']);
    const released = await run(commerce, 'inventory', 'release', ['res-1']);
    assert.equal(released.released, true);
    assert.deepEqual(calls, [
      ['inventory.reserve', 'W-1', 3, 'order', 'ord-1', undefined],
      ['inventory.releaseReservation', 'res-1'],
    ]);
  });
});

describe('promotions command', () => {
  it('uses the promotions property, never commerce.promotions()', async () => {
    const evaluation = {
      originalSubtotalExact: '10',
      totalDiscountExact: '1',
      discountedSubtotalExact: '9',
      grandTotalExact: '9',
    };
    const { commerce, calls } = makeCommerce({
      promotions: {
        get: () => {
          throw new Error('invalid uuid');
        },
        getByCode: () => ({ id: 'p1', code: 'SAVE', name: 'Save' }),
        applyToCart: () => evaluation,
      },
    });
    await run(commerce, 'promotions', 'get', ['SAVE']);
    const applied = await run(commerce, 'promotions', 'apply', ['cart-1'], false);
    assert.match(applied.formatted, /Grand total: {9}9/);
    assert.deepEqual(calls, [
      ['promotions.get', 'SAVE'],
      ['promotions.getByCode', 'SAVE'],
      ['promotions.applyToCart', 'cart-1'],
    ]);
  });
});

describe('wishlists command', () => {
  it('converts by composing products + carts at the exact variant price', async () => {
    const wishlist = {
      id: 'wl-1',
      customerId: 'c1',
      name: 'Wants',
      isPublic: false,
      items: [{ productId: 'prod-1', quantity: 2, addedAt: 'now' }],
    };
    const { commerce, calls } = makeCommerce({
      wishlists: { get: () => wishlist, removeItem: () => undefined },
      products: {
        get: () => ({ id: 'prod-1', name: 'Widget', status: 'active' }),
        getVariants: () => [{ id: 'var-1', sku: 'W-1', isDefault: true, priceExact: '12.50' }],
      },
      carts: { create: () => ({ id: 'cart-1' }), addItemExact: () => ({}) },
    });
    const result = await run(commerce, 'wishlists', 'convert', ['wl-1', 'true']);
    assert.equal(result.cartId, 'cart-1');
    assert.equal(result.itemsAdded, 1);
    assert.deepEqual(result.removedFromWishlist, ['prod-1']);
    assert.deepEqual(calls, [
      ['wishlists.get', 'wl-1'],
      ['products.get', 'prod-1'],
      ['products.getVariants', 'prod-1'],
      ['carts.create', { customerId: 'c1' }],
      [
        'carts.addItemExact',
        'cart-1',
        {
          productId: 'prod-1',
          variantId: 'var-1',
          sku: 'W-1',
          name: 'Widget',
          quantity: 2,
          unitPrice: '12.50',
        },
      ],
      ['wishlists.removeItem', 'wl-1', 'prod-1'],
    ]);
  });

  it('creates with isPublic, not a visibility string', async () => {
    const { commerce, calls } = makeCommerce({
      wishlists: { create: (i) => ({ id: 'wl-1', items: [], ...i }) },
    });
    await run(commerce, 'wishlists', 'create', ['c1', 'Wants', 'public']);
    assert.deepEqual(calls, [
      ['wishlists.create', { customerId: 'c1', name: 'Wants', isPublic: true }],
    ]);
  });
});

describe('sync command', () => {
  it('explains that it needs a db handle instead of throwing a TypeError', async () => {
    const { commerce } = makeCommerce({});
    // `ensureConfigured` may refuse first when sync is not set up; either way
    // the failure is a described Error, never `commerce.db` being undefined.
    await assert.rejects(run(commerce, 'sync', 'outbox', []), (error) => {
      assert.ok(!(error instanceof TypeError), error.message);
      assert.match(error.message, /sync|database handle/i);
      return true;
    });
  });
});
