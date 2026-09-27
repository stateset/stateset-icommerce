/**
 * Fraud Detection Tools — handler behaviour
 *
 * Mocks here define only methods the Node binding's `Fraud` class really has
 * (checked against bindings/node/index.d.ts in fraud-tools.test.js), and each
 * test asserts the exact arguments the binding receives.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { fraudTools } from '../../src/tools/fraud.js';

function findTool(name) {
  const tool = fraudTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock data — shaped like FraudAssessmentOutput / FraudRuleOutput
// ============================================================================

const ORDER_ID = '11111111-1111-4111-8111-111111111111';

const mockSignal = {
  orderId: ORDER_ID,
  signalType: 'proxy_vpn',
  score: 0.85,
  details: 'known VPN exit',
  detectedAt: '2026-02-01T00:00:00Z',
};

const mockAssessment = {
  orderId: ORDER_ID,
  riskScore: 0.85,
  signals: [mockSignal],
  decision: 'review',
  needsReview: true,
  createdAt: '2026-02-01T00:00:00Z',
  updatedAt: '2026-02-01T00:00:00Z',
};

const mockRule = {
  id: 'fr_001',
  name: 'VPN',
  signalType: 'proxy_vpn',
  threshold: 0.7,
  action: 'reject',
  enabled: true,
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-01T00:00:00Z',
};

/** A `commerce.fraud` mock that records every call. */
function makeCommerce(overrides = {}) {
  const calls = [];
  const record =
    (name, impl) =>
    async (...args) => {
      calls.push({ name, args });
      return impl(...args);
    };
  const fraud = {
    createAssessment: record('createAssessment', (input) => ({
      ...mockAssessment,
      orderId: input.orderId,
    })),
    getAssessment: record('getAssessment', (orderId) =>
      orderId === ORDER_ID ? mockAssessment : null,
    ),
    listAssessments: record('listAssessments', () => [
      mockAssessment,
      {
        ...mockAssessment,
        orderId: 'o2',
        signals: [{ ...mockSignal, orderId: 'o2', signalType: 'address_mismatch', score: 0.3 }],
      },
    ]),
    reviewAssessment: record('reviewAssessment', (orderId, decision, reviewer, notes) => ({
      ...mockAssessment,
      orderId,
      decision,
      needsReview: false,
      reviewedBy: reviewer,
      reviewNotes: notes,
    })),
    createRule: record('createRule', (input) => ({ ...mockRule, ...input })),
    updateRule: record('updateRule', (id, input) => ({ ...mockRule, id, ...input })),
  };
  for (const [name, impl] of Object.entries(overrides)) fraud[name] = record(name, impl);
  return { commerce: { fraud }, calls };
}

// ============================================================================
// assess_order_fraud
// ============================================================================

describe('assess_order_fraud', () => {
  const tool = findTool('assess_order_fraud');
  const params = {
    orderId: ORDER_ID,
    signals: [{ signalType: 'proxy_vpn', score: 0.85, details: 'known VPN exit' }],
  };

  it('previews without --apply, because the assessment is persisted', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({ commerce, params });
    assert.equal(result.success, false);
    assert.match(result.error, /--apply/);
    assert.equal(calls.length, 0);
  });

  it('calls commerce.fraud.createAssessment with the order and signals', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({ commerce, params, allowApply: true });
    assert.deepEqual(calls, [
      {
        name: 'createAssessment',
        args: [
          {
            orderId: ORDER_ID,
            signals: [{ signalType: 'proxy_vpn', score: 0.85, details: 'known VPN exit' }],
          },
        ],
      },
    ]);
    assert.equal(result.success, true);
    assert.equal(result.assessment.orderId, ORDER_ID);
    assert.equal(result.assessment.riskScore, 0.85);
    assert.equal(result.assessment.decision, 'review');
    assert.equal(result.assessment.needsReview, true);
    assert.deepEqual(result.assessment.signals, [mockSignal]);
  });

  it('rejects an unknown signal type at the schema', () => {
    const r = tool.inputSchema.signals.safeParse([
      { signalType: 'velocity_check', score: 0.2, details: 'x' },
    ]);
    assert.equal(r.success, false);
  });

  it('rejects a score outside 0..1 at the schema', () => {
    const r = tool.inputSchema.signals.safeParse([
      { signalType: 'proxy_vpn', score: 35, details: 'x' },
    ]);
    assert.equal(r.success, false);
  });

  it('propagates binding errors', async () => {
    const { commerce } = makeCommerce({
      createAssessment: () => {
        throw new Error('UNIQUE constraint failed: fraud_assessments.order_id');
      },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params, allowApply: true }),
      /UNIQUE constraint/,
    );
  });
});

// ============================================================================
// get_fraud_assessment
// ============================================================================

describe('get_fraud_assessment', () => {
  const tool = findTool('get_fraud_assessment');

  it('looks the assessment up by order ID', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({ commerce, params: { orderId: ORDER_ID } });
    assert.deepEqual(calls, [{ name: 'getAssessment', args: [ORDER_ID] }]);
    assert.equal(result.success, true);
    assert.equal(result.assessment.orderId, ORDER_ID);
    assert.equal(result.assessment.decision, 'review');
    assert.deepEqual(result.assessment.signals, [mockSignal]);
  });

  it('returns success: false when the order has no assessment', async () => {
    const { commerce } = makeCommerce();
    const result = await tool.handler({ commerce, params: { orderId: 'nope' } });
    assert.deepEqual(result, { success: false, error: 'Fraud assessment not found' });
  });
});

// ============================================================================
// list_fraud_signals
// ============================================================================

describe('list_fraud_signals', () => {
  const tool = findTool('list_fraud_signals');

  it('reads one order’s signals from its assessment', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({ commerce, params: { orderId: ORDER_ID, limit: 50 } });
    assert.deepEqual(calls, [{ name: 'getAssessment', args: [ORDER_ID] }]);
    assert.equal(result.returned, 1);
    assert.deepEqual(result.signals, [mockSignal]);
  });

  it('returns no signals for an order with no assessment', async () => {
    const { commerce } = makeCommerce();
    const result = await tool.handler({ commerce, params: { orderId: 'nope', limit: 50 } });
    assert.deepEqual(result, { success: true, returned: 0, signals: [] });
  });

  it('filters one order’s assessment by minRiskScore', async () => {
    const { commerce } = makeCommerce();
    const result = await tool.handler({
      commerce,
      params: { orderId: ORDER_ID, minRiskScore: 0.9, limit: 50 },
    });
    assert.equal(result.returned, 0);
  });

  it('without orderId, lists assessments with minRiskScore and limit and flattens signals', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({ commerce, params: { minRiskScore: 0.2, limit: 50 } });
    assert.deepEqual(calls, [
      { name: 'listAssessments', args: [{ minRiskScore: 0.2, limit: 50 }] },
    ]);
    assert.equal(result.returned, 2);
    assert.deepEqual(
      result.signals.map((s) => [s.orderId, s.signalType]),
      [
        [ORDER_ID, 'proxy_vpn'],
        ['o2', 'address_mismatch'],
      ],
    );
  });

  it('slices the flattened signals to limit', async () => {
    const { commerce } = makeCommerce();
    const result = await tool.handler({ commerce, params: { limit: 1 } });
    assert.equal(result.returned, 1);
  });
});

// ============================================================================
// create_fraud_rule / update_fraud_rule
// ============================================================================

describe('create_fraud_rule', () => {
  const tool = findTool('create_fraud_rule');
  const params = {
    name: 'VPN',
    description: 'Reject VPN traffic',
    signalType: 'proxy_vpn',
    threshold: 0.7,
    action: 'reject',
  };

  it('previews without --apply', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({ commerce, params });
    assert.equal(result.success, false);
    assert.equal(calls.length, 0);
  });

  it('passes a CreateFraudRuleInput to commerce.fraud.createRule', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({ commerce, params, allowApply: true });
    assert.deepEqual(calls, [{ name: 'createRule', args: [params] }]);
    assert.equal(result.success, true);
    assert.equal(result.rule.signalType, 'proxy_vpn');
  });

  it('only accepts FraudDecision values as the action', () => {
    assert.equal(tool.inputSchema.action.safeParse('flag').success, false);
    assert.equal(tool.inputSchema.action.safeParse('review').success, true);
  });
});

describe('update_fraud_rule', () => {
  const tool = findTool('update_fraud_rule');

  it('previews without --apply', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({ commerce, params: { ruleId: 'fr_001', enabled: false } });
    assert.equal(result.success, false);
    assert.equal(calls.length, 0);
  });

  it('passes the rule ID and an UpdateFraudRuleInput to commerce.fraud.updateRule', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({
      commerce,
      params: { ruleId: 'fr_001', threshold: 0.9, enabled: false },
      allowApply: true,
    });
    assert.deepEqual(calls, [
      {
        name: 'updateRule',
        args: [
          'fr_001',
          {
            name: undefined,
            description: undefined,
            threshold: 0.9,
            action: undefined,
            enabled: false,
          },
        ],
      },
    ]);
    assert.equal(result.rule.enabled, false);
  });
});

// ============================================================================
// review_flagged_order
// ============================================================================

describe('review_flagged_order', () => {
  const tool = findTool('review_flagged_order');
  const params = {
    orderId: ORDER_ID,
    decision: 'reject',
    reviewer: 'ops@example.com',
    reason: 'VPN confirmed',
  };

  it('previews without --apply', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({ commerce, params });
    assert.equal(result.success, false);
    assert.equal(calls.length, 0);
  });

  it('calls reviewAssessment(orderId, decision, reviewer, notes)', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await tool.handler({ commerce, params, allowApply: true });
    assert.deepEqual(calls, [
      {
        name: 'reviewAssessment',
        args: [ORDER_ID, 'reject', 'ops@example.com', 'VPN confirmed'],
      },
    ]);
    assert.equal(result.success, true);
    assert.equal(result.message, 'Order rejected');
    assert.equal(result.assessment.decision, 'reject');
    assert.equal(result.assessment.reviewedBy, 'ops@example.com');
    assert.equal(result.assessment.reviewNotes, 'VPN confirmed');
  });

  for (const [decision, message] of [
    ['accept', 'Order accepted'],
    ['review', 'Order kept in review'],
  ]) {
    it(`says "${message}" for ${decision}`, async () => {
      const { commerce } = makeCommerce();
      const result = await tool.handler({
        commerce,
        params: { ...params, decision },
        allowApply: true,
      });
      assert.equal(result.message, message);
    });
  }

  it('only accepts FraudDecision values', () => {
    assert.equal(tool.inputSchema.decision.safeParse('approve').success, false);
    assert.equal(tool.inputSchema.decision.safeParse('escalate').success, false);
  });

  it('propagates binding errors for an order with no assessment', async () => {
    const { commerce } = makeCommerce({
      reviewAssessment: () => {
        throw new Error('Failed to review fraud assessment: Record not found');
      },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params, allowApply: true }),
      /Record not found/,
    );
  });
});
