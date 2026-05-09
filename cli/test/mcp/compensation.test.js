// Unit tests for cli/src/mcp/compensation.js
//
// Covers:
//  - Static lookup tables have the expected shape
//  - coerceReplayIdSource normalizes string/number/null/undefined
//  - extractReplayIdFromSource walks key candidates in order
//  - _extractFirstIdLikeValue prefers `id`, then any *_id key
//  - buildCompensationParams handles named hints, fallback id, and the
//    "no resolvable params" case (returns null)

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  AGENTIC_COMPENSATION_HINTS,
  AGENTIC_COMPENSATION_PARAM_HINTS,
  AGENTIC_IDEMPOTENCY_HINTS,
  coerceReplayIdSource,
  extractReplayIdFromSource,
  _extractFirstIdLikeValue,
  buildCompensationParams,
} from '../../src/mcp/compensation.js';

describe('compensation lookup tables', () => {
  it('AGENTIC_COMPENSATION_HINTS maps every forward tool to a non-empty list', () => {
    for (const [forward, compensations] of Object.entries(AGENTIC_COMPENSATION_HINTS)) {
      assert.ok(forward.length > 0, `forward tool name should be non-empty`);
      assert.ok(
        Array.isArray(compensations) && compensations.length > 0,
        `compensations for ${forward} should be a non-empty array`,
      );
      for (const c of compensations) {
        assert.equal(typeof c, 'string');
      }
    }
  });

  it('AGENTIC_COMPENSATION_PARAM_HINTS lists at least one key per compensation', () => {
    for (const [comp, keys] of Object.entries(AGENTIC_COMPENSATION_PARAM_HINTS)) {
      assert.ok(comp.length > 0);
      assert.ok(Array.isArray(keys) && keys.length > 0, `${comp} should have keys`);
    }
  });

  it('every compensation tool referenced from HINTS has param hints', () => {
    const referenced = new Set();
    for (const list of Object.values(AGENTIC_COMPENSATION_HINTS)) {
      for (const c of list) referenced.add(c);
    }
    for (const c of referenced) {
      assert.ok(
        AGENTIC_COMPENSATION_PARAM_HINTS[c],
        `compensation ${c} is referenced but has no param hints`,
      );
    }
  });

  it('AGENTIC_IDEMPOTENCY_HINTS is a Set of payment-shaped tool names', () => {
    assert.ok(AGENTIC_IDEMPOTENCY_HINTS instanceof Set);
    assert.ok(AGENTIC_IDEMPOTENCY_HINTS.has('create_payment'));
    assert.ok(AGENTIC_IDEMPOTENCY_HINTS.has('create_refund'));
    assert.ok(!AGENTIC_IDEMPOTENCY_HINTS.has('list_orders'));
  });
});

describe('coerceReplayIdSource', () => {
  it('passes through non-empty strings', () => {
    assert.equal(coerceReplayIdSource('abc-123'), 'abc-123');
  });

  it('stringifies numbers, including 0', () => {
    assert.equal(coerceReplayIdSource(42), '42');
    assert.equal(coerceReplayIdSource(0), '0');
  });

  it('returns undefined for empty / nullish / non-id types', () => {
    assert.equal(coerceReplayIdSource(''), undefined);
    assert.equal(coerceReplayIdSource(null), undefined);
    assert.equal(coerceReplayIdSource(undefined), undefined);
    assert.equal(coerceReplayIdSource({}), undefined);
    assert.equal(coerceReplayIdSource([1, 2]), undefined);
    assert.equal(coerceReplayIdSource(true), undefined);
  });
});

describe('extractReplayIdFromSource', () => {
  it('returns the first key candidate that has a usable value', () => {
    const source = { orderId: 'ord_1', paymentId: 'pay_2' };
    assert.equal(
      extractReplayIdFromSource(source, ['paymentId', 'orderId']),
      'pay_2',
    );
    assert.equal(
      extractReplayIdFromSource(source, ['missing', 'orderId']),
      'ord_1',
    );
  });

  it('returns undefined when no key matches', () => {
    assert.equal(
      extractReplayIdFromSource({ irrelevant: true }, ['orderId']),
      undefined,
    );
  });

  it('returns undefined for non-object sources', () => {
    assert.equal(extractReplayIdFromSource(null, ['id']), undefined);
    assert.equal(extractReplayIdFromSource('foo', ['id']), undefined);
    assert.equal(extractReplayIdFromSource(42, ['id']), undefined);
  });
});

describe('_extractFirstIdLikeValue', () => {
  it('prefers `id` over other *_id keys', () => {
    assert.equal(
      _extractFirstIdLikeValue({ id: 'top', orderId: 'lower' }),
      'top',
    );
  });

  it('falls back to the first *_id key when `id` is missing', () => {
    assert.equal(
      _extractFirstIdLikeValue({ name: 'x', orderId: 'ord_1', paymentId: 'pay_2' }),
      'ord_1',
    );
  });

  it('returns undefined when no id-shaped key exists', () => {
    assert.equal(_extractFirstIdLikeValue({ name: 'x', count: 3 }), undefined);
  });

  it('returns undefined for non-object sources', () => {
    assert.equal(_extractFirstIdLikeValue(null), undefined);
    assert.equal(_extractFirstIdLikeValue('s'), undefined);
  });
});

describe('buildCompensationParams', () => {
  it('uses the named param hint when present in the original params', () => {
    const out = buildCompensationParams('cancel_order', { orderId: 'ord_1' }, null);
    assert.deepEqual(out, { orderId: 'ord_1' });
  });

  it('reads the named hint out of nested result paths (e.g. result.order.id)', () => {
    // cancel_order wants `orderId`. We provide a result with the order's
    // top-level `id`, which is in a hint-eligible location (result.order).
    const out = buildCompensationParams(
      'cancel_order',
      {},
      { order: { id: 'ord_99' } },
    );
    assert.deepEqual(out, { orderId: 'ord_99' });
  });

  it('falls back to {id: <first id-like>} when no named hint matches', () => {
    // A compensation tool we don't have hints for: bypasses the hint loop
    // and lands on the fallback id-extraction. The fallback walks a fixed
    // list of common id key names; here we hand it `customerId`, which is
    // in that list.
    const out = buildCompensationParams(
      'something_unknown',
      { customerId: 'cust_42' },
      null,
    );
    assert.deepEqual(out, { id: 'cust_42' });
  });

  it('returns null when fallback id keys are absent (custom keys only)', () => {
    // The fallback key list is finite — `entityId` is NOT in it, so a
    // params-only object using a custom id name yields null.
    const out = buildCompensationParams(
      'something_unknown',
      { entityId: 'ent_42' },
      null,
    );
    assert.equal(out, null);
  });

  it('returns null when nothing resolvable is present', () => {
    const out = buildCompensationParams('cancel_order', { irrelevant: true }, {});
    assert.equal(out, null);
  });

  it('reads compensation params from result.cart for cart-related tools', () => {
    const out = buildCompensationParams(
      'cancel_cart',
      {},
      { cart: { cartId: 'cart_z' } },
    );
    assert.deepEqual(out, { cartId: 'cart_z' });
  });

  it('reads release_reservation params from result.reservation', () => {
    const out = buildCompensationParams(
      'release_reservation',
      {},
      { reservation: { reservationId: 'res_88' } },
    );
    assert.deepEqual(out, { reservationId: 'res_88' });
  });
});
