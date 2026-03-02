import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { fraudTools } from '../../src/tools/fraud.js';

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

const byName = Object.fromEntries(fraudTools.map((t) => [t.name, t]));

const EXPECTED_NAMES = [
  'assess_order_fraud',
  'get_fraud_assessment',
  'list_fraud_signals',
  'create_fraud_rule',
  'update_fraud_rule',
  'review_flagged_order',
];

// ---------------------------------------------------------------------------
// Module exports
// ---------------------------------------------------------------------------

describe('fraudTools — module exports', () => {
  it('exports an array of 6 tools', () => {
    assert.ok(Array.isArray(fraudTools));
    assert.equal(fraudTools.length, 6);
  });

  it('exports expected tool names in order', () => {
    const names = fraudTools.map((t) => t.name);
    assert.deepStrictEqual(names, EXPECTED_NAMES);
  });

  it('all tools have handler functions', () => {
    for (const tool of fraudTools) {
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
    }
  });

  it('all tools have valid permissions', () => {
    for (const tool of fraudTools) {
      assert.ok(
        ['read', 'write', 'admin'].includes(tool.permission),
        `${tool.name} has invalid permission: ${tool.permission}`,
      );
    }
  });

  it('all tools have non-empty descriptions', () => {
    for (const tool of fraudTools) {
      assert.ok(tool.description, `${tool.name} missing description`);
      assert.ok(tool.description.length > 10, `${tool.name} description too short`);
    }
  });

  it('all tools have an inputSchema object', () => {
    for (const tool of fraudTools) {
      assert.ok(tool.inputSchema && typeof tool.inputSchema === 'object', `${tool.name} missing inputSchema`);
    }
  });
});

// ---------------------------------------------------------------------------
// Permission checks
// ---------------------------------------------------------------------------

describe('fraudTools — permission assignments', () => {
  it('assess_order_fraud is read', () => {
    assert.equal(byName['assess_order_fraud'].permission, 'read');
  });

  it('get_fraud_assessment is read', () => {
    assert.equal(byName['get_fraud_assessment'].permission, 'read');
  });

  it('list_fraud_signals is read', () => {
    assert.equal(byName['list_fraud_signals'].permission, 'read');
  });

  it('create_fraud_rule is admin', () => {
    assert.equal(byName['create_fraud_rule'].permission, 'admin');
  });

  it('update_fraud_rule is admin', () => {
    assert.equal(byName['update_fraud_rule'].permission, 'admin');
  });

  it('review_flagged_order is write', () => {
    assert.equal(byName['review_flagged_order'].permission, 'write');
  });
});

// ---------------------------------------------------------------------------
// Input schema validation
// ---------------------------------------------------------------------------

describe('fraudTools — input schemas', () => {
  it('assess_order_fraud has required orderId and optional fields', () => {
    const schema = byName['assess_order_fraud'].inputSchema;
    assert.ok(schema.orderId, 'missing orderId');
    assert.ok(schema.customerIp, 'missing customerIp');
    assert.ok(schema.deviceFingerprint, 'missing deviceFingerprint');
    assert.ok(schema.billingAddress, 'missing billingAddress');
    assert.ok(schema.shippingAddress, 'missing shippingAddress');
  });

  it('get_fraud_assessment has assessmentId', () => {
    const schema = byName['get_fraud_assessment'].inputSchema;
    assert.ok(schema.assessmentId, 'missing assessmentId');
  });

  it('list_fraud_signals has optional orderId, riskLevel, limit', () => {
    const schema = byName['list_fraud_signals'].inputSchema;
    assert.ok(schema.orderId, 'missing orderId');
    assert.ok(schema.riskLevel, 'missing riskLevel');
    assert.ok(schema.limit, 'missing limit');
  });

  it('create_fraud_rule has name, condition, action, and optional fields', () => {
    const schema = byName['create_fraud_rule'].inputSchema;
    assert.ok(schema.name, 'missing name');
    assert.ok(schema.description, 'missing description');
    assert.ok(schema.condition, 'missing condition');
    assert.ok(schema.action, 'missing action');
    assert.ok(schema.scoreAdjustment, 'missing scoreAdjustment');
    assert.ok(schema.priority, 'missing priority');
    assert.ok(schema.enabled, 'missing enabled');
  });

  it('update_fraud_rule has ruleId and optional update fields', () => {
    const schema = byName['update_fraud_rule'].inputSchema;
    assert.ok(schema.ruleId, 'missing ruleId');
    assert.ok(schema.name, 'missing name');
    assert.ok(schema.condition, 'missing condition');
    assert.ok(schema.action, 'missing action');
    assert.ok(schema.scoreAdjustment, 'missing scoreAdjustment');
    assert.ok(schema.priority, 'missing priority');
    assert.ok(schema.enabled, 'missing enabled');
  });

  it('review_flagged_order has assessmentId, decision, reason', () => {
    const schema = byName['review_flagged_order'].inputSchema;
    assert.ok(schema.assessmentId, 'missing assessmentId');
    assert.ok(schema.decision, 'missing decision');
    assert.ok(schema.reason, 'missing reason');
    assert.ok(schema.reviewerNote, 'missing reviewerNote');
  });
});

// ---------------------------------------------------------------------------
// Handler apply-guard (write/admin tools)
// ---------------------------------------------------------------------------

describe('fraudTools — apply-guard on write/admin tools', () => {
  it('create_fraud_rule requires --apply', async () => {
    const result = await byName['create_fraud_rule'].handler({
      params: {
        name: 'Block high-risk',
        condition: { field: 'order_amount', operator: 'gt', value: 10000 },
        action: 'block',
      },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
    assert.ok(result.wouldDo);
  });

  it('update_fraud_rule requires --apply', async () => {
    const result = await byName['update_fraud_rule'].handler({
      params: { ruleId: 'rule-1', enabled: false },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
  });

  it('review_flagged_order requires --apply', async () => {
    const result = await byName['review_flagged_order'].handler({
      params: { assessmentId: 'fa-1', decision: 'approve', reason: 'Legit customer' },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
  });
});

// ---------------------------------------------------------------------------
// Handler error paths (commerce stub missing methods)
// ---------------------------------------------------------------------------

describe('fraudTools — handler error paths', () => {
  it('assess_order_fraud fails gracefully with empty commerce', async () => {
    try {
      await byName['assess_order_fraud'].handler({
        params: { orderId: 'order-1' },
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('get_fraud_assessment fails gracefully with empty commerce', async () => {
    try {
      await byName['get_fraud_assessment'].handler({
        params: { assessmentId: 'fa-1' },
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('list_fraud_signals fails gracefully with empty commerce', async () => {
    try {
      await byName['list_fraud_signals'].handler({
        params: { limit: 10 },
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('create_fraud_rule fails gracefully with empty commerce when allowApply=true', async () => {
    try {
      await byName['create_fraud_rule'].handler({
        params: {
          name: 'Test rule',
          condition: { field: 'email_domain', operator: 'eq', value: 'spam.com' },
          action: 'flag',
        },
        allowApply: true,
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('update_fraud_rule fails gracefully with empty commerce when allowApply=true', async () => {
    try {
      await byName['update_fraud_rule'].handler({
        params: { ruleId: 'rule-1', enabled: false },
        allowApply: true,
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('review_flagged_order fails gracefully with empty commerce when allowApply=true', async () => {
    try {
      await byName['review_flagged_order'].handler({
        params: { assessmentId: 'fa-1', decision: 'reject', reason: 'Suspicious' },
        allowApply: true,
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });
});

// ---------------------------------------------------------------------------
// Handler success paths (mocked commerce)
// ---------------------------------------------------------------------------

describe('fraudTools — handler success paths (mocked commerce)', () => {
  const mockAssessment = {
    id: 'fa-001',
    orderId: 'order-1',
    riskScore: 35,
    riskLevel: 'low',
    recommendation: 'accept',
    signals: [],
    matchedRules: [],
    assessedAt: '2026-01-15T00:00:00Z',
    reviewStatus: 'pending',
    reviewedBy: null,
    reviewedAt: null,
  };

  const mockSignals = [
    {
      id: 'sig-1',
      orderId: 'order-1',
      type: 'velocity',
      description: 'Multiple orders in 1 hour',
      severity: 'medium',
      metadata: {},
      detectedAt: '2026-01-15T00:00:00Z',
    },
  ];

  const mockRule = { id: 'rule-1', name: 'Test rule', action: 'flag', enabled: true };

  const commerce = {
    fraud: {
      assessOrder: async () => mockAssessment,
      getAssessment: async (id) => (id === 'fa-001' ? mockAssessment : null),
      listSignals: async () => mockSignals,
      createRule: async (data) => ({ ...mockRule, ...data }),
      updateRule: async (_id, data) => ({ ...mockRule, ...data }),
      reviewOrder: async (data) => ({ ...mockAssessment, reviewStatus: data.decision }),
    },
  };

  it('assess_order_fraud returns success with assessment shape', async () => {
    const result = await byName['assess_order_fraud'].handler({
      params: { orderId: 'order-1' },
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.assessment);
    assert.equal(result.assessment.orderId, 'order-1');
    assert.equal(result.assessment.riskScore, 35);
    assert.equal(result.assessment.riskLevel, 'low');
  });

  it('get_fraud_assessment returns success for existing assessment', async () => {
    const result = await byName['get_fraud_assessment'].handler({
      params: { assessmentId: 'fa-001' },
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.assessment);
    assert.equal(result.assessment.id, 'fa-001');
  });

  it('get_fraud_assessment returns not-found for missing assessment', async () => {
    const result = await byName['get_fraud_assessment'].handler({
      params: { assessmentId: 'nonexistent' },
      commerce,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('list_fraud_signals returns signal array', async () => {
    const result = await byName['list_fraud_signals'].handler({
      params: { limit: 50 },
      commerce,
    });
    assert.equal(result.success, true);
    assert.equal(result.returned, 1);
    assert.ok(Array.isArray(result.signals));
    assert.equal(result.signals[0].type, 'velocity');
  });

  it('create_fraud_rule returns success with allowApply', async () => {
    const result = await byName['create_fraud_rule'].handler({
      params: {
        name: 'High amount rule',
        condition: { field: 'order_amount', operator: 'gt', value: 5000 },
        action: 'review',
      },
      allowApply: true,
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('created'));
    assert.ok(result.rule);
  });

  it('update_fraud_rule returns success with allowApply', async () => {
    const result = await byName['update_fraud_rule'].handler({
      params: { ruleId: 'rule-1', enabled: false },
      allowApply: true,
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('updated'));
  });

  it('review_flagged_order returns success with allowApply', async () => {
    const result = await byName['review_flagged_order'].handler({
      params: { assessmentId: 'fa-001', decision: 'approve', reason: 'Trusted customer' },
      allowApply: true,
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('approved'));
  });

  it('review_flagged_order reject message says rejected', async () => {
    const result = await byName['review_flagged_order'].handler({
      params: { assessmentId: 'fa-001', decision: 'reject', reason: 'Fraud confirmed' },
      allowApply: true,
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('rejected'));
  });

  it('review_flagged_order escalate message says escalated', async () => {
    const result = await byName['review_flagged_order'].handler({
      params: { assessmentId: 'fa-001', decision: 'escalate', reason: 'Needs manager review' },
      allowApply: true,
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('escalated'));
  });
});
