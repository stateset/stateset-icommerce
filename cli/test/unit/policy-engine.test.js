/**
 * Tests for cli/src/policies/engine.js
 *
 * Covers: Operators, Condition, ConditionGroup, PolicyAction, PolicyRule,
 * PolicySet, PolicyResult, PolicyEngine, PolicyTemplates.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import {
  Operators,
  Condition,
  ConditionGroup,
  PolicyAction,
  PolicyRule,
  PolicySet,
  PolicyResult,
  PolicyEngine,
  PolicyTemplates,
} from '../../src/policies/engine.js';

// ---------------------------------------------------------------------------
// Operators
// ---------------------------------------------------------------------------

describe('Operators', () => {
  it('eq returns true for equal values', () => {
    assert.ok(Operators.eq(1, 1));
    assert.ok(Operators.eq('a', 'a'));
    assert.ok(!Operators.eq(1, 2));
  });

  it('neq returns true for unequal values', () => {
    assert.ok(Operators.neq(1, 2));
    assert.ok(!Operators.neq(1, 1));
  });

  it('gt / gte / lt / lte compare correctly', () => {
    assert.ok(Operators.gt(5, 3));
    assert.ok(!Operators.gt(3, 5));
    assert.ok(Operators.gte(5, 5));
    assert.ok(Operators.lt(3, 5));
    assert.ok(!Operators.lt(5, 3));
    assert.ok(Operators.lte(5, 5));
  });

  it('contains checks substring', () => {
    assert.ok(Operators.contains('hello world', 'world'));
    assert.ok(!Operators.contains('hello', 'world'));
  });

  it('startsWith / endsWith', () => {
    assert.ok(Operators.startsWith('hello', 'hel'));
    assert.ok(!Operators.startsWith('hello', 'llo'));
    assert.ok(Operators.endsWith('hello', 'llo'));
    assert.ok(!Operators.endsWith('hello', 'hel'));
  });

  it('matches tests regex pattern', () => {
    assert.ok(Operators.matches('hello123', '\\d+'));
    assert.ok(!Operators.matches('hello', '\\d+'));
  });

  it('matches rejects patterns over 200 chars', () => {
    const longPattern = 'a'.repeat(201);
    assert.ok(!Operators.matches('aaa', longPattern));
  });

  it('matches returns false for invalid regex', () => {
    assert.ok(!Operators.matches('test', '[invalid'));
  });

  it('in / notIn check array membership', () => {
    assert.ok(Operators.in('a', ['a', 'b', 'c']));
    assert.ok(!Operators.in('d', ['a', 'b']));
    assert.ok(Operators.notIn('d', ['a', 'b']));
    assert.ok(!Operators.notIn('a', ['a', 'b']));
    assert.ok(!Operators.in('a', 'not-array'));
    assert.ok(Operators.notIn('a', 'not-array'));
  });

  it('isEmpty / isNotEmpty', () => {
    assert.ok(Operators.isEmpty(null));
    assert.ok(Operators.isEmpty([]));
    assert.ok(Operators.isEmpty({}));
    assert.ok(!Operators.isEmpty([1]));
    assert.ok(!Operators.isEmpty({ a: 1 }));
    assert.ok(Operators.isNotEmpty([1]));
    assert.ok(!Operators.isNotEmpty(null));
  });

  it('isNull / isNotNull', () => {
    assert.ok(Operators.isNull(null));
    assert.ok(Operators.isNull(undefined));
    assert.ok(!Operators.isNull(0));
    assert.ok(Operators.isNotNull(0));
    assert.ok(!Operators.isNotNull(null));
  });

  it('isTrue / isFalse', () => {
    assert.ok(Operators.isTrue(true));
    assert.ok(!Operators.isTrue(1));
    assert.ok(Operators.isFalse(false));
    assert.ok(!Operators.isFalse(0));
  });

  it('between checks range', () => {
    assert.ok(Operators.between(5, [1, 10]));
    assert.ok(Operators.between(1, [1, 10]));
    assert.ok(Operators.between(10, [1, 10]));
    assert.ok(!Operators.between(0, [1, 10]));
  });

  it('divisibleBy checks modulo', () => {
    assert.ok(Operators.divisibleBy(10, 5));
    assert.ok(!Operators.divisibleBy(10, 3));
  });
});

// ---------------------------------------------------------------------------
// Condition
// ---------------------------------------------------------------------------

describe('Condition', () => {
  it('evaluates simple field comparison', () => {
    const cond = new Condition({ field: 'order.total', operator: 'gt', value: 100 });
    assert.ok(cond.evaluate({ order: { total: 150 } }));
    assert.ok(!cond.evaluate({ order: { total: 50 } }));
  });

  it('resolves dynamic reference values (${...})', () => {
    const cond = new Condition({
      field: 'inventory.quantity',
      operator: 'lte',
      value: '${inventory.reorderPoint}',
    });

    assert.ok(cond.evaluate({ inventory: { quantity: 3, reorderPoint: 5 } }));
    assert.ok(!cond.evaluate({ inventory: { quantity: 6, reorderPoint: 5 } }));
  });

  it('can compare a field against another field via dynamic reference', () => {
    const cond = new Condition({
      field: 'order.shippingAddress.country',
      operator: 'neq',
      value: '${order.billingAddress.country}',
    });

    assert.ok(
      !cond.evaluate({
        order: {
          shippingAddress: { country: 'US' },
          billingAddress: { country: 'US' },
        },
      })
    );

    assert.ok(
      cond.evaluate({
        order: {
          shippingAddress: { country: 'US' },
          billingAddress: { country: 'CA' },
        },
      })
    );
  });

  it('returns false when a dynamic reference value is missing (safe default)', () => {
    const cond = new Condition({
      field: 'order.shippingAddress.country',
      operator: 'neq',
      value: '${order.billingAddress.country}',
    });

    // Without a billing address, do not treat this as a mismatch.
    assert.ok(
      !cond.evaluate({
        order: { shippingAddress: { country: 'US' } },
      })
    );
  });

  it('does not short-circuit unary operators when value is a dynamic reference', () => {
    const cond = new Condition({ field: 'missing.path', operator: 'isNull', value: '${x.y.z}' });
    assert.ok(cond.evaluate({}));
  });

  it('supports dot-notation nested fields', () => {
    const cond = new Condition({ field: 'a.b.c', operator: 'eq', value: 42 });
    assert.ok(cond.evaluate({ a: { b: { c: 42 } } }));
    assert.ok(!cond.evaluate({ a: { b: { c: 0 } } }));
  });

  it('supports array index access', () => {
    const cond = new Condition({ field: 'items[0]', operator: 'eq', value: 'apple' });
    assert.ok(cond.evaluate({ items: ['apple', 'banana'] }));
  });

  it('returns undefined for missing paths', () => {
    const cond = new Condition({ field: 'missing.path', operator: 'isNull' });
    assert.ok(cond.evaluate({}));
  });

  it('supports negate', () => {
    const cond = new Condition({ field: 'x', operator: 'eq', value: 1, negate: true });
    assert.ok(cond.evaluate({ x: 2 }));
    assert.ok(!cond.evaluate({ x: 1 }));
  });

  it('throws on unknown operator', () => {
    const cond = new Condition({ field: 'x', operator: 'bogus', value: 1 });
    assert.throws(() => cond.evaluate({ x: 1 }), /Unknown operator/);
  });

  it('serializes to JSON', () => {
    const cond = new Condition({ field: 'x', operator: 'eq', value: 1 });
    const json = cond.toJSON();
    assert.equal(json.field, 'x');
    assert.equal(json.operator, 'eq');
    assert.equal(json.value, 1);
    assert.equal(json.negate, false);
  });
});

// ---------------------------------------------------------------------------
// ConditionGroup
// ---------------------------------------------------------------------------

describe('ConditionGroup', () => {
  it('AND logic requires all conditions', () => {
    const group = new ConditionGroup({
      logic: 'and',
      conditions: [
        { field: 'a', operator: 'eq', value: 1 },
        { field: 'b', operator: 'eq', value: 2 },
      ],
    });
    assert.ok(group.evaluate({ a: 1, b: 2 }));
    assert.ok(!group.evaluate({ a: 1, b: 3 }));
  });

  it('OR logic requires any condition', () => {
    const group = new ConditionGroup({
      logic: 'or',
      conditions: [
        { field: 'a', operator: 'eq', value: 1 },
        { field: 'b', operator: 'eq', value: 2 },
      ],
    });
    assert.ok(group.evaluate({ a: 1, b: 99 }));
    assert.ok(group.evaluate({ a: 99, b: 2 }));
    assert.ok(!group.evaluate({ a: 99, b: 99 }));
  });

  it('empty conditions evaluates to true', () => {
    const group = new ConditionGroup({ logic: 'and', conditions: [] });
    assert.ok(group.evaluate({}));
  });

  it('supports nested groups', () => {
    const group = new ConditionGroup({
      logic: 'and',
      conditions: [
        { field: 'x', operator: 'eq', value: 1 },
        {
          logic: 'or',
          conditions: [
            { field: 'y', operator: 'eq', value: 2 },
            { field: 'z', operator: 'eq', value: 3 },
          ],
        },
      ],
    });
    assert.ok(group.evaluate({ x: 1, y: 2, z: 0 }));
    assert.ok(group.evaluate({ x: 1, y: 0, z: 3 }));
    assert.ok(!group.evaluate({ x: 1, y: 0, z: 0 }));
    assert.ok(!group.evaluate({ x: 0, y: 2, z: 3 }));
  });

  it('serializes to JSON', () => {
    const group = new ConditionGroup({
      logic: 'or',
      conditions: [{ field: 'a', operator: 'eq', value: 1 }],
    });
    const json = group.toJSON();
    assert.equal(json.logic, 'or');
    assert.equal(json.conditions.length, 1);
  });
});

// ---------------------------------------------------------------------------
// PolicyAction
// ---------------------------------------------------------------------------

describe('PolicyAction', () => {
  it('stores all fields', () => {
    const action = new PolicyAction({
      type: 'agent',
      agent: 'returns',
      request: 'approve',
    });
    assert.equal(action.type, 'agent');
    assert.equal(action.agent, 'returns');
    assert.equal(action.request, 'approve');
  });

  it('serializes to JSON', () => {
    const action = new PolicyAction({ type: 'deny' });
    const json = action.toJSON();
    assert.equal(json.type, 'deny');
  });
});

// ---------------------------------------------------------------------------
// PolicyRule
// ---------------------------------------------------------------------------

describe('PolicyRule', () => {
  it('matches when conditions are met', () => {
    const rule = new PolicyRule({
      name: 'test',
      conditions: { field: 'x', operator: 'gt', value: 10 },
      action: { type: 'allow' },
    });
    assert.ok(rule.matches({ x: 20 }));
    assert.ok(!rule.matches({ x: 5 }));
  });

  it('does not match when disabled', () => {
    const rule = new PolicyRule({
      name: 'test',
      enabled: false,
      conditions: { field: 'x', operator: 'eq', value: 1 },
      action: { type: 'allow' },
    });
    assert.ok(!rule.matches({ x: 1 }));
  });

  it('accepts array of conditions (AND)', () => {
    const rule = new PolicyRule({
      name: 'test',
      conditions: [
        { field: 'a', operator: 'eq', value: 1 },
        { field: 'b', operator: 'eq', value: 2 },
      ],
      action: { type: 'allow' },
    });
    assert.ok(rule.matches({ a: 1, b: 2 }));
    assert.ok(!rule.matches({ a: 1, b: 0 }));
  });

  it('accepts ConditionGroup with logic', () => {
    const rule = new PolicyRule({
      name: 'test',
      conditions: {
        logic: 'or',
        conditions: [
          { field: 'a', operator: 'eq', value: 1 },
          { field: 'b', operator: 'eq', value: 2 },
        ],
      },
      action: { type: 'allow' },
    });
    assert.ok(rule.matches({ a: 1, b: 0 }));
  });

  it('serializes to JSON', () => {
    const rule = new PolicyRule({
      name: 'test',
      conditions: { field: 'x', operator: 'eq', value: 1 },
      action: { type: 'deny' },
    });
    const json = rule.toJSON();
    assert.equal(json.name, 'test');
    assert.equal(json.action.type, 'deny');
    assert.ok(json.id);
  });
});

// ---------------------------------------------------------------------------
// PolicySet
// ---------------------------------------------------------------------------

describe('PolicySet', () => {
  it('evaluates rules by priority', () => {
    const ps = new PolicySet({
      name: 'test',
      domain: 'orders',
      rules: [
        {
          name: 'low',
          priority: 10,
          conditions: { field: 'x', operator: 'gt', value: 0 },
          action: { type: 'allow' },
        },
        {
          name: 'high',
          priority: 100,
          conditions: { field: 'x', operator: 'gt', value: 0 },
          action: { type: 'deny' },
        },
      ],
    });
    const result = ps.evaluate({ x: 5 });
    assert.ok(result.matched);
    assert.equal(result.rules[0].name, 'high');
  });

  it('stops on stopOnMatch', () => {
    const ps = new PolicySet({
      name: 'test',
      domain: 'orders',
      rules: [
        {
          name: 'first',
          priority: 100,
          conditions: { field: 'x', operator: 'eq', value: 1 },
          action: { type: 'deny' },
          stopOnMatch: true,
        },
        {
          name: 'second',
          priority: 50,
          conditions: { field: 'x', operator: 'eq', value: 1 },
          action: { type: 'allow' },
        },
      ],
    });
    const result = ps.evaluate({ x: 1 });
    assert.equal(result.rules.length, 1);
    assert.equal(result.rules[0].name, 'first');
  });

  it('returns defaultApplied when no rules match', () => {
    const ps = new PolicySet({
      name: 'test',
      domain: 'orders',
      rules: [
        {
          name: 'never',
          conditions: { field: 'x', operator: 'eq', value: 999 },
          action: { type: 'deny' },
        },
      ],
    });
    const result = ps.evaluate({ x: 1 });
    assert.ok(!result.matched);
    assert.ok(result.defaultApplied);
  });

  it('serializes to JSON', () => {
    const ps = new PolicySet({
      name: 'test',
      domain: 'inventory',
      rules: [],
    });
    const json = ps.toJSON();
    assert.equal(json.name, 'test');
    assert.equal(json.domain, 'inventory');
  });
});

// ---------------------------------------------------------------------------
// PolicyResult
// ---------------------------------------------------------------------------

describe('PolicyResult', () => {
  it('stores all fields', () => {
    const result = new PolicyResult({
      policySetId: 'ps1',
      policySetName: 'Test',
      domain: 'orders',
      context: {},
      matched: true,
      rules: [],
      actions: [],
      defaultApplied: false,
    });
    assert.equal(result.policySetId, 'ps1');
    assert.ok(result.evaluatedAt);
  });
});

// ---------------------------------------------------------------------------
// PolicyEngine
// ---------------------------------------------------------------------------

describe('PolicyEngine', () => {
  let engine;

  beforeEach(() => {
    engine = new PolicyEngine({ storePath: null });
  });

  it('registers and evaluates policy sets', async () => {
    engine.registerPolicySet({
      name: 'Returns Policy',
      domain: 'returns',
      rules: [
        {
          name: 'small_return',
          conditions: { field: 'return.value', operator: 'lt', value: 100 },
          action: { type: 'allow' },
        },
      ],
    });

    const result = await engine.evaluate('returns', { return: { value: 50 } });
    assert.ok(!result.shouldDeny);
    assert.ok(result.actions.length > 0);
  });

  it('evaluates deny actions correctly', async () => {
    engine.registerPolicySet({
      name: 'Block Policy',
      domain: 'orders',
      rules: [
        {
          name: 'block_fraud',
          conditions: { field: 'order.total', operator: 'gt', value: 1000 },
          action: { type: 'deny' },
        },
      ],
    });

    const result = await engine.evaluate('orders', { order: { total: 5000 } });
    assert.ok(result.shouldDeny);
  });

  it('applies default action when no rules match', async () => {
    engine.registerPolicySet({
      name: 'Test',
      domain: 'orders',
      rules: [
        {
          name: 'never',
          conditions: { field: 'x', operator: 'eq', value: 999 },
          action: { type: 'deny' },
        },
      ],
      defaultAction: { type: 'allow' },
    });

    const result = await engine.evaluate('orders', { x: 1 });
    assert.ok(!result.shouldDeny);
    assert.equal(result.actions.length, 1);
    assert.equal(result.actions[0].type, 'allow');
  });

  it('evaluateAndExecute runs executor for non-deny actions', async () => {
    const executed = [];
    const eng = new PolicyEngine({
      storePath: null,
      executor: (action, ctx) => {
        executed.push({ action, ctx });
        return 'done';
      },
    });

    eng.registerPolicySet({
      name: 'Test',
      domain: 'orders',
      rules: [
        {
          name: 'agent_action',
          conditions: { field: 'x', operator: 'eq', value: 1 },
          action: { type: 'agent', agent: 'orders', request: 'process' },
        },
      ],
    });

    const result = await eng.evaluateAndExecute('orders', { x: 1 });
    assert.ok(result.allowed);
    assert.equal(result.executed.length, 1);
    assert.ok(result.executed[0].success);
    assert.equal(executed.length, 1);
  });

  it('evaluateAndExecute blocks on deny', async () => {
    engine.registerPolicySet({
      name: 'Test',
      domain: 'orders',
      rules: [
        {
          name: 'block',
          conditions: { field: 'x', operator: 'eq', value: 1 },
          action: { type: 'deny' },
        },
      ],
    });

    const result = await engine.evaluateAndExecute('orders', { x: 1 });
    assert.ok(!result.allowed);
  });

  it('tracks evaluation history', async () => {
    engine.registerPolicySet({
      name: 'Test',
      domain: 'test',
      rules: [],
    });

    await engine.evaluate('test', { a: 1 });
    await engine.evaluate('test', { a: 2 });

    const history = engine.getHistory();
    assert.equal(history.length, 2);
  });

  it('getHistory filters by domain', async () => {
    engine.registerPolicySet({ name: 'A', domain: 'a', rules: [] });
    engine.registerPolicySet({ name: 'B', domain: 'b', rules: [] });

    await engine.evaluate('a', {});
    await engine.evaluate('b', {});

    assert.equal(engine.getHistory({ domain: 'a' }).length, 1);
    assert.equal(engine.getHistory({ domain: 'b' }).length, 1);
  });

  it('getHistory respects limit', async () => {
    engine.registerPolicySet({ name: 'X', domain: 'x', rules: [] });
    for (let i = 0; i < 10; i++) {
      await engine.evaluate('x', { i });
    }
    assert.equal(engine.getHistory({ limit: 3 }).length, 3);
  });

  it('trims history to 1000 entries', async () => {
    engine.registerPolicySet({ name: 'X', domain: 'x', rules: [] });
    for (let i = 0; i < 1010; i++) {
      engine.evaluationHistory.push({ domain: 'x', i });
    }
    await engine.evaluate('x', {});
    assert.ok(engine.evaluationHistory.length <= 1001);
  });

  it('getPoliciesForDomain returns empty for unknown domain', () => {
    assert.deepStrictEqual(engine.getPoliciesForDomain('unknown'), []);
  });

  it('listPolicySets returns all policy sets', () => {
    engine.registerPolicySet({ name: 'A', domain: 'a', rules: [] });
    engine.registerPolicySet({ name: 'B', domain: 'b', rules: [] });
    const list = engine.listPolicySets();
    assert.equal(list.length, 2);
  });

  it('getStatus returns counts', () => {
    engine.registerPolicySet({
      name: 'A',
      domain: 'a',
      rules: [
        {
          name: 'r1',
          conditions: { field: 'x', operator: 'eq', value: 1 },
          action: { type: 'allow' },
        },
      ],
    });

    const status = engine.getStatus();
    assert.equal(status.totalPolicySets, 1);
    assert.equal(status.totalRules, 1);
    assert.ok(status.byDomain.a);
  });

  it('emits events', async () => {
    const events = [];
    engine.on('policySet:registered', (e) => events.push(e));
    engine.on('evaluated', (e) => events.push(e));

    engine.registerPolicySet({ name: 'X', domain: 'x', rules: [] });
    await engine.evaluate('x', {});

    assert.equal(events.length, 2);
  });
});

// ---------------------------------------------------------------------------
// PolicyTemplates
// ---------------------------------------------------------------------------

describe('PolicyTemplates', () => {
  it('has expected template keys', () => {
    const expected = [
      'autoApproveReturns',
      'inventoryRestock',
      'orderFraudDetection',
      'promotionEligibility',
      'subscriptionRules',
    ];
    for (const key of expected) {
      assert.ok(key in PolicyTemplates, `missing template: ${key}`);
    }
  });

  it('templates can be loaded as PolicySets', () => {
    for (const [key, template] of Object.entries(PolicyTemplates)) {
      const ps = new PolicySet(template);
      assert.ok(ps.name, `template ${key} missing name`);
      assert.ok(ps.domain, `template ${key} missing domain`);
      assert.ok(ps.rules.length > 0, `template ${key} has no rules`);
    }
  });
});
