/**
 * Fraud API tests for @stateset/embedded Node.js bindings.
 *
 * Rule CRUD, assessment creation, listing, and manual review.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { randomUUID } = require('node:crypto');

test('Fraud: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');
  const orderId = randomUUID();

  await t.test('API exists and is supported', async () => {
    assert.ok(commerce.fraud, 'fraud API should exist');
    assert.equal(await commerce.fraud.isSupported(), true);
  });

  let rule;
  await t.test('createRule stores a snake_case signal type and action', async () => {
    rule = await commerce.fraud.createRule({
      name: 'High velocity',
      description: 'Reject rapid repeat orders',
      signalType: 'velocity_spike',
      threshold: 0.8,
      action: 'reject',
    });
    assert.ok(rule.id);
    assert.equal(rule.signalType, 'velocity_spike');
    assert.equal(rule.action, 'reject');
    assert.equal(rule.enabled, true);
  });

  await t.test('createRule rejects an unknown signal type', async () => {
    await assert.rejects(
      () =>
        commerce.fraud.createRule({
          name: 'bad',
          signalType: 'not_a_signal',
          threshold: 0.5,
          action: 'review',
        }),
      /Invalid fraud signal type/,
    );
  });

  await t.test('getRule returns the rule, and null when missing', async () => {
    const found = await commerce.fraud.getRule(rule.id);
    assert.equal(found.name, 'High velocity');
    assert.equal(await commerce.fraud.getRule(randomUUID()), null);
  });

  await t.test('updateRule changes threshold and action', async () => {
    const updated = await commerce.fraud.updateRule(rule.id, {
      threshold: 0.6,
      action: 'review',
    });
    assert.equal(updated.threshold, 0.6);
    assert.equal(updated.action, 'review');
  });

  await t.test('listRules and getActiveRules include the rule', async () => {
    const rules = await commerce.fraud.listRules({ signalType: 'velocity_spike' });
    assert.ok(rules.some((r) => r.id === rule.id));
    const active = await commerce.fraud.getActiveRules();
    assert.ok(active.some((r) => r.id === rule.id));
  });

  let assessment;
  await t.test('createAssessment scores signals and decides', async () => {
    assessment = await commerce.fraud.createAssessment({
      orderId,
      signals: [
        { signalType: 'velocity_spike', score: 0.9, details: 'six orders in an hour' },
        { signalType: 'address_mismatch', score: 0.4, details: 'billing != shipping' },
      ],
    });
    assert.equal(assessment.orderId, orderId);
    assert.equal(assessment.riskScore, 0.9);
    assert.equal(assessment.signals.length, 2);
    assert.equal(assessment.decision, 'review');
    assert.equal(assessment.reviewedBy ?? null, null);
    assert.equal(assessment.needsReview, true);
  });

  await t.test('getAssessment returns it, and null when missing', async () => {
    const found = await commerce.fraud.getAssessment(orderId);
    assert.equal(found.orderId, orderId);
    assert.equal(await commerce.fraud.getAssessment(randomUUID()), null);
  });

  await t.test('listAssessments filters by decision', async () => {
    const listed = await commerce.fraud.listAssessments({ decision: 'review' });
    assert.ok(listed.some((a) => a.orderId === orderId));
  });

  await t.test('reviewAssessment records the reviewer decision', async () => {
    const reviewed = await commerce.fraud.reviewAssessment(
      orderId,
      'accept',
      'risk-analyst',
      'verified by phone',
    );
    assert.equal(reviewed.decision, 'accept');
    assert.equal(reviewed.reviewedBy, 'risk-analyst');
    assert.equal(reviewed.reviewNotes, 'verified by phone');
    assert.equal(reviewed.needsReview, false);
  });

  await t.test('deleteRule removes the rule', async () => {
    await commerce.fraud.deleteRule(rule.id);
    assert.equal(await commerce.fraud.getRule(rule.id), null);
  });
});
