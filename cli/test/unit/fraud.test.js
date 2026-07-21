/**
 * Fraud Detection Tools Test Suite
 *
 * Tests for the fraudTools module (cli/src/tools/fraud.js):
 * - assess_order_fraud (read)
 * - get_fraud_assessment (read)
 * - list_fraud_signals (read)
 * - create_fraud_rule (admin)
 * - update_fraud_rule (admin)
 * - review_flagged_order (write)
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { fraudTools } from '../../src/tools/fraud.js';

// ============================================================================
// Helper: find tool by name from a tools array
// ============================================================================

function findTool(tools, name) {
  const tool = tools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock data
// ============================================================================

const mockAssessment = {
  id: 'fa_001',
  orderId: 'ord_001',
  riskScore: 0.35,
  riskLevel: 'low',
  recommendation: 'accept',
  signals: [{ type: 'velocity_check', score: 0.2, details: 'Normal velocity' }],
  matchedRules: [],
  reviewStatus: null,
  reviewedBy: null,
  assessedAt: '2026-02-01T00:00:00Z',
  reviewedAt: null,
};

const mockRule = {
  id: 'fr_001',
  name: 'High Value Alert',
  description: 'Flag orders over $5000',
  condition: { field: 'order_amount', operator: 'gt', value: 5000 },
  action: 'review',
  scoreAdjustment: 30,
  priority: 50,
  enabled: true,
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-01T00:00:00Z',
};

const mockSignal = {
  id: 'fs_001',
  orderId: 'ord_001',
  type: 'velocity_check',
  description: 'Normal purchase velocity',
  severity: 'low',
  metadata: { checkCount: 2, windowHours: 24 },
  detectedAt: '2026-02-01T00:00:00Z',
};

// ============================================================================
// Mock commerce factory
// ============================================================================

function makeFraudCommerce(overrides = {}) {
  return {
    fraud: {
      assessOrder: async (data) => ({ ...mockAssessment, orderId: data.orderId }),
      getAssessment: async (id) => (id === 'fa_001' ? mockAssessment : null),
      listSignals: async (_opts) => [mockSignal],
      createRule: async (data) => ({ ...mockRule, ...data }),
      updateRule: async (id, data) => ({ ...mockRule, id, ...data }),
      reviewOrder: async (data) => ({
        ...mockAssessment,
        assessmentId: data.assessmentId,
        reviewStatus: data.decision,
        reviewedBy: 'agent',
        reviewedAt: '2026-02-01T12:00:00Z',
      }),
      ...overrides,
    },
  };
}

// ============================================================================
// Structural sanity check
// ============================================================================

describe('Fraud Tools — structure', () => {
  it('exports an array', () => {
    assert.ok(Array.isArray(fraudTools));
  });

  it('exports exactly 6 tools', () => {
    assert.equal(fraudTools.length, 6);
  });

  it('every tool has name, handler, and permission', () => {
    for (const tool of fraudTools) {
      assert.ok(tool.name, `missing name`);
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
      assert.ok(tool.permission, `${tool.name} missing permission`);
    }
  });

  it('admin tools have permission: admin', () => {
    const adminTools = ['create_fraud_rule', 'update_fraud_rule'];
    for (const name of adminTools) {
      const tool = findTool(fraudTools, name);
      assert.equal(tool.permission, 'admin', `${name} should have admin permission`);
    }
  });

  it('read tools have permission: read', () => {
    const readTools = ['assess_order_fraud', 'get_fraud_assessment', 'list_fraud_signals'];
    for (const name of readTools) {
      const tool = findTool(fraudTools, name);
      assert.equal(tool.permission, 'read', `${name} should have read permission`);
    }
  });

  it('review_flagged_order has permission: write', () => {
    const tool = findTool(fraudTools, 'review_flagged_order');
    assert.equal(tool.permission, 'write');
  });
});

// ============================================================================
// assess_order_fraud
// ============================================================================

describe('assess_order_fraud', () => {
  const tool = findTool(fraudTools, 'assess_order_fraud');

  it('returns assessment with riskScore for valid order', async () => {
    const result = await tool.handler({
      commerce: makeFraudCommerce(),
      params: { orderId: 'ord_001' },
    });
    assert.equal(result.success, true);
    assert.ok(result.assessment);
    assert.equal(result.assessment.orderId, 'ord_001');
    assert.equal(result.assessment.riskScore, 0.35);
    assert.equal(result.assessment.riskLevel, 'low');
    assert.ok(Array.isArray(result.assessment.signals));
  });

  it('passes all optional fields to commerce.fraud.assessOrder', async () => {
    let calledWith;
    const commerce = makeFraudCommerce({
      assessOrder: async (data) => {
        calledWith = data;
        return { ...mockAssessment, orderId: data.orderId };
      },
    });
    await tool.handler({
      commerce,
      params: {
        orderId: 'ord_002',
        customerIp: '203.0.113.42',
        deviceFingerprint: 'fp_abc123',
        billingAddress: { country: 'US', region: 'CA', postalCode: '90210' },
        shippingAddress: { country: 'US', region: 'NY', postalCode: '10001' },
      },
    });
    assert.equal(calledWith.orderId, 'ord_002');
    assert.equal(calledWith.customerIp, '203.0.113.42');
    assert.equal(calledWith.deviceFingerprint, 'fp_abc123');
    assert.equal(calledWith.billingAddress.country, 'US');
    assert.equal(calledWith.shippingAddress.country, 'US');
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeFraudCommerce({
      assessOrder: async () => {
        throw new Error('assessment engine unavailable');
      },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { orderId: 'ord_001' } }),
      /assessment engine unavailable/,
    );
  });
});

// ============================================================================
// get_fraud_assessment
// ============================================================================

describe('get_fraud_assessment', () => {
  const tool = findTool(fraudTools, 'get_fraud_assessment');

  it('returns assessment details for valid ID', async () => {
    const result = await tool.handler({
      commerce: makeFraudCommerce(),
      params: { assessmentId: 'fa_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.assessment.id, 'fa_001');
    assert.equal(result.assessment.orderId, 'ord_001');
    assert.equal(result.assessment.riskScore, 0.35);
    assert.equal(result.assessment.riskLevel, 'low');
    assert.ok(result.assessment.signals);
    assert.ok('reviewStatus' in result.assessment);
    assert.ok('reviewedBy' in result.assessment);
  });

  it('returns success: false for unknown assessment ID', async () => {
    const result = await tool.handler({
      commerce: makeFraudCommerce(),
      params: { assessmentId: 'fa_nope' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeFraudCommerce({
      getAssessment: async () => {
        throw new Error('lookup failed');
      },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { assessmentId: 'fa_001' } }),
      /lookup failed/,
    );
  });
});

// ============================================================================
// list_fraud_signals
// ============================================================================

describe('list_fraud_signals', () => {
  const tool = findTool(fraudTools, 'list_fraud_signals');

  it('returns signals with returned count', async () => {
    const result = await tool.handler({
      commerce: makeFraudCommerce(),
      params: {},
    });
    assert.equal(result.success, true);
    assert.equal(result.returned, 1);
    assert.ok(Array.isArray(result.signals));
    assert.equal(result.signals[0].id, 'fs_001');
    assert.equal(result.signals[0].type, 'velocity_check');
  });

  it('passes orderId and riskLevel filters to commerce.fraud.listSignals', async () => {
    let calledOpts;
    const commerce = makeFraudCommerce({
      listSignals: async (opts) => {
        calledOpts = opts;
        return [];
      },
    });
    await tool.handler({
      commerce,
      params: { orderId: 'ord_001', riskLevel: 'high' },
    });
    assert.equal(calledOpts.orderId, 'ord_001');
    assert.equal(calledOpts.riskLevel, 'high');
  });

  it('slices results to limit', async () => {
    const manySignals = Array.from({ length: 20 }, (_, i) => ({
      ...mockSignal,
      id: `fs_${String(i).padStart(3, '0')}`,
    }));
    const commerce = makeFraudCommerce({
      listSignals: async () => manySignals,
    });
    const result = await tool.handler({ commerce, params: { limit: 5 } });
    assert.equal(result.returned, 5);
    assert.equal(result.signals.length, 5);
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeFraudCommerce({
      listSignals: async () => {
        throw new Error('signals query failed');
      },
    });
    await assert.rejects(() => tool.handler({ commerce, params: {} }), /signals query failed/);
  });
});

// ============================================================================
// create_fraud_rule
// ============================================================================

describe('create_fraud_rule', () => {
  const tool = findTool(fraudTools, 'create_fraud_rule');

  it('returns preview (success: false) without --apply', async () => {
    const result = await tool.handler({
      commerce: makeFraudCommerce(),
      params: {
        name: 'High Value Alert',
        condition: { field: 'order_amount', operator: 'gt', value: 5000 },
        action: 'review',
      },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
    assert.ok(result.hint);
  });

  it('creates rule with --apply and returns success: true', async () => {
    const result = await tool.handler({
      commerce: makeFraudCommerce(),
      params: {
        name: 'High Value Alert',
        condition: { field: 'order_amount', operator: 'gt', value: 5000 },
        action: 'review',
        scoreAdjustment: 30,
        priority: 50,
        enabled: true,
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('created'));
    assert.ok(result.rule);
    assert.equal(result.rule.name, 'High Value Alert');
  });

  it('passes all fields to commerce.fraud.createRule', async () => {
    let calledWith;
    const commerce = makeFraudCommerce({
      createRule: async (data) => {
        calledWith = data;
        return { ...mockRule, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: {
        name: 'Foreign IP Block',
        description: 'Block orders from high-risk countries',
        condition: { field: 'shipping_country', operator: 'in', value: ['CN', 'RU'] },
        action: 'block',
        scoreAdjustment: 80,
        priority: 10,
        enabled: true,
      },
      allowApply: true,
    });
    assert.equal(calledWith.name, 'Foreign IP Block');
    assert.equal(calledWith.action, 'block');
    assert.equal(calledWith.condition.operator, 'in');
    assert.deepEqual(calledWith.condition.value, ['CN', 'RU']);
    assert.equal(calledWith.priority, 10);
  });

  it('defaults priority to 50 and enabled to true when not provided', async () => {
    let calledWith;
    const commerce = makeFraudCommerce({
      createRule: async (data) => {
        calledWith = data;
        return { ...mockRule, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: {
        name: 'Simple Rule',
        condition: { field: 'order_amount', operator: 'gt', value: 100 },
        action: 'flag',
      },
      allowApply: true,
    });
    assert.equal(calledWith.priority, 50);
    assert.equal(calledWith.enabled, true);
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeFraudCommerce({
      createRule: async () => {
        throw new Error('rule creation failed');
      },
    });
    await assert.rejects(
      () =>
        tool.handler({
          commerce,
          params: {
            name: 'Test Rule',
            condition: { field: 'order_amount', operator: 'gt', value: 100 },
            action: 'flag',
          },
          allowApply: true,
        }),
      /rule creation failed/,
    );
  });
});

// ============================================================================
// update_fraud_rule
// ============================================================================

describe('update_fraud_rule', () => {
  const tool = findTool(fraudTools, 'update_fraud_rule');

  it('returns preview (success: false) without --apply', async () => {
    const result = await tool.handler({
      commerce: makeFraudCommerce(),
      params: { ruleId: 'fr_001', enabled: false },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
  });

  it('updates rule with --apply and returns success: true', async () => {
    const result = await tool.handler({
      commerce: makeFraudCommerce(),
      params: { ruleId: 'fr_001', name: 'Updated Alert', enabled: false },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('updated'));
    assert.ok(result.rule);
  });

  it('passes ruleId and partial update data to commerce.fraud.updateRule', async () => {
    let calledId, calledData;
    const commerce = makeFraudCommerce({
      updateRule: async (id, data) => {
        calledId = id;
        calledData = data;
        return { ...mockRule, id, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: {
        ruleId: 'fr_001',
        name: 'Revised Rule',
        action: 'flag',
        priority: 20,
        enabled: false,
      },
      allowApply: true,
    });
    assert.equal(calledId, 'fr_001');
    assert.equal(calledData.name, 'Revised Rule');
    assert.equal(calledData.action, 'flag');
    assert.equal(calledData.priority, 20);
    assert.equal(calledData.enabled, false);
  });

  it('supports updating condition', async () => {
    let calledData;
    const commerce = makeFraudCommerce({
      updateRule: async (_id, data) => {
        calledData = data;
        return { ...mockRule, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: {
        ruleId: 'fr_001',
        condition: { field: 'order_amount', operator: 'gte', value: 10000 },
      },
      allowApply: true,
    });
    assert.equal(calledData.condition.operator, 'gte');
    assert.equal(calledData.condition.value, 10000);
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeFraudCommerce({
      updateRule: async () => {
        throw new Error('rule not found');
      },
    });
    await assert.rejects(
      () =>
        tool.handler({ commerce, params: { ruleId: 'fr_nope', enabled: false }, allowApply: true }),
      /rule not found/,
    );
  });
});

// ============================================================================
// review_flagged_order
// ============================================================================

describe('review_flagged_order', () => {
  const tool = findTool(fraudTools, 'review_flagged_order');

  it('returns preview (success: false) without --apply', async () => {
    const result = await tool.handler({
      commerce: makeFraudCommerce(),
      params: { assessmentId: 'fa_001', decision: 'approve', reason: 'Verified customer' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
  });

  it('approves flagged order with --apply', async () => {
    const result = await tool.handler({
      commerce: makeFraudCommerce(),
      params: { assessmentId: 'fa_001', decision: 'approve', reason: 'Verified customer identity' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('approved'));
    assert.ok(result.assessment);
  });

  it('rejects flagged order with --apply', async () => {
    const result = await tool.handler({
      commerce: makeFraudCommerce(),
      params: { assessmentId: 'fa_001', decision: 'reject', reason: 'Confirmed fraud attempt' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('rejected'));
  });

  it('escalates flagged order with --apply', async () => {
    const result = await tool.handler({
      commerce: makeFraudCommerce(),
      params: { assessmentId: 'fa_001', decision: 'escalate', reason: 'Needs senior review' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('escalated'));
  });

  it('passes all fields including optional reviewerNote to commerce.fraud.reviewOrder', async () => {
    let calledWith;
    const commerce = makeFraudCommerce({
      reviewOrder: async (data) => {
        calledWith = data;
        return { ...mockAssessment, reviewStatus: data.decision };
      },
    });
    await tool.handler({
      commerce,
      params: {
        assessmentId: 'fa_001',
        decision: 'approve',
        reason: 'Customer confirmed purchase',
        reviewerNote: 'Spoke with customer directly',
      },
      allowApply: true,
    });
    assert.equal(calledWith.assessmentId, 'fa_001');
    assert.equal(calledWith.decision, 'approve');
    assert.equal(calledWith.reason, 'Customer confirmed purchase');
    assert.equal(calledWith.reviewerNote, 'Spoke with customer directly');
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeFraudCommerce({
      reviewOrder: async () => {
        throw new Error('assessment already reviewed');
      },
    });
    await assert.rejects(
      () =>
        tool.handler({
          commerce,
          params: { assessmentId: 'fa_001', decision: 'approve', reason: 'OK' },
          allowApply: true,
        }),
      /assessment already reviewed/,
    );
  });
});
