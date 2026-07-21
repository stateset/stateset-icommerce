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

function makePlan(overrides = {}) {
  return {
    id: 'plan_001',
    code: 'COFFEE_MONTHLY',
    name: 'Coffee Club Monthly',
    status: 'active',
    billingInterval: 'monthly',
    price: '29.99',
    currency: 'USD',
    trialDays: 14,
    description: 'Monthly coffee subscription',
    ...overrides,
  };
}

function makeSub(overrides = {}) {
  return {
    id: 'sub_001',
    subscriptionNumber: 'SUB-100001',
    customerId: 'cust_001',
    planName: 'Coffee Club Monthly',
    status: 'active',
    price: '29.99',
    currency: 'USD',
    nextBillingDate: '2026-03-20T00:00:00Z',
    billingCycleCount: 3,
    ...overrides,
  };
}

function makeCycle(overrides = {}) {
  return {
    id: 'cycle_001',
    cycleNumber: 1,
    status: 'paid',
    periodStart: '2026-02-01T00:00:00Z',
    periodEnd: '2026-03-01T00:00:00Z',
    total: '29.99',
    currency: 'USD',
    billedAt: '2026-02-01T00:00:00Z',
    ...overrides,
  };
}

function makeEvent(overrides = {}) {
  return {
    id: 'evt_001',
    eventType: 'subscription.created',
    description: 'Subscription created',
    triggeredBy: 'system',
    createdAt: '2026-02-20T00:00:00Z',
    ...overrides,
  };
}

function makeCommerce(overrides = {}) {
  return {
    listSubscriptionPlans: async () => [makePlan()],
    getSubscriptionPlan: async (id) => (id === 'nonexistent' ? null : makePlan({ id })),
    getSubscriptionPlanByCode: async (code) =>
      code === 'MISSING' || code === 'nonexistent' ? null : makePlan({ code }),
    createSubscriptionPlan: async (data) => makePlan({ id: 'plan_new', ...data }),
    activateSubscriptionPlan: async (id) => makePlan({ id, status: 'active' }),
    updateSubscriptionPlan: async (id, updates) => makePlan({ id, ...updates }),
    archiveSubscriptionPlan: async (id) => makePlan({ id, status: 'archived' }),
    listSubscriptions: async () => [makeSub()],
    getSubscription: async (id) => (id === 'nonexistent' ? null : makeSub({ id })),
    getSubscriptionByNumber: async (number) =>
      number === 'MISSING' || number === 'nonexistent'
        ? null
        : makeSub({ subscriptionNumber: number }),
    createSubscription: async (data) => makeSub({ id: 'sub_new', ...data }),
    pauseSubscription: async (id) => makeSub({ id, status: 'paused' }),
    updateSubscription: async (id, updates) => makeSub({ id, ...updates }),
    resumeSubscription: async (id) => makeSub({ id, status: 'active' }),
    cancelSubscription: async (id) => makeSub({ id, status: 'cancelled' }),
    skipBillingCycle: async (id) => makeSub({ id, nextBillingDate: '2026-04-20T00:00:00Z' }),
    listBillingCycles: async () => [makeCycle()],
    getBillingCycle: async (id) => (id === 'nonexistent' ? null : makeCycle({ id })),
    getSubscriptionEvents: async () => [makeEvent()],
    ...overrides,
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
    assert.strictEqual(plan.id, 'plan_001');
    assert.strictEqual(plan.name, 'Coffee Club Monthly');
    assert.strictEqual(plan.billingInterval, 'monthly');
    assert.strictEqual(plan.trialDays, 14);
  });

  it('passes status and billingInterval filters', async () => {
    let calledWith = {};
    const commerce = makeCommerce({
      listSubscriptionPlans: async (filters) => {
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

  it('returns plan by ID', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: { planId: 'plan_001' } });
    assert.strictEqual(result.success, true);
    assert.ok(result.plan);
  });

  it('falls back to plan code lookup when available', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { planId: 'COFFEE_MONTHLY' },
    });
    assert.strictEqual(result.success, true);
    assert.ok(result.plan);
  });

  it('returns error when plan not found', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { planId: 'nonexistent' },
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

  it('converts price to string before creating', async () => {
    let calledWith = {};
    const commerce = makeCommerce({
      createSubscriptionPlan: async (data) => {
        calledWith = data;
        return makePlan(data);
      },
    });
    await tool.handler({ commerce, params, allowApply: true });
    assert.strictEqual(calledWith.price, '49.99');
  });
});

// ---------------------------------------------------------------------------
// activate_subscription_plan
// ---------------------------------------------------------------------------

describe('activate_subscription_plan', () => {
  const tool = findTool('activate_subscription_plan');
  const params = { planId: 'plan_001' };

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
  const params = { planId: 'plan_001', updates: { name: 'Updated Plan' } };

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
});

// ---------------------------------------------------------------------------
// archive_subscription_plan
// ---------------------------------------------------------------------------

describe('archive_subscription_plan', () => {
  const tool = findTool('archive_subscription_plan');
  const params = { planId: 'plan_001' };

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
    assert.strictEqual(sub.id, 'sub_001');
    assert.strictEqual(sub.subscriptionNumber, 'SUB-100001');
    assert.strictEqual(sub.status, 'active');
  });

  it('passes filters to commerce', async () => {
    let calledWith = {};
    const commerce = makeCommerce({
      listSubscriptions: async (filters) => {
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
      params: { subscriptionId: 'sub_001' },
    });
    assert.ok(result);
    assert.strictEqual(result.id, 'sub_001');
  });

  it('returns error when subscription not found', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { subscriptionId: 'nonexistent' },
    });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.toLowerCase().includes('not found'));
  });

  it('falls back to subscription number lookup when available', async () => {
    const result = await tool.handler({
      commerce: makeCommerce({ getSubscription: async () => null }),
      params: { subscriptionId: 'SUB-100001' },
    });
    assert.ok(result);
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

  it('creates subscription when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.subscription);
    assert.ok(result.message.includes('SUB-'));
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
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('paused'));
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
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { ...params, immediate: true },
      allowApply: true,
    });
    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('immediately'));
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
    assert.strictEqual(cycle.id, 'cycle_001');
    assert.strictEqual(cycle.status, 'paid');
    assert.strictEqual(cycle.total, '29.99');
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
      params: { cycleId: 'cycle_001' },
    });
    assert.ok(result);
    assert.strictEqual(result.id, 'cycle_001');
  });

  it('returns error when cycle not found', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: { cycleId: 'nonexistent' },
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
    assert.strictEqual(evt.eventType, 'subscription.created');
  });

  it('passes limit parameter', async () => {
    let passedLimit;
    const commerce = makeCommerce({
      getSubscriptionEvents: async (_id, limit) => {
        passedLimit = limit;
        return [];
      },
    });
    await tool.handler({ commerce, params: { subscriptionId: 'sub_001', limit: 5 } });
    assert.strictEqual(passedLimit, 5);
  });
});
