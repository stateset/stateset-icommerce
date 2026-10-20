import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  createFinalityTracker,
  getFinalityRequirement,
} from '../../src/a2a/settlement-finality.js';

function finalTracker(settled = false) {
  const tracker = createFinalityTracker();
  tracker.trackSettlement('payment', 'transaction', 'base', 100);
  tracker.updateConfirmations('payment', 2, 101);
  if (settled) tracker.markSettled('payment');
  return tracker;
}

test('confirmation regression revokes final and application-settled payments', () => {
  for (const settled of [false, true]) {
    const tracker = finalTracker(settled);
    const events = [];
    tracker.events.on('settlement_reorged', (event) => events.push(event));
    const result = tracker.updateConfirmations('payment', 1, 100);
    assert.equal(result.state, 'reorged');
    assert.equal(result.finalAt, null);
    assert.equal(tracker.checkFinality('payment').isFinal, false);
    assert.equal(tracker.getSettlementStatus('payment').isFinal, false);
    assert.equal(tracker.listPending()[0].intentId, 'payment');
    assert.equal(events.length, 1);
    assert.equal(events[0].reason, 'confirmation_regression');
    assert.throws(() => tracker.markSettled('payment'), /expected final/);
  }
});

test('an observed head below the inclusion block revokes finality even with a stale count', () => {
  const tracker = finalTracker(true);
  const result = tracker.updateConfirmations('payment', 2, 99);
  assert.equal(result.state, 'reorged');
  assert.equal(result.confirmations, 0);
  assert.equal(result.latestBlock, 99);
  assert.equal(result.finalAt, null);
  assert.equal(tracker.checkFinality('payment').isFinal, false);
});

test('malformed counts and block heights refuse without partial state changes', () => {
  const bad = [-1, 0.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1, '2', null, undefined];
  const tracker = finalTracker();
  const original = tracker.getSettlementStatus('payment');
  const metrics = tracker.getMetrics();
  for (const value of bad) {
    assert.throws(() => tracker.updateConfirmations('payment', value, 101), /confirmations/);
    assert.throws(() => tracker.updateConfirmations('payment', 2, value), /latestBlock/);
    assert.throws(() => tracker.detectReorg('payment', value), /newBlockNumber/);
    assert.throws(
      () => tracker.trackSettlement('other', 'transaction', 'base', value),
      /blockNumber/,
    );
    assert.deepEqual(tracker.getSettlementStatus('payment'), original);
    assert.deepEqual(tracker.getMetrics(), metrics);
  }
});

test('confirmation count cannot exceed the observed inclusion-to-head window', () => {
  const tracker = createFinalityTracker();
  tracker.trackSettlement('payment', 'transaction', 'base', 100);
  assert.throws(() => tracker.updateConfirmations('payment', 2, 100), /inclusion.*head/i);
  assert.equal(tracker.checkFinality('payment').isFinal, false);
});

test('failed settlements remain terminal after a reorg and repeated failures are idempotent', () => {
  const tracker = finalTracker();
  const events = [];
  tracker.events.on('settlement_failed', (event) => events.push(event));
  tracker.markFailed('payment', 'reverted');
  const failed = tracker.getSettlementStatus('payment');
  tracker.markFailed('payment', 'retry');
  tracker.detectReorg('payment', 99);
  tracker.updateConfirmations('payment', 2, 101);
  assert.deepEqual(tracker.getSettlementStatus('payment'), failed);
  assert.equal(tracker.getMetrics().totalFailed, 1);
  assert.equal(tracker.getMetrics().totalFinal, 0);
  assert.equal(events.length, 1);
});

test('refinality reports current counts and one reorg event per invalidation episode', () => {
  const tracker = finalTracker(true);
  const reorgs = [];
  const finals = [];
  tracker.events.on('settlement_reorged', (event) => reorgs.push(event));
  tracker.events.on('settlement_final', (event) => finals.push(event));
  tracker.updateConfirmations('payment', 1, 100);
  tracker.updateConfirmations('payment', 0, 100);
  tracker.detectReorg('payment', 99);
  tracker.detectReorg('payment', 99);
  assert.equal(reorgs.length, 1);
  assert.equal(tracker.getMetrics().totalFinal, 0);
  assert.equal(tracker.getMetrics().finalityRate, 0);
  assert.equal(tracker.getMetrics().totalReorgs, 1);
  tracker.updateConfirmations('payment', 1, 100);
  assert.equal(tracker.checkFinality('payment').isFinal, false);
  tracker.updateConfirmations('payment', 2, 101);
  assert.equal(tracker.getMetrics().totalFinal, 1);
  assert.equal(tracker.getMetrics().finalityRate, 1);
  assert.equal(finals.length, 1);
  tracker.markSettled('payment');
  const settled = tracker.getSettlementStatus('payment');
  tracker.markSettled('payment');
  assert.deepEqual(tracker.getSettlementStatus('payment'), settled);
});

test('explicit reorg clears final timestamps, records the observed head and cannot duplicate metrics', () => {
  const tracker = finalTracker(true);
  tracker.detectReorg('payment', 99);
  tracker.detectReorg('payment', 98);
  const status = tracker.getSettlementStatus('payment');
  assert.equal(status.finalAt, null);
  assert.equal(status.latestBlock, 98);
  assert.equal(tracker.getMetrics().totalFinal, 0);
  assert.equal(tracker.getMetrics().totalReorgs, 1);
});

test('operator confirmation overrides are validated and isolated from later mutation', () => {
  const requirements = { base: 3 };
  const tracker = createFinalityTracker({ confirmationRequirements: requirements });
  requirements.base = 0;
  tracker.trackSettlement('payment', 'transaction', 'base', 100);
  tracker.updateConfirmations('payment', 2, 101);
  assert.equal(tracker.checkFinality('payment').isFinal, false);
  assert.equal(tracker.checkFinality('payment').required, 3);
  tracker.updateConfirmations('payment', 3, 102);
  assert.equal(tracker.checkFinality('payment').isFinal, true);
  assert.equal(getFinalityRequirement('base'), 2);
  assert.throws(() => {
    tracker.CHAIN_FINALITY_REQUIREMENTS.base = 0;
  }, TypeError);
  assert.throws(() => {
    tracker.FinalityState.FINAL = 'broadcast';
  }, TypeError);
  for (const count of [0, -1, 0.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1, '2']) {
    assert.throws(
      () => createFinalityTracker({ confirmationRequirements: { base: count } }),
      /confirmation/i,
    );
  }
});

test('prototype property names are ordinary unknown chains with conservative depth', () => {
  for (const chain of ['__proto__', 'constructor', 'toString']) {
    assert.equal(getFinalityRequirement(chain), 12);
    const tracker = createFinalityTracker();
    const record = tracker.trackSettlement('payment', 'transaction', chain, 100);
    assert.equal(record.requiredConfirmations, 12);
    tracker.updateConfirmations('payment', 11, 110);
    assert.equal(tracker.checkFinality('payment').isFinal, false);
  }
});

test('tracking refuses absent identities before allocating any state', () => {
  const tracker = createFinalityTracker();
  for (const value of ['', '  ', null, undefined, 1]) {
    assert.throws(() => tracker.trackSettlement(value, 'transaction', 'base', 100), /intentId/);
    assert.throws(() => tracker.trackSettlement('payment', value, 'base', 100), /txHash/);
    assert.throws(() => tracker.trackSettlement('payment', 'transaction', value, 100), /chain/);
  }
  assert.equal(tracker.getMetrics().totalTracked, 0);
});
