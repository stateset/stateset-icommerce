/**
 * Unit tests for a2a/rules-engine.js — Declarative Rules Engine
 *
 * Covers: addRule, removeRule, getRule, listRules, enableRule, disableRule,
 * evaluate (simple + compound conditions, all operators), testRule,
 * getAuditLog, built-in templates, priority ordering, block-wins semantics,
 * explanation strings, and edge cases.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { createRulesEngine } from '../../src/a2a/rules-engine.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeSimpleBlockRule(overrides = {}) {
  return {
    name: 'Block rule',
    description: 'Blocks everything',
    agentAddress: '0xAgent1',
    condition: { field: 'amount', operator: 'gt', value: 100 },
    action: { type: 'block', params: { reason: 'too expensive' } },
    priority: 80,
    enabled: true,
    tags: ['safety'],
    ...overrides,
  };
}

function makeSimpleApproveRule(overrides = {}) {
  return {
    name: 'Approve rule',
    description: 'Approves low-value transactions',
    agentAddress: '0xAgent1',
    condition: { field: 'amount', operator: 'lte', value: 100 },
    action: { type: 'approve', params: {} },
    priority: 50,
    enabled: true,
    tags: ['default'],
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Rules Engine', () => {
  /** @type {ReturnType<typeof createRulesEngine>} */
  let engine;

  beforeEach(() => {
    engine = createRulesEngine();
  });

  // 1. addRule creates rule with correct fields
  describe('addRule', () => {
    it('creates a rule with correct fields and returns an ID', () => {
      const id = engine.addRule(makeSimpleBlockRule());
      assert.ok(typeof id === 'string' && id.length > 0, 'should return a UUID string');

      const rule = engine.getRule(id);
      assert.equal(rule.name, 'Block rule');
      assert.equal(rule.description, 'Blocks everything');
      assert.equal(rule.agentAddress, '0xAgent1');
      assert.equal(rule.priority, 80);
      assert.equal(rule.enabled, true);
      assert.deepEqual(rule.tags, ['safety']);
      assert.equal(rule.action.type, 'block');
      assert.ok(rule.createdAt);
      assert.ok(rule.updatedAt);
    });

    it('rejects rules without a name', () => {
      assert.throws(() => engine.addRule({ condition: {}, action: { type: 'block' } }), /name/i);
    });

    it('rejects rules without a condition', () => {
      assert.throws(
        () => engine.addRule({ name: 'x', action: { type: 'block' } }),
        /condition/i,
      );
    });

    it('rejects rules without an action type', () => {
      assert.throws(
        () => engine.addRule({ name: 'x', condition: { field: 'a', operator: 'eq', value: 1 }, action: {} }),
        /action/i,
      );
    });

    it('rejects priority outside 1-100', () => {
      assert.throws(
        () => engine.addRule({ ...makeSimpleBlockRule(), priority: 0 }),
        /priority/i,
      );
      assert.throws(
        () => engine.addRule({ ...makeSimpleBlockRule(), priority: 101 }),
        /priority/i,
      );
    });

    it('defaults priority to 50 and enabled to true', () => {
      const id = engine.addRule({
        name: 'Minimal',
        condition: { field: 'x', operator: 'eq', value: 1 },
        action: { type: 'notify', params: {} },
      });
      const rule = engine.getRule(id);
      assert.equal(rule.priority, 50);
      assert.equal(rule.enabled, true);
    });
  });

  // 2. evaluate with simple condition matches correctly
  describe('evaluate — simple condition', () => {
    it('matches when condition is satisfied', () => {
      engine.addRule(makeSimpleBlockRule());
      const result = engine.evaluate({ amount: 200 });
      assert.equal(result.allowed, false);
      assert.equal(result.appliedRules.length, 1);
      assert.equal(result.appliedRules[0].matched, true);
    });

    it('does not match when condition is not satisfied', () => {
      engine.addRule(makeSimpleBlockRule());
      const result = engine.evaluate({ amount: 50 });
      assert.equal(result.allowed, true);
      assert.equal(result.appliedRules[0].matched, false);
    });
  });

  // 3. evaluate with compound `all` conditions (AND logic)
  describe('evaluate — compound all (AND)', () => {
    it('matches only when all sub-conditions are true', () => {
      engine.addRule({
        name: 'Compound AND',
        condition: {
          all: [
            { field: 'amount', operator: 'gt', value: 100 },
            { field: 'region', operator: 'eq', value: 'restricted' },
          ],
        },
        action: { type: 'block', params: {} },
        priority: 80,
      });

      // Both true
      const r1 = engine.evaluate({ amount: 200, region: 'restricted' });
      assert.equal(r1.allowed, false);

      // One false
      const r2 = engine.evaluate({ amount: 200, region: 'open' });
      assert.equal(r2.allowed, true);

      // Both false
      const r3 = engine.evaluate({ amount: 50, region: 'open' });
      assert.equal(r3.allowed, true);
    });
  });

  // 4. evaluate with compound `any` conditions (OR logic)
  describe('evaluate — compound any (OR)', () => {
    it('matches when any sub-condition is true', () => {
      engine.addRule({
        name: 'Compound OR',
        condition: {
          any: [
            { field: 'isFlagged', operator: 'eq', value: true },
            { field: 'amount', operator: 'gt', value: 10000 },
          ],
        },
        action: { type: 'block', params: {} },
        priority: 80,
      });

      const r1 = engine.evaluate({ isFlagged: true, amount: 5 });
      assert.equal(r1.allowed, false);

      const r2 = engine.evaluate({ isFlagged: false, amount: 20000 });
      assert.equal(r2.allowed, false);

      const r3 = engine.evaluate({ isFlagged: false, amount: 50 });
      assert.equal(r3.allowed, true);
    });
  });

  // 5. Block action prevents operation
  describe('block action', () => {
    it('sets allowed to false when block action fires', () => {
      engine.addRule(makeSimpleBlockRule());
      const result = engine.evaluate({ amount: 500 });
      assert.equal(result.allowed, false);
    });

    it('non-block actions do not set allowed to false', () => {
      engine.addRule({
        name: 'Notify rule',
        condition: { field: 'amount', operator: 'gt', value: 10 },
        action: { type: 'notify', params: { channel: 'slack' } },
        priority: 50,
      });
      const result = engine.evaluate({ amount: 500 });
      assert.equal(result.allowed, true);
      assert.equal(result.appliedRules[0].matched, true);
    });
  });

  // 6. Higher priority rules evaluated first
  describe('priority ordering', () => {
    it('evaluates higher priority rules first', () => {
      engine.addRule(makeSimpleApproveRule({ priority: 30, name: 'Low' }));
      engine.addRule(makeSimpleBlockRule({ priority: 90, name: 'High' }));
      engine.addRule(makeSimpleApproveRule({ priority: 60, name: 'Mid' }));

      const result = engine.evaluate({ amount: 200 });
      assert.equal(result.appliedRules[0].name, 'High');
      assert.equal(result.appliedRules[1].name, 'Mid');
      assert.equal(result.appliedRules[2].name, 'Low');
    });
  });

  // 7. First block wins over later approve
  describe('first block wins', () => {
    it('blocks even when a later lower-priority rule would approve', () => {
      engine.addRule({
        name: 'Block high value',
        condition: { field: 'amount', operator: 'gt', value: 100 },
        action: { type: 'block', params: {} },
        priority: 90,
      });
      engine.addRule({
        name: 'Approve all',
        condition: { field: 'amount', operator: 'gt', value: 0 },
        action: { type: 'approve', params: {} },
        priority: 10,
      });

      const result = engine.evaluate({ amount: 500 });
      assert.equal(result.allowed, false);
    });
  });

  // 8. Built-in templates create valid rules
  describe('built-in templates', () => {
    it('HIGH_VALUE_GUARD creates a valid rule', () => {
      const id = engine.addFromTemplate('HIGH_VALUE_GUARD', {}, 2000);
      const rule = engine.getRule(id);
      assert.equal(rule.name, 'High-value guard');
      assert.equal(rule.condition.value, 2000);
      assert.equal(rule.action.type, 'require_escrow');
      assert.ok(rule.tags.includes('financial'));
    });

    it('LOW_REPUTATION_FILTER creates a valid rule', () => {
      const id = engine.addFromTemplate('LOW_REPUTATION_FILTER', {}, 2.0);
      const rule = engine.getRule(id);
      assert.equal(rule.condition.value, 2.0);
      assert.equal(rule.action.type, 'block');
    });

    it('DAILY_SPEND_LIMIT creates a valid rule', () => {
      const id = engine.addFromTemplate('DAILY_SPEND_LIMIT');
      const rule = engine.getRule(id);
      assert.equal(rule.condition.value, 5000);
      assert.equal(rule.action.type, 'block');
    });

    it('FIRST_TIME_BUYER_ESCROW creates a valid rule', () => {
      const id = engine.addFromTemplate('FIRST_TIME_BUYER_ESCROW');
      const rule = engine.getRule(id);
      assert.equal(rule.condition.field, 'isFirstTimeBuyer');
      assert.equal(rule.action.type, 'require_escrow');
    });

    it('DISPUTE_RATE_BLACKLIST creates a valid rule', () => {
      const id = engine.addFromTemplate('DISPUTE_RATE_BLACKLIST', {}, 5);
      const rule = engine.getRule(id);
      assert.equal(rule.condition.value, 5);
      assert.equal(rule.action.type, 'block');
    });

    it('throws for unknown template', () => {
      assert.throws(() => engine.addFromTemplate('NONEXISTENT'), /Unknown template/);
    });

    it('exposes available template names', () => {
      assert.ok(Array.isArray(engine.TEMPLATES));
      assert.ok(engine.TEMPLATES.includes('HIGH_VALUE_GUARD'));
      assert.ok(engine.TEMPLATES.includes('LOW_REPUTATION_FILTER'));
      assert.ok(engine.TEMPLATES.includes('DAILY_SPEND_LIMIT'));
      assert.ok(engine.TEMPLATES.includes('FIRST_TIME_BUYER_ESCROW'));
      assert.ok(engine.TEMPLATES.includes('DISPUTE_RATE_BLACKLIST'));
    });
  });

  // 9. testRule doesn't execute action
  describe('testRule', () => {
    it('returns matched status without adding to audit log', () => {
      const id = engine.addRule(makeSimpleBlockRule());

      const before = engine.getAuditLog();
      assert.equal(before.length, 0);

      const result = engine.testRule(id, { amount: 500 });
      assert.equal(result.matched, true);
      assert.equal(result.action.type, 'block');

      // Audit log should still be empty
      const after = engine.getAuditLog();
      assert.equal(after.length, 0);
    });

    it('returns not matched when condition is not satisfied', () => {
      const id = engine.addRule(makeSimpleBlockRule());
      const result = engine.testRule(id, { amount: 10 });
      assert.equal(result.matched, false);
    });

    it('throws for unknown rule ID', () => {
      assert.throws(() => engine.testRule('nonexistent', {}), /not found/i);
    });
  });

  // 10. getAuditLog tracks evaluations
  describe('getAuditLog', () => {
    it('records each evaluate() call', () => {
      engine.addRule(makeSimpleBlockRule());

      engine.evaluate({ amount: 500 });
      engine.evaluate({ amount: 10 });
      engine.evaluate({ amount: 300 });

      const log = engine.getAuditLog();
      assert.equal(log.length, 3);
      // Most recent first
      assert.ok(log[0].timestamp >= log[1].timestamp);
    });

    it('respects limit parameter', () => {
      engine.addRule(makeSimpleBlockRule());
      for (let i = 0; i < 10; i++) {
        engine.evaluate({ amount: i });
      }

      const log = engine.getAuditLog(3);
      assert.equal(log.length, 3);
    });

    it('includes context, allowed, appliedRules, and explanation', () => {
      engine.addRule(makeSimpleBlockRule());
      engine.evaluate({ amount: 999 });

      const [entry] = engine.getAuditLog(1);
      assert.deepEqual(entry.context, { amount: 999 });
      assert.equal(entry.allowed, false);
      assert.ok(Array.isArray(entry.appliedRules));
      assert.ok(typeof entry.explanation === 'string');
      assert.ok(entry.id);
      assert.ok(entry.timestamp);
    });
  });

  // 11. enableRule / disableRule toggle works
  describe('enableRule / disableRule', () => {
    it('disables a rule so it is skipped during evaluation', () => {
      const id = engine.addRule(makeSimpleBlockRule());

      // Enabled — should block
      let result = engine.evaluate({ amount: 500 });
      assert.equal(result.allowed, false);

      // Disable
      const disabled = engine.disableRule(id);
      assert.equal(disabled, true);
      assert.equal(engine.getRule(id).enabled, false);

      // Now should allow
      result = engine.evaluate({ amount: 500 });
      assert.equal(result.allowed, true);
      // Disabled rules are not in appliedRules
      assert.equal(result.appliedRules.length, 0);
    });

    it('re-enables a disabled rule', () => {
      const id = engine.addRule(makeSimpleBlockRule({ enabled: false }));

      let result = engine.evaluate({ amount: 500 });
      assert.equal(result.allowed, true);

      engine.enableRule(id);
      result = engine.evaluate({ amount: 500 });
      assert.equal(result.allowed, false);
    });

    it('returns false for unknown rule IDs', () => {
      assert.equal(engine.enableRule('fake-id'), false);
      assert.equal(engine.disableRule('fake-id'), false);
    });
  });

  // 12. Operators: eq, gt, lt, in, contains, matches (regex)
  describe('operators', () => {
    it('eq — equals', () => {
      engine.addRule({
        name: 'eq test',
        condition: { field: 'status', operator: 'eq', value: 'active' },
        action: { type: 'block', params: {} },
        priority: 50,
      });
      assert.equal(engine.evaluate({ status: 'active' }).allowed, false);
      assert.equal(engine.evaluate({ status: 'inactive' }).allowed, true);
    });

    it('neq — not equals', () => {
      engine.addRule({
        name: 'neq test',
        condition: { field: 'status', operator: 'neq', value: 'verified' },
        action: { type: 'block', params: {} },
        priority: 50,
      });
      assert.equal(engine.evaluate({ status: 'unverified' }).allowed, false);
      assert.equal(engine.evaluate({ status: 'verified' }).allowed, true);
    });

    it('gt — greater than', () => {
      engine.addRule({
        name: 'gt test',
        condition: { field: 'amount', operator: 'gt', value: 100 },
        action: { type: 'block', params: {} },
        priority: 50,
      });
      assert.equal(engine.evaluate({ amount: 200 }).allowed, false);
      assert.equal(engine.evaluate({ amount: 100 }).allowed, true);
      assert.equal(engine.evaluate({ amount: 50 }).allowed, true);
    });

    it('gte — greater than or equal', () => {
      engine.addRule({
        name: 'gte test',
        condition: { field: 'amount', operator: 'gte', value: 100 },
        action: { type: 'block', params: {} },
        priority: 50,
      });
      assert.equal(engine.evaluate({ amount: 100 }).allowed, false);
      assert.equal(engine.evaluate({ amount: 99 }).allowed, true);
    });

    it('lt — less than', () => {
      engine.addRule({
        name: 'lt test',
        condition: { field: 'score', operator: 'lt', value: 3 },
        action: { type: 'block', params: {} },
        priority: 50,
      });
      assert.equal(engine.evaluate({ score: 2 }).allowed, false);
      assert.equal(engine.evaluate({ score: 3 }).allowed, true);
      assert.equal(engine.evaluate({ score: 5 }).allowed, true);
    });

    it('lte — less than or equal', () => {
      engine.addRule({
        name: 'lte test',
        condition: { field: 'score', operator: 'lte', value: 3 },
        action: { type: 'block', params: {} },
        priority: 50,
      });
      assert.equal(engine.evaluate({ score: 3 }).allowed, false);
      assert.equal(engine.evaluate({ score: 4 }).allowed, true);
    });

    it('in — value in array', () => {
      engine.addRule({
        name: 'in test',
        condition: { field: 'country', operator: 'in', value: ['US', 'CA', 'GB'] },
        action: { type: 'block', params: {} },
        priority: 50,
      });
      assert.equal(engine.evaluate({ country: 'US' }).allowed, false);
      assert.equal(engine.evaluate({ country: 'DE' }).allowed, true);
    });

    it('not_in — value not in array', () => {
      engine.addRule({
        name: 'not_in test',
        condition: { field: 'country', operator: 'not_in', value: ['US', 'CA'] },
        action: { type: 'block', params: {} },
        priority: 50,
      });
      assert.equal(engine.evaluate({ country: 'DE' }).allowed, false);
      assert.equal(engine.evaluate({ country: 'US' }).allowed, true);
    });

    it('contains — substring match', () => {
      engine.addRule({
        name: 'contains test',
        condition: { field: 'email', operator: 'contains', value: '@blocked.com' },
        action: { type: 'block', params: {} },
        priority: 50,
      });
      assert.equal(engine.evaluate({ email: 'bad@blocked.com' }).allowed, false);
      assert.equal(engine.evaluate({ email: 'good@example.com' }).allowed, true);
    });

    it('matches — regex match', () => {
      engine.addRule({
        name: 'matches test',
        condition: { field: 'memo', operator: 'matches', value: '^URGENT' },
        action: { type: 'flag_review', params: {} },
        priority: 50,
      });
      const r1 = engine.evaluate({ memo: 'URGENT: need payment' });
      assert.equal(r1.appliedRules[0].matched, true);

      const r2 = engine.evaluate({ memo: 'normal message' });
      assert.equal(r2.appliedRules[0].matched, false);
    });

    it('matches — handles invalid regex gracefully', () => {
      engine.addRule({
        name: 'bad regex',
        condition: { field: 'memo', operator: 'matches', value: '[invalid' },
        action: { type: 'block', params: {} },
        priority: 50,
      });
      const result = engine.evaluate({ memo: 'anything' });
      assert.equal(result.appliedRules[0].matched, false);
      assert.equal(result.allowed, true);
    });
  });

  // 13. Explanation includes matched rule names
  describe('explanation', () => {
    it('mentions matched rule names when blocked', () => {
      engine.addRule(makeSimpleBlockRule({ name: 'Spend Guard' }));
      const result = engine.evaluate({ amount: 500 });
      assert.ok(result.explanation.includes('Spend Guard'));
      assert.ok(result.explanation.includes('Blocked'));
    });

    it('mentions matched rule names when allowed', () => {
      engine.addRule({
        name: 'Flag for review',
        condition: { field: 'amount', operator: 'gt', value: 10 },
        action: { type: 'flag_review', params: {} },
        priority: 50,
      });
      const result = engine.evaluate({ amount: 100 });
      assert.ok(result.explanation.includes('Flag for review'));
      assert.ok(result.explanation.includes('allowed'));
    });
  });

  // 14. Empty context against no rules returns allowed
  describe('no rules / empty context', () => {
    it('returns allowed with no rules registered', () => {
      const result = engine.evaluate({});
      assert.equal(result.allowed, true);
      assert.equal(result.appliedRules.length, 0);
      assert.ok(result.explanation.includes('No rules matched'));
    });

    it('returns allowed with empty context and non-matching rules', () => {
      engine.addRule(makeSimpleBlockRule());
      const result = engine.evaluate({});
      assert.equal(result.allowed, true);
    });
  });

  // Additional coverage
  describe('removeRule', () => {
    it('removes an existing rule', () => {
      const id = engine.addRule(makeSimpleBlockRule());
      assert.ok(engine.getRule(id));
      assert.equal(engine.removeRule(id), true);
      assert.equal(engine.getRule(id), null);
    });

    it('returns false for nonexistent rule', () => {
      assert.equal(engine.removeRule('nonexistent'), false);
    });
  });

  describe('listRules', () => {
    it('returns all rules sorted by priority descending', () => {
      engine.addRule(makeSimpleBlockRule({ name: 'A', priority: 30 }));
      engine.addRule(makeSimpleBlockRule({ name: 'B', priority: 90 }));
      engine.addRule(makeSimpleBlockRule({ name: 'C', priority: 60 }));

      const rules = engine.listRules();
      assert.equal(rules.length, 3);
      assert.equal(rules[0].name, 'B');
      assert.equal(rules[1].name, 'C');
      assert.equal(rules[2].name, 'A');
    });

    it('filters by agentAddress', () => {
      engine.addRule(makeSimpleBlockRule({ agentAddress: '0xA' }));
      engine.addRule(makeSimpleBlockRule({ agentAddress: '0xB' }));

      const filtered = engine.listRules({ agentAddress: '0xA' });
      assert.equal(filtered.length, 1);
      assert.equal(filtered[0].agentAddress, '0xA');
    });

    it('filters by enabled', () => {
      engine.addRule(makeSimpleBlockRule({ enabled: true, name: 'On' }));
      engine.addRule(makeSimpleBlockRule({ enabled: false, name: 'Off' }));

      assert.equal(engine.listRules({ enabled: true }).length, 1);
      assert.equal(engine.listRules({ enabled: false }).length, 1);
    });

    it('filters by tags (any match)', () => {
      engine.addRule(makeSimpleBlockRule({ tags: ['safety', 'financial'] }));
      engine.addRule(makeSimpleBlockRule({ tags: ['trust'] }));

      const filtered = engine.listRules({ tags: ['trust'] });
      assert.equal(filtered.length, 1);
    });
  });

  describe('dot-path field resolution', () => {
    it('resolves nested field paths', () => {
      engine.addRule({
        name: 'Nested check',
        condition: { field: 'order.total', operator: 'gt', value: 500 },
        action: { type: 'block', params: {} },
        priority: 50,
      });

      assert.equal(engine.evaluate({ order: { total: 600 } }).allowed, false);
      assert.equal(engine.evaluate({ order: { total: 100 } }).allowed, true);
    });
  });

  describe('nested compound conditions', () => {
    it('handles deeply nested all/any combinations', () => {
      engine.addRule({
        name: 'Nested compound',
        condition: {
          all: [
            { field: 'amount', operator: 'gt', value: 100 },
            {
              any: [
                { field: 'region', operator: 'eq', value: 'EU' },
                { field: 'region', operator: 'eq', value: 'APAC' },
              ],
            },
          ],
        },
        action: { type: 'block', params: {} },
        priority: 50,
      });

      assert.equal(engine.evaluate({ amount: 200, region: 'EU' }).allowed, false);
      assert.equal(engine.evaluate({ amount: 200, region: 'APAC' }).allowed, false);
      assert.equal(engine.evaluate({ amount: 200, region: 'US' }).allowed, true);
      assert.equal(engine.evaluate({ amount: 50, region: 'EU' }).allowed, true);
    });
  });
});
