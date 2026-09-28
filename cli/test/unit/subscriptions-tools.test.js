/**
 * Subscriptions Tools — Comprehensive Test Suite
 *
 * Tests every tool exported from src/tools/subscriptions.js:
 *   list_subscription_plans, get_subscription_plan, create_subscription_plan,
 *   activate_subscription_plan, update_subscription_plan, archive_subscription_plan,
 *   list_subscriptions, get_subscription, create_subscription, pause_subscription,
 *   update_subscription, resume_subscription,
 *   cancel_subscription, skip_billing_cycle, list_billing_cycles,
 *   get_billing_cycle, get_subscription_events
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { subscriptionTools } from '../../src/tools/subscriptions.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function findTool(name) {
  const tool = subscriptionTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found in subscriptionTools`);
  return tool;
}

// Real SubscriptionPlanOutput / SubscriptionOutput / BillingCycleOutput /
// SubscriptionEventOutput shapes (bindings/node/index.d.ts): money is a float
// `price` plus an exact `priceExact` twin; ids are UUIDs.
const PLAN_ID = '11111111-1111-4111-8111-111111111111';
const SUB_ID = '22222222-2222-4222-8222-222222222222';
const CYCLE_ID = '33333333-3333-4333-8333-333333333333';
const MISSING_ID = '99999999-9999-4999-8999-999999999999';

function makePlan(overrides = {}) {
  return {
    id: PLAN_ID,
    code: 'COFFEE_MONTHLY',
    name: 'Coffee Club Monthly',
    status: 'active',
    billingInterval: 'monthly',
    price: 29.99,
    priceExact: '29.99',
    currency: 'USD',
    trialDays: 14,
    trialRequiresPaymentMethod: false,
    description: 'Monthly coffee subscription',
    ...overrides,
  };
}

function makeSub(overrides = {}) {
  return {
    id: SUB_ID,
    subscriptionNumber: 'SUB-100001',
    customerId: 'cust_001',
    planId: PLAN_ID,
    planName: 'Coffee Club Monthly',
    status: 'active',
    price: 29.99,
    priceExact: '29.99',
    currency: 'USD',
    nextBillingDate: '2026-03-20T00:00:00Z',
    billingCycleCount: 3,
    ...overrides,
  };
}

function makeCycle(overrides = {}) {
  return {
    id: CYCLE_ID,
    subscriptionId: SUB_ID,
    cycleNumber: 1,
    status: 'paid',
    periodStart: '2026-02-01T00:00:00Z',
    periodEnd: '2026-03-01T00:00:00Z',
    total: 29.99,
    totalExact: '29.99',
    currency: 'USD',
    billedAt: '2026-02-01T00:00:00Z',
    ...overrides,
  };
}

function makeEvent(overrides = {}) {
  return {
    id: 'evt_001',
    subscriptionId: SUB_ID,
    eventType: 'created',
    description: 'Subscription created',
    triggeredBy: 'system',
    createdAt: '2026-02-20T00:00:00Z',
    ...overrides,
  };
}

/**
 * Mirrors the binding's `Subscriptions` class: only methods that really exist
 * on `commerce.subscriptions`. `overrides` replaces individual methods.
 */
function makeCommerce(overrides = {}) {
  return {
    subscriptions: {
      listPlans: async () => [makePlan()],
      getPlan: async (id) => (id === MISSING_ID ? null : makePlan({ id })),
      getPlanByCode: async (code) => (code === 'MISSING' ? null : makePlan({ code })),
      createPlan: async (input) => makePlan({ id: PLAN_ID, ...input }),
      activatePlan: async (id) => makePlan({ id, status: 'active' }),
      updatePlan: async (id, input) => makePlan({ id, ...input }),
      archivePlan: async (id) => makePlan({ id, status: 'archived' }),
      subscribe: async (input) => makeSub({ ...input }),
      get: async (id) => (id === MISSING_ID ? null : makeSub({ id })),
      getByNumber: async (number) =>
        number === 'MISSING' ? null : makeSub({ subscriptionNumber: number }),
      list: async () => [makeSub()],
      update: async (id, input) => makeSub({ id, ...input }),
      pause: async (id) => makeSub({ id, status: 'paused' }),
      resume: async (id) => makeSub({ id, status: 'active' }),
      cancel: async (id) => makeSub({ id, status: 'cancelled' }),
      skipBilling: async (id) => makeSub({ id, nextBillingDate: '2026-04-20T00:00:00Z' }),
      listBillingCycles: async () => [makeCycle()],
      getBillingCycle: async (id) => (id === MISSING_ID ? null : makeCycle({ id })),
      getEvents: async () => [makeEvent()],
      ...overrides,
    },
  };
}

// ---------------------------------------------------------------------------
// Structure tests
// ---------------------------------------------------------------------------

describe('Subscription Tools — structure', () => {
  it('exports an array of 17 tools', () => {
    assert.ok(Array.isArray(subscriptionTools));
    assert.strictEqual(subscriptionTools.length, 17);
  });

  it('every tool has name, handler, permission, and inputSchema', () => {
    for (const tool of subscriptionTools) {
      assert.ok(typeof tool.name === 'string', `Missing name`);
      assert.ok(typeof tool.handler === 'function', `${tool.name}: handler not a function`);
      assert.ok(typeof tool.permission === 'string', `${tool.name}: missing permission`);
      assert.ok(typeof tool.inputSchema === 'object', `${tool.name}: missing inputSchema`);
    }
  });

  it('tool names are unique', () => {
    const names = subscriptionTools.map((t) => t.name);
    assert.strictEqual(new Set(names).size, names.length);
  });
});

// ---------------------------------------------------------------------------
// list_subscription_plans
// ---------------------------------------------------------------------------

describe('list_subscription_plans', () => {
  const tool = findTool('list_subscription_plans');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns plans array with success', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: {} });
    assert.strictEqual(result.success, true);
    assert.ok(Array.isArray(result.plans));
    assert.strictEqual(result.count, 1);
  });

  it('maps plan fields correctly', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: {} });
    const plan = result.plans[0];
    assert.strictEqual(plan.id, PLAN_ID);
    assert.strictEqual(plan.name, 'Coffee Club Monthly');
    // Exact decimal twin is surfaced, never the float.
    assert.strictEqual(plan.price, '29.99');
    assert.strictEqual(plan.billingInterval, 'monthly');
    assert.strictEqual(plan.trialDays, 14);
  });

  it('passes status and billingInterval filters', async () => {
    let calledWith = {};
    const commerce = makeCommerce({
      listPlans: async (filters) => {
        calledWith = filters;
        return [];
      },
    });
    await tool.handler({ commerce, params: { status: 'active', billingInterval: 'monthly' } });
    assert.strictEqual(calledWith.status, 'active');
    assert.strictEqual(calledWith.billingInterval, 'monthly');
  });
});

// ---------------------------------------------------------------------------
// get_subscription_plan
// ---------------------------------------------------------------------------

describe('get_subscription_plan', () => {
  const tool = findTool('get_subscription_plan');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns plan by ID via subscriptions.getPlan', async () => {
    const calls = [];
    const commerce = makeCommerce({
      getPlan: async (id) => (calls.push(['getPlan', id]), makePlan({ id })),
      getPlanByCode: async (code) => (calls.push(['getPlanByCode', code]), null),
    });
    const result = await tool.handler({ commerce, params: { planId: PLAN_ID } });
    assert.strictEqual(result.success, true);
    assert.ok(result.plan);
    assert.deepStrictEqual(calls, [['getPlan', PLAN_ID]]);
  });

  it('routes a non-UUID plan code to getPlanByCode (getPlan rejects non-UUIDs)', async () => {
    const calls = [];
    const commerce = makeCommerce({
      getPlan: async () => {
        throw new Error('Invalid UUID');
      },
      getPlanByCode: async (code) => (calls.push(code), makePlan({ code })),
    });
    const result = await tool.handler({ commerce, params: { planId: 'COFFEE_MONTHLY' } });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.plan.code, 'COFFEE_MONTHLY');
    assert.deepStrictEqual(calls, ['COFFEE_MONTHLY']);
  });

  it('returns error when plan code not found', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: { planId: 'MISSING' } });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.toLowerCase().includes('not found'));
  });

  it('returns error when plan ID not found', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { planId: MISSING_ID },
    });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.toLowerCase().includes('not found'));
  });
});

// ---------------------------------------------------------------------------
// create_subscription_plan
// ---------------------------------------------------------------------------

describe('create_subscription_plan', () => {
  const tool = findTool('create_subscription_plan');
  const params = { name: 'Pro Plan', billingInterval: 'monthly', price: 49.99 };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
    assert.ok(result.wouldCreate);
  });

  it('creates plan when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.plan);
    assert.ok(result.message.includes('Pro Plan'));
  });

  it('passes price and setupFee as numbers (CreateSubscriptionPlanInput is number-typed)', async () => {
    let calledWith = {};
    const commerce = makeCommerce({
      createPlan: async (input) => {
        calledWith = input;
        return makePlan(input);
      },
    });
    await tool.handler({ commerce, params: { ...params, setupFee: 5 }, allowApply: true });
    assert.strictEqual(calledWith.price, 49.99);
    assert.strictEqual(calledWith.setupFee, 5);
    assert.strictEqual(calledWith.billingInterval, 'monthly');
    assert.strictEqual(calledWith.name, 'Pro Plan');
  });
});

// ---------------------------------------------------------------------------
// activate_subscription_plan
// ---------------------------------------------------------------------------

describe('activate_subscription_plan', () => {
  const tool = findTool('activate_subscription_plan');
  const params = { planId: PLAN_ID };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldActivate);
  });

  it('activates plan when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('activated'));
  });
});

describe('update_subscription_plan', () => {
  const tool = findTool('update_subscription_plan');
  const params = { planId: PLAN_ID, updates: { name: 'Updated Plan' } };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldUpdate);
  });

  it('updates plan when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.plan.name, 'Updated Plan');
  });

  it('coerces decimal-string money to numbers for updatePlan', async () => {
    let calledWith;
    const commerce = makeCommerce({
      updatePlan: async (id, input) => {
        calledWith = [id, input];
        return makePlan({ id, ...input });
      },
    });
    const result = await tool.handler({
      commerce,
      params: { planId: PLAN_ID, updates: { price: '31.50', setupFee: 2 } },
      allowApply: true,
    });
    assert.strictEqual(result.success, true);
    assert.deepStrictEqual(calledWith, [PLAN_ID, { price: 31.5, setupFee: 2 }]);
  });

  it('refuses fields UpdateSubscriptionPlanInput does not have', async () => {
    let called = false;
    const commerce = makeCommerce({
      updatePlan: async () => {
        called = true;
      },
    });
    const result = await tool.handler({
      commerce,
      params: { planId: PLAN_ID, updates: { metadata: { a: 1 } } },
      allowApply: true,
    });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('metadata'));
    assert.strictEqual(called, false);
  });
});

// ---------------------------------------------------------------------------
// archive_subscription_plan
// ---------------------------------------------------------------------------

describe('archive_subscription_plan', () => {
  const tool = findTool('archive_subscription_plan');
  const params = { planId: PLAN_ID };

  it('has delete permission', () => {
    assert.strictEqual(tool.permission, 'delete');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldArchive);
  });

  it('archives plan when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('archived'));
  });
});

// ---------------------------------------------------------------------------
// list_subscriptions
// ---------------------------------------------------------------------------

describe('list_subscriptions', () => {
  const tool = findTool('list_subscriptions');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns subscriptions array', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: {} });
    assert.ok(Array.isArray(result.subscriptions));
    assert.strictEqual(result.count, 1);
  });

  it('maps subscription fields correctly', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: {} });
    const sub = result.subscriptions[0];
    assert.strictEqual(sub.id, SUB_ID);
    assert.strictEqual(sub.subscriptionNumber, 'SUB-100001');
    assert.strictEqual(sub.price, '29.99');
    assert.strictEqual(sub.status, 'active');
  });

  it('passes filters to commerce', async () => {
    let calledWith = {};
    const commerce = makeCommerce({
      list: async (filters) => {
        calledWith = filters;
        return [];
      },
    });
    await tool.handler({ commerce, params: { customerId: 'cust_001', status: 'active' } });
    assert.strictEqual(calledWith.customerId, 'cust_001');
    assert.strictEqual(calledWith.status, 'active');
  });
});

// ---------------------------------------------------------------------------
// get_subscription
// ---------------------------------------------------------------------------

describe('get_subscription', () => {
  const tool = findTool('get_subscription');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns subscription by ID', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { subscriptionId: SUB_ID },
    });
    assert.ok(result);
    assert.strictEqual(result.id, SUB_ID);
  });

  it('returns error when subscription not found', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { subscriptionId: MISSING_ID },
    });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.toLowerCase().includes('not found'));
  });

  it('routes a subscription number to getByNumber (get rejects non-UUIDs)', async () => {
    const result = await tool.handler({
      commerce: makeCommerce({
        get: async () => {
          throw new Error('Invalid UUID');
        },
      }),
      params: { subscriptionId: 'SUB-100001' },
    });
    assert.strictEqual(result.subscriptionNumber, 'SUB-100001');
  });
});

// ---------------------------------------------------------------------------
// create_subscription
// ---------------------------------------------------------------------------

describe('create_subscription', () => {
  const tool = findTool('create_subscription');
  const params = { customerId: 'cust_001', planId: 'plan_001' };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldSubscribe);
    assert.strictEqual(result.wouldSubscribe.customerId, 'cust_001');
  });

  it('creates subscription via subscriptions.subscribe when allowApply is true', async () => {
    let calledWith;
    const commerce = makeCommerce({
      subscribe: async (input) => {
        calledWith = input;
        return makeSub(input);
      },
    });
    const result = await tool.handler({
      commerce,
      params: { ...params, skipTrial: true },
      allowApply: true,
    });
    assert.strictEqual(result.success, true);
    assert.ok(result.subscription);
    assert.ok(result.message.includes('SUB-'));
    assert.strictEqual(calledWith.customerId, 'cust_001');
    assert.strictEqual(calledWith.planId, 'plan_001');
    assert.strictEqual(calledWith.skipTrial, true);
  });
});

// ---------------------------------------------------------------------------
// pause_subscription
// ---------------------------------------------------------------------------

describe('pause_subscription', () => {
  const tool = findTool('pause_subscription');
  const params = { subscriptionId: 'sub_001' };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldPause);
  });

  it('pauses subscription when allowApply is true', async () => {
    let calledWith;
    const commerce = makeCommerce({
      pause: async (id, input) => {
        calledWith = [id, input];
        return makeSub({ id, status: 'paused' });
      },
    });
    const result = await tool.handler({
      commerce,
      params: { ...params, reason: 'vacation', resumeAt: '2027-01-01' },
      allowApply: true,
    });
    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('paused'));
    assert.strictEqual(calledWith[0], 'sub_001');
    assert.strictEqual(calledWith[1].reason, 'vacation');
    assert.strictEqual(calledWith[1].resumeAt, '2027-01-01T00:00:00.000Z');
  });
});

describe('update_subscription', () => {
  const tool = findTool('update_subscription');
  const params = { subscriptionId: 'sub_001', updates: { status: 'past_due' } };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldUpdate);
  });

  it('updates subscription when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.subscription.status, 'past_due');
  });

  it('coerces decimal-string price and refuses unknown fields', async () => {
    let calledWith;
    const commerce = makeCommerce({
      update: async (id, input) => {
        calledWith = [id, input];
        return makeSub({ id, ...input });
      },
    });
    const ok = await tool.handler({
      commerce,
      params: { subscriptionId: SUB_ID, updates: { price: '27.50', couponCode: 'SAVE' } },
      allowApply: true,
    });
    assert.strictEqual(ok.success, true);
    assert.deepStrictEqual(calledWith, [SUB_ID, { price: 27.5, couponCode: 'SAVE' }]);

    calledWith = undefined;
    const refused = await tool.handler({
      commerce,
      params: { subscriptionId: SUB_ID, updates: { metadata: {} } },
      allowApply: true,
    });
    assert.strictEqual(refused.success, false);
    assert.strictEqual(calledWith, undefined);
  });
});

// ---------------------------------------------------------------------------
// resume_subscription
// ---------------------------------------------------------------------------

describe('resume_subscription', () => {
  const tool = findTool('resume_subscription');
  const params = { subscriptionId: 'sub_001' };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldResume);
  });

  it('resumes subscription when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('resumed'));
  });
});

// ---------------------------------------------------------------------------
// cancel_subscription
// ---------------------------------------------------------------------------

describe('cancel_subscription', () => {
  const tool = findTool('cancel_subscription');
  const params = { subscriptionId: 'sub_001' };

  it('has delete permission', () => {
    assert.strictEqual(tool.permission, 'delete');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldCancel);
  });

  it('cancels at period end by default when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('period end'));
  });

  it('cancels immediately when immediate is true', async () => {
    let calledWith;
    const commerce = makeCommerce({
      cancel: async (id, input) => {
        calledWith = [id, input];
        return makeSub({ id, status: 'cancelled' });
      },
    });
    const result = await tool.handler({
      commerce,
      params: { ...params, immediate: true, reason: 'done' },
      allowApply: true,
    });
    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('immediately'));
    assert.deepStrictEqual(calledWith, ['sub_001', { immediate: true, reason: 'done' }]);
  });
});

// ---------------------------------------------------------------------------
// skip_billing_cycle
// ---------------------------------------------------------------------------

describe('skip_billing_cycle', () => {
  const tool = findTool('skip_billing_cycle');
  const params = { subscriptionId: 'sub_001' };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.wouldSkip);
  });

  it('skips billing cycle when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('skipped'));
    assert.ok(result.nextBillingDate);
  });

  it('calls subscriptions.skipBilling with the reason', async () => {
    let calledWith;
    const commerce = makeCommerce({
      skipBilling: async (id, input) => {
        calledWith = [id, input];
        return makeSub({ id });
      },
    });
    await tool.handler({ commerce, params: { ...params, reason: 'away' }, allowApply: true });
    assert.deepStrictEqual(calledWith, ['sub_001', { reason: 'away' }]);
  });
});

// ---------------------------------------------------------------------------
// list_billing_cycles
// ---------------------------------------------------------------------------

describe('list_billing_cycles', () => {
  const tool = findTool('list_billing_cycles');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns billing cycles array', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { subscriptionId: 'sub_001' },
    });
    assert.ok(Array.isArray(result.cycles));
    assert.strictEqual(result.count, 1);
  });

  it('maps cycle fields correctly', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { subscriptionId: 'sub_001' },
    });
    const cycle = result.cycles[0];
    assert.strictEqual(cycle.id, CYCLE_ID);
    assert.strictEqual(cycle.status, 'paid');
    assert.strictEqual(cycle.total, '29.99');
  });

  it('passes subscriptionId and status filter', async () => {
    let calledWith;
    const commerce = makeCommerce({
      listBillingCycles: async (filter) => {
        calledWith = filter;
        return [];
      },
    });
    await tool.handler({ commerce, params: { subscriptionId: SUB_ID, status: 'scheduled' } });
    assert.deepStrictEqual(calledWith, { subscriptionId: SUB_ID, status: 'scheduled' });
  });
});

// ---------------------------------------------------------------------------
// get_billing_cycle
// ---------------------------------------------------------------------------

describe('get_billing_cycle', () => {
  const tool = findTool('get_billing_cycle');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns billing cycle by ID', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { cycleId: CYCLE_ID },
    });
    assert.ok(result);
    assert.strictEqual(result.id, CYCLE_ID);
  });

  it('returns error when cycle not found', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { cycleId: MISSING_ID },
    });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.toLowerCase().includes('not found'));
  });
});

// ---------------------------------------------------------------------------
// get_subscription_events
// ---------------------------------------------------------------------------

describe('get_subscription_events', () => {
  const tool = findTool('get_subscription_events');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns events array', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { subscriptionId: 'sub_001' },
    });
    assert.ok(Array.isArray(result.events));
    assert.strictEqual(result.count, 1);
  });

  it('maps event fields correctly', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { subscriptionId: 'sub_001' },
    });
    const evt = result.events[0];
    assert.strictEqual(evt.id, 'evt_001');
    assert.strictEqual(evt.eventType, 'created');
  });

  it('applies limit client-side (getEvents takes only the subscription id)', async () => {
    let args;
    const commerce = makeCommerce({
      getEvents: async (...a) => {
        args = a;
        return [makeEvent({ id: 'e1' }), makeEvent({ id: 'e2' }), makeEvent({ id: 'e3' })];
      },
    });
    const result = await tool.handler({ commerce, params: { subscriptionId: SUB_ID, limit: 2 } });
    assert.deepStrictEqual(args, [SUB_ID]);
    assert.strictEqual(result.count, 2);
    assert.deepStrictEqual(
      result.events.map((e) => e.id),
      ['e1', 'e2'],
    );
  });
});
