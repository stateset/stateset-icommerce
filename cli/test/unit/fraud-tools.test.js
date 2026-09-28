/**
 * Fraud tools — module shape, schemas, apply guard, and binding surface.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';

import { fraudTools } from '../../src/tools/fraud.js';
import { bindingClassMethods, toolModuleCalls } from '../helpers/binding-class-methods.js';

const byName = Object.fromEntries(fraudTools.map((t) => [t.name, t]));
const parse = (name, params) => z.object(byName[name].inputSchema).safeParse(params);

const EXPECTED_NAMES = [
  'assess_order_fraud',
  'get_fraud_assessment',
  'list_fraud_signals',
  'create_fraud_rule',
  'update_fraud_rule',
  'review_flagged_order',
];

describe('fraudTools — module exports', () => {
  it('exports the expected tools in order', () => {
    assert.deepEqual(
      fraudTools.map((t) => t.name),
      EXPECTED_NAMES,
    );
  });

  it('every tool has a handler, description, and inputSchema', () => {
    for (const tool of fraudTools) {
      assert.equal(typeof tool.handler, 'function', tool.name);
      assert.ok(tool.description.length > 0, tool.name);
      assert.equal(typeof tool.inputSchema, 'object', tool.name);
    }
  });

  it('keeps its permission assignments', () => {
    assert.deepEqual(Object.fromEntries(fraudTools.map((t) => [t.name, t.permission])), {
      assess_order_fraud: 'write',
      get_fraud_assessment: 'read',
      list_fraud_signals: 'read',
      create_fraud_rule: 'admin',
      update_fraud_rule: 'admin',
      review_flagged_order: 'write',
    });
  });
});

describe('fraudTools — binding surface', () => {
  const fraudMethods = bindingClassMethods('Fraud');

  it('calls only methods the binding’s Fraud class declares', () => {
    const called = toolModuleCalls('fraud.js', 'fraud');
    assert.ok(called.size >= 6, `only found ${[...called]}`);
    for (const method of called) {
      assert.ok(fraudMethods.has(method), `commerce.fraud.${method} is not on the binding`);
    }
  });

  it('the handler-test mock methods all exist on the binding', () => {
    for (const method of [
      'createAssessment',
      'getAssessment',
      'listAssessments',
      'reviewAssessment',
      'createRule',
      'updateRule',
    ]) {
      assert.ok(fraudMethods.has(method), method);
    }
  });
});

describe('fraudTools — input schemas', () => {
  it('assess_order_fraud takes an orderId and typed signals', () => {
    assert.equal(
      parse('assess_order_fraud', {
        orderId: 'o1',
        signals: [{ signalType: 'geo_ip_anomaly', score: 0.5, details: 'IP in another country' }],
      }).success,
      true,
    );
    assert.equal(parse('assess_order_fraud', { orderId: 'o1' }).success, false);
    // The old address/IP fields meant nothing to the engine and are gone.
    assert.equal('customerIp' in byName.assess_order_fraud.inputSchema, false);
  });

  it('get_fraud_assessment is keyed by orderId', () => {
    assert.deepEqual(Object.keys(byName.get_fraud_assessment.inputSchema), ['orderId']);
  });

  it('list_fraud_signals takes optional orderId, minRiskScore, limit', () => {
    assert.deepEqual(Object.keys(byName.list_fraud_signals.inputSchema).sort(), [
      'limit',
      'minRiskScore',
      'orderId',
    ]);
    assert.equal(parse('list_fraud_signals', {}).success, true);
    assert.equal(parse('list_fraud_signals', { minRiskScore: 2 }).success, false);
  });

  it('create_fraud_rule requires name, signalType, threshold, action', () => {
    assert.equal(parse('create_fraud_rule', { name: 'r' }).success, false);
    assert.equal(
      parse('create_fraud_rule', {
        name: 'r',
        signalType: 'disposable_email',
        threshold: 0.5,
        action: 'review',
      }).success,
      true,
    );
  });

  it('update_fraud_rule requires ruleId only', () => {
    assert.equal(parse('update_fraud_rule', { ruleId: 'r1' }).success, true);
    assert.equal(parse('update_fraud_rule', {}).success, false);
  });

  it('review_flagged_order requires orderId, decision, reviewer, reason', () => {
    assert.equal(
      parse('review_flagged_order', {
        orderId: 'o1',
        decision: 'accept',
        reviewer: 'ops',
        reason: 'ok',
      }).success,
      true,
    );
    assert.equal(
      parse('review_flagged_order', { orderId: 'o1', decision: 'accept', reason: 'ok' }).success,
      false,
    );
  });
});

describe('fraudTools — apply guard', () => {
  const previews = {
    assess_order_fraud: { orderId: 'o1', signals: [] },
    create_fraud_rule: { name: 'r', signalType: 'proxy_vpn', threshold: 0.5, action: 'reject' },
    update_fraud_rule: { ruleId: 'r1', enabled: false },
    review_flagged_order: { orderId: 'o1', decision: 'reject', reviewer: 'ops', reason: 'x' },
  };
  for (const [name, params] of Object.entries(previews)) {
    it(`${name} does not touch the binding without --apply`, async () => {
      const result = await byName[name].handler({ commerce: {}, params });
      assert.equal(result.success, false);
      assert.match(result.error, /--apply/);
      assert.deepEqual(result.wouldDo, params);
    });
  }
});

describe('fraudTools — handler error paths (binding missing)', () => {
  for (const [name, params, allowApply] of [
    ['get_fraud_assessment', { orderId: 'o1' }, false],
    ['list_fraud_signals', { limit: 5 }, false],
    ['assess_order_fraud', { orderId: 'o1', signals: [] }, true],
  ]) {
    it(`${name} throws a TypeError when commerce.fraud is absent`, async () => {
      await assert.rejects(
        () => byName[name].handler({ commerce: {}, params, allowApply }),
        TypeError,
      );
    });
  }
});
