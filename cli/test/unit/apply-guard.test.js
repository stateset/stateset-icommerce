/**
 * Unit tests for utils/apply-guard.js
 *
 * Tests the applyRequired() helper that returns a standardized
 * response when a write operation is attempted without --apply.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { applyRequired } from '../../src/utils/apply-guard.js';

// ===========================================================================
// Return shape
// ===========================================================================

describe('applyRequired — return shape', () => {
  it('returns success: false', () => {
    const result = applyRequired('Create supplier');
    assert.strictEqual(result.success, false);
  });

  it('returns an error string', () => {
    const result = applyRequired('Create supplier');
    assert.strictEqual(typeof result.error, 'string');
    assert.ok(result.error.length > 0, 'error should not be empty');
  });

  it('returns a hint string', () => {
    const result = applyRequired('Create supplier');
    assert.strictEqual(typeof result.hint, 'string');
    assert.ok(result.hint.includes('--apply'), 'hint should mention --apply');
  });
});

// ===========================================================================
// Operation name in error message
// ===========================================================================

describe('applyRequired — operation name', () => {
  it('includes the operation name in the error message', () => {
    const result = applyRequired('Create supplier');
    assert.ok(
      result.error.includes('Create supplier'),
      `error "${result.error}" should include "Create supplier"`,
    );
  });

  it('includes "requires --apply" phrasing', () => {
    const result = applyRequired('Approve return');
    assert.ok(result.error.includes('--apply'), `error "${result.error}" should include "--apply"`);
  });

  it('works with different operation names', () => {
    const operations = [
      'Ship order',
      'Cancel subscription',
      'Delete customer',
      'Adjust inventory',
      'Complete checkout',
    ];
    for (const op of operations) {
      const result = applyRequired(op);
      assert.strictEqual(result.success, false);
      assert.ok(result.error.includes(op), `error for "${op}" should include the operation name`);
    }
  });
});

// ===========================================================================
// Preview data (wouldDo)
// ===========================================================================

describe('applyRequired — preview data', () => {
  it('includes preview data as wouldDo when provided', () => {
    const preview = { id: 'ORD-123', status: 'shipped' };
    const result = applyRequired('Ship order', preview);
    assert.deepStrictEqual(result.wouldDo, preview);
  });

  it('omits wouldDo when preview is not provided', () => {
    const result = applyRequired('Create supplier');
    assert.strictEqual('wouldDo' in result, false);
  });

  it('omits wouldDo when preview is null', () => {
    const result = applyRequired('Create supplier', null);
    assert.strictEqual('wouldDo' in result, false);
  });

  it('includes wouldDo with an empty object preview', () => {
    const result = applyRequired('Create supplier', {});
    assert.deepStrictEqual(result.wouldDo, {});
  });

  it('includes wouldDo with a complex preview object', () => {
    const preview = {
      customer: { email: 'alice@example.com', name: 'Alice' },
      items: [
        { sku: 'WIDGET-001', qty: 2, price: 29.99 },
        { sku: 'GADGET-002', qty: 1, price: 49.99 },
      ],
      total: 109.97,
    };
    const result = applyRequired('Complete checkout', preview);
    assert.deepStrictEqual(result.wouldDo, preview);
  });

  it('includes wouldDo with an array preview', () => {
    const preview = ['item-1', 'item-2', 'item-3'];
    const result = applyRequired('Bulk delete', preview);
    assert.deepStrictEqual(result.wouldDo, preview);
  });

  it('includes wouldDo with a string preview', () => {
    const result = applyRequired('Echo test', 'some-string');
    assert.strictEqual(result.wouldDo, 'some-string');
  });
});

// ===========================================================================
// Edge cases
// ===========================================================================

describe('applyRequired — edge cases', () => {
  it('works with an empty operation string', () => {
    const result = applyRequired('');
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('preserves exact operation name (no trimming or mutation)', () => {
    const result = applyRequired('  padded name  ');
    assert.ok(result.error.includes('  padded name  '));
  });

  it('returns a fresh object on each call (no shared references)', () => {
    const a = applyRequired('Op A');
    const b = applyRequired('Op B');
    assert.notStrictEqual(a, b);
    assert.ok(a.error.includes('Op A'));
    assert.ok(b.error.includes('Op B'));
  });
});
