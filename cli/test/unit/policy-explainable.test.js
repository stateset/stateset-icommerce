/**
 * Tests for explainable policy features in cli/src/policies/engine.js
 *
 * Covers: PolicyExplanation, TransformAuditEntry, evaluateWithDetail(),
 * matchesWithDetail(), deny-overrides precedence, evaluateDryRun(),
 * PolicyAction reason/remediation, backward compatibility, integration
 * scenarios, and applyPolicyTransform audit.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import {
  PolicyEngine,
  PolicySet,
  PolicyRule,
  PolicyAction,
  Condition,
  ConditionGroup,
  PolicyExplanation,
  TransformAuditEntry,
  Operators,
  PolicyResult,
  PolicyTemplates,
} from '../../src/policies/engine.js';

// ---------------------------------------------------------------------------
// PolicyExplanation
// ---------------------------------------------------------------------------

describe('PolicyExplanation', () => {
  it('constructor stores all fields', () => {
    const conditions = [
      { field: 'order.total', operator: 'gt', expectedValue: 100, actualValue: 250, matched: true },
    ];
    const explanation = new PolicyExplanation({
      policySetId: 'ps-1',
      policySetName: 'Order Limits',
      ruleId: 'r-1',
      ruleName: 'high_value_order',
      ruleDescription: 'Flag high-value orders',
      actionType: 'deny',
      reason: 'Order exceeds $100 limit',
      remediation: 'Split order into smaller amounts',
      conditions,
    });

    assert.equal(explanation.policySetId, 'ps-1');
    assert.equal(explanation.policySetName, 'Order Limits');
    assert.equal(explanation.ruleId, 'r-1');
    assert.equal(explanation.ruleName, 'high_value_order');
    assert.equal(explanation.ruleDescription, 'Flag high-value orders');
    assert.equal(explanation.actionType, 'deny');
    assert.equal(explanation.reason, 'Order exceeds $100 limit');
    assert.equal(explanation.remediation, 'Split order into smaller amounts');
    assert.deepStrictEqual(explanation.conditions, conditions);
  });

  it('toString() produces readable output with reason, conditions, remediation', () => {
    const explanation = new PolicyExplanation({
      policySetId: 'ps-2',
      policySetName: 'Fraud Detection',
      ruleId: 'r-2',
      ruleName: 'velocity_check',
      ruleDescription: 'Multiple orders in short time',
      actionType: 'deny',
      reason: 'Too many orders in 24h',
      remediation: 'Wait 24 hours before placing another order',
      conditions: [
        { field: 'customer.ordersLast24h', operator: 'gt', expectedValue: 3, actualValue: 5, matched: true },
      ],
    });

    const str = explanation.toString();
    assert.ok(str.includes('Fraud Detection'), 'should include policy set name');
    assert.ok(str.includes('velocity_check'), 'should include rule name');
    assert.ok(str.includes('deny'), 'should include action type');
    assert.ok(str.includes('Too many orders in 24h'), 'should include reason');
    assert.ok(str.includes('customer.ordersLast24h'), 'should include condition field');
    assert.ok(str.includes('gt'), 'should include operator');
    assert.ok(str.includes('Wait 24 hours'), 'should include remediation');
    assert.ok(str.includes('matched: true'), 'should include matched status');
  });

  it('toJSON() returns plain object with all fields', () => {
    const explanation = new PolicyExplanation({
      policySetId: 'ps-3',
      policySetName: 'Stock Policy',
      ruleId: 'r-3',
      ruleName: 'low_stock',
      ruleDescription: 'Low stock alert',
      actionType: 'notify',
      reason: 'Stock below threshold',
      remediation: 'Reorder from supplier',
      conditions: [{ field: 'stock', operator: 'lt', expectedValue: 10, actualValue: 3, matched: true }],
    });

    const json = explanation.toJSON();
    assert.equal(json.policySetId, 'ps-3');
    assert.equal(json.policySetName, 'Stock Policy');
    assert.equal(json.ruleId, 'r-3');
    assert.equal(json.ruleName, 'low_stock');
    assert.equal(json.ruleDescription, 'Low stock alert');
    assert.equal(json.actionType, 'notify');
    assert.equal(json.reason, 'Stock below threshold');
    assert.equal(json.remediation, 'Reorder from supplier');
    assert.equal(json.conditions.length, 1);
    // Should be a plain object, not a class instance
    assert.equal(typeof json, 'object');
    assert.ok(!(json instanceof PolicyExplanation));
  });

  it('empty conditions array works', () => {
    const explanation = new PolicyExplanation({
      policySetId: 'ps-4',
      policySetName: 'Simple',
      ruleId: 'r-4',
      ruleName: 'blanket_allow',
      actionType: 'allow',
      conditions: [],
    });

    assert.deepStrictEqual(explanation.conditions, []);
    const str = explanation.toString();
    assert.ok(str.includes('Simple'), 'should still produce valid string');
    assert.ok(!str.includes('Remediation'), 'should not include remediation line when null');

    const json = explanation.toJSON();
    assert.deepStrictEqual(json.conditions, []);
  });
});

// ---------------------------------------------------------------------------
// TransformAuditEntry
// ---------------------------------------------------------------------------

describe('TransformAuditEntry', () => {
  it('constructor stores all fields + auto-generates timestamp', () => {
    const before = Date.now();
    const entry = new TransformAuditEntry({
      ruleId: 'r-10',
      ruleName: 'cap_discount',
      policySetId: 'ps-10',
      field: 'discount.percentage',
      before: 50,
      after: 20,
    });

    assert.equal(entry.ruleId, 'r-10');
    assert.equal(entry.ruleName, 'cap_discount');
    assert.equal(entry.policySetId, 'ps-10');
    assert.equal(entry.field, 'discount.percentage');
    assert.equal(entry.before, 50);
    assert.equal(entry.after, 20);
    assert.ok(entry.timestamp, 'timestamp should be set');

    // Timestamp should be a valid ISO string close to now
    const ts = new Date(entry.timestamp).getTime();
    assert.ok(ts >= before, 'timestamp should be at or after creation time');
    assert.ok(ts <= Date.now() + 1000, 'timestamp should be within 1s of now');
  });

  it('toJSON() returns plain object', () => {
    const entry = new TransformAuditEntry({
      ruleId: 'r-11',
      ruleName: 'normalize_currency',
      policySetId: 'ps-11',
      field: 'order.currency',
      before: 'usd',
      after: 'USD',
    });

    const json = entry.toJSON();
    assert.equal(json.ruleId, 'r-11');
    assert.equal(json.ruleName, 'normalize_currency');
    assert.equal(json.policySetId, 'ps-11');
    assert.equal(json.field, 'order.currency');
    assert.equal(json.before, 'usd');
    assert.equal(json.after, 'USD');
    assert.ok(json.timestamp);
    assert.equal(typeof json, 'object');
    assert.ok(!(json instanceof TransformAuditEntry));
  });
});

// ---------------------------------------------------------------------------
// Condition.evaluateWithDetail
// ---------------------------------------------------------------------------

describe('Condition.evaluateWithDetail', () => {
  it('simple match returns matched=true with field values', () => {
    const cond = new Condition({ field: 'order.total', operator: 'gt', value: 100 });
    const detail = cond.evaluateWithDetail({ order: { total: 250 } });

    assert.equal(detail.matched, true);
    assert.equal(detail.field, 'order.total');
    assert.equal(detail.operator, 'gt');
    assert.equal(detail.expectedValue, 100);
    assert.equal(detail.actualValue, 250);
  });

  it('mismatch returns matched=false with actual vs expected', () => {
    const cond = new Condition({ field: 'order.total', operator: 'gt', value: 100 });
    const detail = cond.evaluateWithDetail({ order: { total: 50 } });

    assert.equal(detail.matched, false);
    assert.equal(detail.field, 'order.total');
    assert.equal(detail.operator, 'gt');
    assert.equal(detail.expectedValue, 100);
    assert.equal(detail.actualValue, 50);
  });

  it('dynamic ref (${...}) resolution works', () => {
    const cond = new Condition({
      field: 'inventory.quantity',
      operator: 'lte',
      value: '${inventory.reorderPoint}',
    });
    const detail = cond.evaluateWithDetail({ inventory: { quantity: 3, reorderPoint: 10 } });

    assert.equal(detail.matched, true);
    assert.equal(detail.field, 'inventory.quantity');
    assert.equal(detail.operator, 'lte');
    // expectedValue should be the resolved value, not the template string
    assert.equal(detail.expectedValue, 10);
    assert.equal(detail.actualValue, 3);
  });

  it('unary operator (isNull) returns expectedValue=null', () => {
    const cond = new Condition({ field: 'customer.email', operator: 'isNull' });
    const detail = cond.evaluateWithDetail({ customer: {} });

    assert.equal(detail.matched, true);
    assert.equal(detail.field, 'customer.email');
    assert.equal(detail.operator, 'isNull');
    assert.equal(detail.expectedValue, null);
    assert.equal(detail.actualValue, undefined);
  });

  it('negate flag inverts matched', () => {
    const cond = new Condition({ field: 'x', operator: 'eq', value: 1, negate: true });

    const matchDetail = cond.evaluateWithDetail({ x: 1 });
    assert.equal(matchDetail.matched, false, 'negate should invert a true result to false');

    const mismatchDetail = cond.evaluateWithDetail({ x: 2 });
    assert.equal(mismatchDetail.matched, true, 'negate should invert a false result to true');
    assert.equal(mismatchDetail.actualValue, 2);
    assert.equal(mismatchDetail.expectedValue, 1);
  });

  it('throws on unknown operator', () => {
    const cond = new Condition({ field: 'x', operator: 'bogus', value: 1 });
    assert.throws(() => cond.evaluateWithDetail({ x: 1 }), /Unknown operator/);
  });

  it('missing dynamic ref returns matched=false with original template as expectedValue', () => {
    const cond = new Condition({
      field: 'order.shippingAddress.country',
      operator: 'neq',
      value: '${order.billingAddress.country}',
    });

    const detail = cond.evaluateWithDetail({ order: { shippingAddress: { country: 'US' } } });
    assert.equal(detail.matched, false);
    // expectedValue should be the original template string since the ref resolved to undefined
    assert.equal(detail.expectedValue, '${order.billingAddress.country}');
    assert.equal(detail.actualValue, 'US');
  });
});

// ---------------------------------------------------------------------------
// ConditionGroup.evaluateWithDetail
// ---------------------------------------------------------------------------

describe('ConditionGroup.evaluateWithDetail', () => {
  it('AND group: all must match', () => {
    const group = new ConditionGroup({
      logic: 'and',
      conditions: [
        { field: 'a', operator: 'eq', value: 1 },
        { field: 'b', operator: 'eq', value: 2 },
      ],
    });

    // All match
    const allMatch = group.evaluateWithDetail({ a: 1, b: 2 });
    assert.equal(allMatch.matched, true);
    assert.equal(allMatch.details.length, 2);
    assert.ok(allMatch.details.every((d) => d.matched));

    // One does not match
    const partial = group.evaluateWithDetail({ a: 1, b: 99 });
    assert.equal(partial.matched, false);
    assert.equal(partial.details.length, 2);
    assert.equal(partial.details[0].matched, true);
    assert.equal(partial.details[1].matched, false);
  });

  it('OR group: at least one matches', () => {
    const group = new ConditionGroup({
      logic: 'or',
      conditions: [
        { field: 'a', operator: 'eq', value: 1 },
        { field: 'b', operator: 'eq', value: 2 },
      ],
    });

    // One matches
    const oneMatch = group.evaluateWithDetail({ a: 1, b: 99 });
    assert.equal(oneMatch.matched, true);
    assert.equal(oneMatch.details.length, 2);
    assert.equal(oneMatch.details[0].matched, true);
    assert.equal(oneMatch.details[1].matched, false);

    // None match
    const noneMatch = group.evaluateWithDetail({ a: 99, b: 99 });
    assert.equal(noneMatch.matched, false);
    assert.ok(noneMatch.details.every((d) => !d.matched));
  });

  it('empty group returns matched=true, details=[]', () => {
    const group = new ConditionGroup({ logic: 'and', conditions: [] });
    const result = group.evaluateWithDetail({});

    assert.equal(result.matched, true);
    assert.deepStrictEqual(result.details, []);
  });

  it('nested group propagates detail from inner groups', () => {
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

    const result = group.evaluateWithDetail({ x: 1, y: 2, z: 0 });
    assert.equal(result.matched, true);
    assert.equal(result.details.length, 2);
    // Second detail is the nested group result
    const nestedDetail = result.details[1];
    assert.ok('details' in nestedDetail, 'nested group should have its own details array');
    assert.equal(nestedDetail.matched, true);
  });
});

// ---------------------------------------------------------------------------
// PolicyRule.matchesWithDetail
// ---------------------------------------------------------------------------

describe('PolicyRule.matchesWithDetail', () => {
  it('matching rule returns condition details', () => {
    const rule = new PolicyRule({
      name: 'high_value',
      conditions: {
        logic: 'and',
        conditions: [
          { field: 'order.total', operator: 'gt', value: 100 },
          { field: 'customer.tier', operator: 'eq', value: 'vip' },
        ],
      },
      action: { type: 'deny' },
    });

    const result = rule.matchesWithDetail({ order: { total: 500 }, customer: { tier: 'vip' } });
    assert.equal(result.matched, true);
    assert.ok(result.conditionDetails.length > 0);
    assert.ok(result.conditionDetails.every((d) => d.matched));
  });

  it('non-matching rule returns matched=false', () => {
    const rule = new PolicyRule({
      name: 'high_value',
      conditions: { field: 'order.total', operator: 'gt', value: 1000 },
      action: { type: 'deny' },
    });

    const result = rule.matchesWithDetail({ order: { total: 50 } });
    assert.equal(result.matched, false);
    assert.ok(result.conditionDetails.length > 0);
    assert.equal(result.conditionDetails[0].matched, false);
  });

  it('disabled rule returns matched=false, empty conditionDetails', () => {
    const rule = new PolicyRule({
      name: 'disabled_rule',
      enabled: false,
      conditions: { field: 'x', operator: 'eq', value: 1 },
      action: { type: 'deny' },
    });

    const result = rule.matchesWithDetail({ x: 1 });
    assert.equal(result.matched, false);
    assert.deepStrictEqual(result.conditionDetails, []);
  });
});

// ---------------------------------------------------------------------------
// PolicySet explainable evaluate
// ---------------------------------------------------------------------------

describe('PolicySet explainable evaluate', () => {
  it('deny rule produces explanation with reason/remediation', () => {
    const ps = new PolicySet({
      name: 'Order Limits',
      domain: 'orders',
      rules: [
        {
          name: 'block_huge_orders',
          description: 'Block orders over $10k',
          conditions: { field: 'order.total', operator: 'gt', value: 10000 },
          action: {
            type: 'deny',
            reason: 'Order exceeds maximum allowed value',
            remediation: 'Contact sales for enterprise orders',
          },
        },
      ],
    });

    const result = ps.evaluate({ order: { total: 15000 } });
    assert.ok(result.matched);
    assert.equal(result.explanations.length, 1);

    const expl = result.explanations[0];
    assert.equal(expl.policySetName, 'Order Limits');
    assert.equal(expl.ruleName, 'block_huge_orders');
    assert.equal(expl.actionType, 'deny');
    assert.ok(expl.reason.includes('exceeds maximum'));
    assert.ok(expl.remediation.includes('Contact sales'));
    assert.ok(expl.conditions.length > 0);
  });

  it('allow rule produces explanation', () => {
    const ps = new PolicySet({
      name: 'VIP Access',
      domain: 'promotions',
      rules: [
        {
          name: 'vip_approved',
          description: 'VIP customers are always approved',
          conditions: { field: 'customer.tier', operator: 'eq', value: 'vip' },
          action: {
            type: 'allow',
            reason: 'VIP customer auto-approved',
          },
        },
      ],
    });

    const result = ps.evaluate({ customer: { tier: 'vip' } });
    assert.ok(result.matched);
    assert.equal(result.explanations.length, 1);
    assert.equal(result.explanations[0].actionType, 'allow');
    assert.ok(result.explanations[0].reason.includes('VIP'));
  });

  it('transform rule produces explanation', () => {
    const ps = new PolicySet({
      name: 'Price Cap',
      domain: 'orders',
      rules: [
        {
          name: 'cap_discount',
          description: 'Cap discount at 30%',
          conditions: { field: 'discount.percentage', operator: 'gt', value: 30 },
          action: {
            type: 'transform',
            transform: { field: 'discount.percentage', value: 30 },
            reason: 'Discount exceeds maximum allowed percentage',
          },
        },
      ],
    });

    const result = ps.evaluate({ discount: { percentage: 50 } });
    assert.ok(result.matched);
    assert.equal(result.explanations.length, 1);
    assert.equal(result.explanations[0].actionType, 'transform');
    assert.ok(result.explanations[0].reason.includes('Discount exceeds'));
  });

  it('no match returns empty explanations, defaultApplied=true', () => {
    const ps = new PolicySet({
      name: 'Niche Policy',
      domain: 'orders',
      rules: [
        {
          name: 'never_matches',
          conditions: { field: 'x', operator: 'eq', value: 'impossible' },
          action: { type: 'deny' },
        },
      ],
    });

    const result = ps.evaluate({ x: 42 });
    assert.equal(result.matched, false);
    assert.deepStrictEqual(result.explanations, []);
    assert.equal(result.defaultApplied, true);
  });
});

// ---------------------------------------------------------------------------
// PolicyEngine deny-overrides
// ---------------------------------------------------------------------------

describe('PolicyEngine deny-overrides', () => {
  let engine;

  beforeEach(() => {
    engine = new PolicyEngine({ storePath: null });
  });

  it('deny from one set + allow from another = shouldDeny=true', async () => {
    engine.registerPolicySet({
      name: 'Allow Policy',
      domain: 'orders',
      rules: [
        {
          name: 'allow_all',
          conditions: { field: 'order.total', operator: 'gt', value: 0 },
          action: { type: 'allow' },
        },
      ],
    });

    engine.registerPolicySet({
      name: 'Deny Policy',
      domain: 'orders',
      rules: [
        {
          name: 'block_big',
          conditions: { field: 'order.total', operator: 'gt', value: 500 },
          action: { type: 'deny', reason: 'Order too large' },
        },
      ],
    });

    const result = await engine.evaluate('orders', { order: { total: 1000 } });
    assert.equal(result.shouldDeny, true, 'deny should override allow');
    assert.equal(result.shouldAllow, false);
  });

  it('allow from both sets = shouldAllow=true, shouldDeny=false', async () => {
    engine.registerPolicySet({
      name: 'Policy A',
      domain: 'orders',
      rules: [
        {
          name: 'allow_a',
          conditions: { field: 'order.total', operator: 'gt', value: 0 },
          action: { type: 'allow' },
        },
      ],
    });

    engine.registerPolicySet({
      name: 'Policy B',
      domain: 'orders',
      rules: [
        {
          name: 'allow_b',
          conditions: { field: 'order.status', operator: 'eq', value: 'pending' },
          action: { type: 'allow' },
        },
      ],
    });

    const result = await engine.evaluate('orders', { order: { total: 100, status: 'pending' } });
    assert.equal(result.shouldAllow, true);
    assert.equal(result.shouldDeny, false);
  });

  it('multiple denies aggregated in explanations', async () => {
    engine.registerPolicySet({
      name: 'Fraud Check',
      domain: 'orders',
      rules: [
        {
          name: 'velocity_deny',
          conditions: { field: 'customer.ordersLast24h', operator: 'gt', value: 3 },
          action: { type: 'deny', reason: 'Velocity limit exceeded' },
        },
      ],
    });

    engine.registerPolicySet({
      name: 'Spend Limit',
      domain: 'orders',
      rules: [
        {
          name: 'spend_deny',
          conditions: { field: 'order.total', operator: 'gt', value: 5000 },
          action: { type: 'deny', reason: 'Spend limit exceeded' },
        },
      ],
    });

    const result = await engine.evaluate('orders', {
      customer: { ordersLast24h: 10 },
      order: { total: 9999 },
    });

    assert.equal(result.shouldDeny, true);
    assert.equal(result.explanations.length, 2);
    const reasons = result.explanations.map((e) => e.reason);
    assert.ok(reasons.some((r) => r.includes('Velocity')));
    assert.ok(reasons.some((r) => r.includes('Spend')));
  });

  it('single deny in single set', async () => {
    engine.registerPolicySet({
      name: 'Simple Deny',
      domain: 'orders',
      rules: [
        {
          name: 'block_zero',
          conditions: { field: 'order.total', operator: 'lte', value: 0 },
          action: { type: 'deny', reason: 'Zero or negative total not allowed' },
        },
      ],
    });

    const result = await engine.evaluate('orders', { order: { total: -5 } });
    assert.equal(result.shouldDeny, true);
    assert.equal(result.explanations.length, 1);
    assert.equal(result.explanations[0].reason, 'Zero or negative total not allowed');
  });
});

// ---------------------------------------------------------------------------
// PolicyEngine evaluateDryRun
// ---------------------------------------------------------------------------

describe('PolicyEngine evaluateDryRun', () => {
  let engine;

  beforeEach(() => {
    engine = new PolicyEngine({ storePath: null });
    engine.registerPolicySet({
      name: 'DryRun Test',
      domain: 'orders',
      rules: [
        {
          name: 'block_large',
          conditions: { field: 'order.total', operator: 'gt', value: 1000 },
          action: { type: 'deny', reason: 'Order too large for dry-run test' },
        },
      ],
    });
  });

  it('returns explanations', async () => {
    const result = await engine.evaluateDryRun('orders', { order: { total: 5000 } });
    assert.equal(result.shouldDeny, true);
    assert.equal(result.explanations.length, 1);
    assert.ok(result.explanations[0].reason.includes('dry-run'));
    assert.equal(result.dryRun, true);
  });

  it('does NOT record in evaluationHistory', async () => {
    const historyBefore = engine.getHistory().length;
    await engine.evaluateDryRun('orders', { order: { total: 5000 } });
    const historyAfter = engine.getHistory().length;

    assert.equal(historyAfter, historyBefore, 'dry-run should not add history entries');
  });

  it('returns context in result', async () => {
    const ctx = { order: { total: 5000 } };
    const result = await engine.evaluateDryRun('orders', ctx);
    assert.deepStrictEqual(result.context, ctx);
  });

  it('regular evaluate does NOT return context', async () => {
    const result = await engine.evaluate('orders', { order: { total: 5000 } });
    assert.equal(result.context, undefined);
  });
});

// ---------------------------------------------------------------------------
// PolicyAction reason/remediation
// ---------------------------------------------------------------------------

describe('PolicyAction reason/remediation', () => {
  it('fields stored correctly', () => {
    const action = new PolicyAction({
      type: 'deny',
      reason: 'Exceeded spending limit',
      remediation: 'Request a limit increase from your admin',
    });

    assert.equal(action.reason, 'Exceeded spending limit');
    assert.equal(action.remediation, 'Request a limit increase from your admin');
  });

  it('serialized in toJSON()', () => {
    const action = new PolicyAction({
      type: 'deny',
      reason: 'Flagged for review',
      remediation: 'Submit identity verification',
    });

    const json = action.toJSON();
    assert.equal(json.reason, 'Flagged for review');
    assert.equal(json.remediation, 'Submit identity verification');
    assert.equal(json.type, 'deny');
  });

  it('default to null when not provided', () => {
    const action = new PolicyAction({ type: 'allow' });
    assert.equal(action.reason, null);
    assert.equal(action.remediation, null);

    const json = action.toJSON();
    assert.equal(json.reason, null);
    assert.equal(json.remediation, null);
  });
});

// ---------------------------------------------------------------------------
// Backward compatibility
// ---------------------------------------------------------------------------

describe('Backward compatibility', () => {
  it('evaluate() still returns matched, rules, actions, defaultApplied', () => {
    const ps = new PolicySet({
      name: 'Compat Test',
      domain: 'orders',
      rules: [
        {
          name: 'basic_rule',
          conditions: { field: 'x', operator: 'eq', value: 1 },
          action: { type: 'allow' },
        },
      ],
    });

    const result = ps.evaluate({ x: 1 });
    // Original fields must still be present
    assert.ok('matched' in result, 'result should have matched');
    assert.ok('rules' in result, 'result should have rules');
    assert.ok('actions' in result, 'result should have actions');
    assert.ok('defaultApplied' in result, 'result should have defaultApplied');
    // New field should also be present
    assert.ok('explanations' in result, 'result should also have explanations');

    assert.equal(result.matched, true);
    assert.equal(result.rules.length, 1);
    assert.equal(result.actions.length, 1);
    assert.equal(result.defaultApplied, false);
  });

  it('PolicyTemplates still load and evaluate correctly', () => {
    const engine = new PolicyEngine({ storePath: null });

    for (const [key, template] of Object.entries(PolicyTemplates)) {
      const ps = engine.registerPolicySet(template);
      assert.ok(ps instanceof PolicySet, `template ${key} should register as PolicySet`);
    }

    // Evaluate the auto-approve returns template
    const returnsPs = engine.getPoliciesForDomain('returns')[0];
    const result = returnsPs.evaluate({
      return: { value: 50 },
      customer: { lifetimeValue: 1000, returnRate: 0.02 },
    });
    assert.ok(result.matched);
    assert.ok(result.explanations.length > 0, 'template evaluation should produce explanations');
  });

  it('PolicyResult still stores all legacy fields', () => {
    const result = new PolicyResult({
      policySetId: 'ps-compat',
      policySetName: 'Compat',
      domain: 'orders',
      context: { x: 1 },
      matched: true,
      rules: [{ id: 'r1', name: 'test' }],
      actions: [{ type: 'allow' }],
      defaultApplied: false,
    });

    assert.equal(result.policySetId, 'ps-compat');
    assert.equal(result.policySetName, 'Compat');
    assert.equal(result.domain, 'orders');
    assert.ok(result.evaluatedAt);
    assert.equal(result.matched, true);
    assert.equal(result.defaultApplied, false);
  });
});

// ---------------------------------------------------------------------------
// Integration: evaluatePolicy structured denial
// ---------------------------------------------------------------------------

describe('Integration: evaluatePolicy structured denial', () => {
  let engine;

  beforeEach(() => {
    engine = new PolicyEngine({ storePath: null });
  });

  it('deny returns reason string from action.reason', async () => {
    engine.registerPolicySet({
      name: 'Denial Policy',
      domain: 'orders',
      rules: [
        {
          name: 'deny_blacklisted',
          conditions: { field: 'customer.blacklisted', operator: 'isTrue' },
          action: { type: 'deny', reason: 'Customer is blacklisted' },
        },
      ],
    });

    const result = await engine.evaluate('orders', { customer: { blacklisted: true } });
    assert.equal(result.shouldDeny, true);
    assert.equal(result.explanations[0].reason, 'Customer is blacklisted');
  });

  it('deny returns remediation from action.remediation', async () => {
    engine.registerPolicySet({
      name: 'KYC Policy',
      domain: 'payments',
      rules: [
        {
          name: 'unverified_deny',
          conditions: { field: 'customer.verified', operator: 'isFalse' },
          action: {
            type: 'deny',
            reason: 'Identity not verified',
            remediation: 'Complete KYC verification at /verify',
          },
        },
      ],
    });

    const result = await engine.evaluate('payments', { customer: { verified: false } });
    assert.equal(result.explanations[0].remediation, 'Complete KYC verification at /verify');
  });

  it('deny returns explanations array', async () => {
    engine.registerPolicySet({
      name: 'Multi-Rule',
      domain: 'orders',
      rules: [
        {
          name: 'rule_a',
          conditions: { field: 'x', operator: 'eq', value: 1 },
          action: { type: 'deny', reason: 'Reason A' },
        },
      ],
    });

    const result = await engine.evaluate('orders', { x: 1 });
    assert.ok(Array.isArray(result.explanations));
    assert.equal(result.explanations.length, 1);
    assert.ok(result.explanations[0] instanceof PolicyExplanation);
  });

  it('multiple rules produce multiple explanations', async () => {
    engine.registerPolicySet({
      name: 'Multi-Rule Policy',
      domain: 'orders',
      rules: [
        {
          name: 'rule_1',
          priority: 100,
          conditions: { field: 'amount', operator: 'gt', value: 100 },
          action: { type: 'deny', reason: 'Amount too high' },
        },
        {
          name: 'rule_2',
          priority: 50,
          conditions: { field: 'currency', operator: 'eq', value: 'BTC' },
          action: { type: 'deny', reason: 'BTC not supported' },
        },
      ],
    });

    const result = await engine.evaluate('orders', { amount: 500, currency: 'BTC' });
    assert.equal(result.explanations.length, 2);
    assert.equal(result.explanations[0].ruleName, 'rule_1');
    assert.equal(result.explanations[1].ruleName, 'rule_2');
  });

  it('explanation conditions are flattened from condition group details', async () => {
    engine.registerPolicySet({
      name: 'Flat Test',
      domain: 'orders',
      rules: [
        {
          name: 'multi_cond',
          conditions: {
            logic: 'and',
            conditions: [
              { field: 'a', operator: 'eq', value: 1 },
              { field: 'b', operator: 'eq', value: 2 },
            ],
          },
          action: { type: 'deny', reason: 'Both conditions met' },
        },
      ],
    });

    const result = await engine.evaluate('orders', { a: 1, b: 2 });
    assert.equal(result.explanations.length, 1);
    // Conditions should be flattened from the group details
    const conds = result.explanations[0].conditions;
    assert.equal(conds.length, 2);
    assert.equal(conds[0].field, 'a');
    assert.equal(conds[1].field, 'b');
  });

  it('transform + deny in same policy set: deny wins', async () => {
    engine.registerPolicySet({
      name: 'Mixed Policy',
      domain: 'orders',
      rules: [
        {
          name: 'transform_rule',
          priority: 50,
          conditions: { field: 'discount', operator: 'gt', value: 30 },
          action: {
            type: 'transform',
            transform: { field: 'discount', value: 30 },
            reason: 'Discount capped',
          },
        },
        {
          name: 'deny_rule',
          priority: 100,
          conditions: { field: 'order.total', operator: 'gt', value: 10000 },
          action: { type: 'deny', reason: 'Order too large' },
        },
      ],
    });

    const result = await engine.evaluate('orders', {
      order: { total: 20000 },
      discount: 50,
    });

    // Deny-overrides means shouldDeny is true even though transform also matched
    assert.equal(result.shouldDeny, true);
    assert.equal(result.explanations.length, 2);

    const denyExpl = result.explanations.find((e) => e.actionType === 'deny');
    assert.ok(denyExpl, 'should have a deny explanation');
    assert.equal(denyExpl.reason, 'Order too large');

    const transformExpl = result.explanations.find((e) => e.actionType === 'transform');
    assert.ok(transformExpl, 'should have a transform explanation');
  });
});

// ---------------------------------------------------------------------------
// applyPolicyTransform audit (TransformAuditEntry usage)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// PolicyEngine event emission with explanations
// ---------------------------------------------------------------------------

describe('PolicyEngine event emission with explanations', () => {
  it('evaluated event includes explanations array', async () => {
    const engine = new PolicyEngine({ storePath: null });
    engine.registerPolicySet({
      name: 'Event Test',
      domain: 'orders',
      rules: [
        {
          name: 'deny_rule',
          conditions: { field: 'x', operator: 'eq', value: 1 },
          action: { type: 'deny', reason: 'Blocked for testing' },
        },
      ],
    });

    const events = [];
    engine.on('evaluated', (e) => events.push(e));

    await engine.evaluate('orders', { x: 1 });

    assert.equal(events.length, 1);
    assert.ok(Array.isArray(events[0].explanations));
    assert.equal(events[0].explanations.length, 1);
    assert.equal(events[0].explanations[0].reason, 'Blocked for testing');
  });
});

// ---------------------------------------------------------------------------
// PolicySet stopOnMatch limits explanations
// ---------------------------------------------------------------------------

describe('PolicySet stopOnMatch limits explanations', () => {
  it('stopOnMatch prevents further explanations', () => {
    const ps = new PolicySet({
      name: 'StopOnMatch Test',
      domain: 'orders',
      rules: [
        {
          name: 'first_rule',
          priority: 100,
          conditions: { field: 'x', operator: 'eq', value: 1 },
          action: { type: 'deny', reason: 'First deny' },
          stopOnMatch: true,
        },
        {
          name: 'second_rule',
          priority: 50,
          conditions: { field: 'x', operator: 'eq', value: 1 },
          action: { type: 'deny', reason: 'Second deny' },
        },
      ],
    });

    const result = ps.evaluate({ x: 1 });
    assert.equal(result.explanations.length, 1, 'stopOnMatch should limit to one explanation');
    assert.equal(result.explanations[0].ruleName, 'first_rule');
  });
});

// ---------------------------------------------------------------------------
// PolicyExplanation reason fallback chain
// ---------------------------------------------------------------------------

describe('PolicyExplanation reason fallback chain', () => {
  it('falls back to metadata.reason when action.reason is absent', () => {
    const ps = new PolicySet({
      name: 'Fallback Test',
      domain: 'orders',
      rules: [
        {
          name: 'metadata_reason_rule',
          description: 'A rule with metadata reason',
          conditions: { field: 'x', operator: 'eq', value: 1 },
          action: { type: 'deny', metadata: { reason: 'Reason from metadata' } },
        },
      ],
    });

    const result = ps.evaluate({ x: 1 });
    assert.equal(result.explanations[0].reason, 'Reason from metadata');
  });

  it('falls back to rule description when both reason fields are absent', () => {
    const ps = new PolicySet({
      name: 'Description Fallback',
      domain: 'orders',
      rules: [
        {
          name: 'desc_reason_rule',
          description: 'Blocked because of business rule XYZ',
          conditions: { field: 'x', operator: 'eq', value: 1 },
          action: { type: 'deny' },
        },
      ],
    });

    const result = ps.evaluate({ x: 1 });
    assert.equal(result.explanations[0].reason, 'Blocked because of business rule XYZ');
  });
});

// ---------------------------------------------------------------------------
// applyPolicyTransform audit (TransformAuditEntry usage)
// ---------------------------------------------------------------------------

describe('applyPolicyTransform audit', () => {
  it('simple field change', () => {
    const entry = new TransformAuditEntry({
      ruleId: 'r-20',
      ruleName: 'cap_price',
      policySetId: 'ps-20',
      field: 'order.price',
      before: 150,
      after: 99.99,
    });

    assert.equal(entry.field, 'order.price');
    assert.equal(entry.before, 150);
    assert.equal(entry.after, 99.99);
    assert.equal(entry.ruleId, 'r-20');

    const json = entry.toJSON();
    assert.equal(json.before, 150);
    assert.equal(json.after, 99.99);
  });

  it('nested merge records before/after', () => {
    const beforeValue = { street: '123 Main St', city: 'Springfield' };
    const afterValue = { street: '123 Main St', city: 'Springfield', state: 'IL' };

    const entry = new TransformAuditEntry({
      ruleId: 'r-21',
      ruleName: 'enrich_address',
      policySetId: 'ps-21',
      field: 'order.shippingAddress',
      before: beforeValue,
      after: afterValue,
    });

    assert.deepStrictEqual(entry.before, beforeValue);
    assert.deepStrictEqual(entry.after, afterValue);
    assert.ok(entry.after.state === 'IL');
    assert.ok(entry.before.state === undefined);
  });

  it('no-op transform (same value)', () => {
    const entry = new TransformAuditEntry({
      ruleId: 'r-22',
      ruleName: 'no_change',
      policySetId: 'ps-22',
      field: 'order.status',
      before: 'pending',
      after: 'pending',
    });

    assert.equal(entry.before, entry.after);
    assert.equal(entry.before, 'pending');

    const json = entry.toJSON();
    assert.equal(json.before, json.after);
  });

  it('multiple fields tracked independently', () => {
    const entries = [
      new TransformAuditEntry({
        ruleId: 'r-23',
        ruleName: 'normalize',
        policySetId: 'ps-23',
        field: 'order.currency',
        before: 'usd',
        after: 'USD',
      }),
      new TransformAuditEntry({
        ruleId: 'r-23',
        ruleName: 'normalize',
        policySetId: 'ps-23',
        field: 'order.country',
        before: 'us',
        after: 'US',
      }),
      new TransformAuditEntry({
        ruleId: 'r-23',
        ruleName: 'normalize',
        policySetId: 'ps-23',
        field: 'order.email',
        before: 'USER@Example.COM',
        after: 'user@example.com',
      }),
    ];

    assert.equal(entries.length, 3);
    assert.equal(entries[0].field, 'order.currency');
    assert.equal(entries[1].field, 'order.country');
    assert.equal(entries[2].field, 'order.email');

    // Each entry should have its own timestamp
    for (const entry of entries) {
      assert.ok(entry.timestamp);
    }

    // All should share the same ruleId and policySetId
    assert.ok(entries.every((e) => e.ruleId === 'r-23'));
    assert.ok(entries.every((e) => e.policySetId === 'ps-23'));

    // Verify JSON serialization for each
    const jsons = entries.map((e) => e.toJSON());
    assert.equal(jsons[0].before, 'usd');
    assert.equal(jsons[0].after, 'USD');
    assert.equal(jsons[2].before, 'USER@Example.COM');
    assert.equal(jsons[2].after, 'user@example.com');
  });
});
