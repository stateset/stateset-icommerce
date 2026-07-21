import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ALERT_ACTION_MAP, mapAlertToAction } from '../../src/heartbeat/alert-actions.js';

const KNOWN_CHECK_IDS = [
  'low-stock',
  'abandoned-carts',
  'overdue-invoices',
  'pending-returns',
  'subscription-churn',
  'revenue-milestone',
];

describe('ALERT_ACTION_MAP', () => {
  it('contains all 6 expected check IDs', () => {
    for (const id of KNOWN_CHECK_IDS) {
      assert.ok(id in ALERT_ACTION_MAP, `Missing check ID: ${id}`);
    }
    assert.equal(Object.keys(ALERT_ACTION_MAP).length, 6);
  });

  it('each entry has action (string), description (string), and channels (array)', () => {
    for (const [id, entry] of Object.entries(ALERT_ACTION_MAP)) {
      assert.equal(typeof entry.action, 'string', `${id}.action must be a string`);
      assert.equal(typeof entry.description, 'string', `${id}.description must be a string`);
      assert.ok(Array.isArray(entry.channels), `${id}.channels must be an array`);
    }
  });

  it('no entry has an empty channels array', () => {
    for (const [id, entry] of Object.entries(ALERT_ACTION_MAP)) {
      assert.ok(entry.channels.length > 0, `${id}.channels must not be empty`);
    }
  });

  it('all action values are either "notify" or "celebrate"', () => {
    const validActions = new Set(['notify', 'celebrate']);
    for (const [id, entry] of Object.entries(ALERT_ACTION_MAP)) {
      assert.ok(
        validActions.has(entry.action),
        `${id}.action "${entry.action}" is not a valid value`,
      );
    }
  });

  it('every entry has at least one channel', () => {
    for (const [id, entry] of Object.entries(ALERT_ACTION_MAP)) {
      assert.ok(entry.channels.length >= 1, `${id} must have at least one channel`);
    }
  });
});

describe('mapAlertToAction', () => {
  it('returns null for an unknown checkId', () => {
    const result = mapAlertToAction({ checkId: 'totally-unknown-check' });
    assert.equal(result, null);
  });

  it('returns an enriched object for a known checkId', () => {
    const alert = {
      checkId: 'low-stock',
      checkName: 'Low Stock',
      status: 'alert',
      details: {},
      timestamp: Date.now(),
    };
    const result = mapAlertToAction(alert);
    assert.notEqual(result, null);
    assert.equal(typeof result, 'object');
  });

  it('result includes the original alert object', () => {
    const alert = {
      checkId: 'low-stock',
      checkName: 'Low Stock',
      status: 'alert',
      details: { count: 3 },
      timestamp: 1000,
    };
    const result = mapAlertToAction(alert);
    assert.deepEqual(result.alert, alert);
  });

  it('result includes suggestedAt as a positive number', () => {
    const alert = { checkId: 'low-stock' };
    const result = mapAlertToAction(alert);
    assert.equal(typeof result.suggestedAt, 'number');
    assert.ok(result.suggestedAt > 0, 'suggestedAt must be > 0');
  });

  it('result includes action from the map', () => {
    const alert = { checkId: 'low-stock' };
    const result = mapAlertToAction(alert);
    assert.equal(result.action, ALERT_ACTION_MAP['low-stock'].action);
  });

  it('result includes description from the map', () => {
    const alert = { checkId: 'low-stock' };
    const result = mapAlertToAction(alert);
    assert.equal(result.description, ALERT_ACTION_MAP['low-stock'].description);
  });

  it('result includes channels from the map', () => {
    const alert = { checkId: 'low-stock' };
    const result = mapAlertToAction(alert);
    assert.deepEqual(result.channels, ALERT_ACTION_MAP['low-stock'].channels);
  });

  it('handles alert with missing checkId gracefully (returns null)', () => {
    const result = mapAlertToAction({});
    assert.equal(result, null);
  });

  it('handles alert with empty string checkId (returns null)', () => {
    const result = mapAlertToAction({ checkId: '' });
    assert.equal(result, null);
  });

  it('each of the 6 known IDs returns the correct action type', () => {
    const expectedActions = {
      'low-stock': 'notify',
      'abandoned-carts': 'notify',
      'overdue-invoices': 'notify',
      'pending-returns': 'notify',
      'subscription-churn': 'notify',
      'revenue-milestone': 'celebrate',
    };

    for (const [checkId, expectedAction] of Object.entries(expectedActions)) {
      const result = mapAlertToAction({ checkId });
      assert.notEqual(result, null, `Expected non-null result for ${checkId}`);
      assert.equal(
        result.action,
        expectedAction,
        `${checkId} should have action "${expectedAction}"`,
      );
    }
  });

  it('suggestedAt is close to Date.now()', () => {
    const before = Date.now();
    const alert = { checkId: 'low-stock' };
    const result = mapAlertToAction(alert);
    const after = Date.now();
    assert.ok(result.suggestedAt >= before, 'suggestedAt should be >= before timestamp');
    assert.ok(result.suggestedAt <= after, 'suggestedAt should be <= after timestamp');
  });
});
