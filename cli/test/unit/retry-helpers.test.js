/**
 * Unit tests for retry-helpers.js
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import { isRetryableError, computeRetryDelay, sleep } from '../../src/retry-helpers.js';

// ===========================================================================
// isRetryableError
// ===========================================================================

describe('isRetryableError', () => {
  it('returns false for null error', () => {
    assert.strictEqual(isRetryableError(null, { retryableErrors: ['timeout'] }), false);
  });

  it('returns false for undefined error', () => {
    assert.strictEqual(isRetryableError(undefined, { retryableErrors: ['timeout'] }), false);
  });

  it('returns false when no patterns are configured', () => {
    assert.strictEqual(isRetryableError(new Error('timeout'), {}), false);
  });

  it('returns false when retryableErrors is empty', () => {
    assert.strictEqual(isRetryableError(new Error('timeout'), { retryableErrors: [] }), false);
  });

  it('returns false when retrySettings is undefined', () => {
    assert.strictEqual(isRetryableError(new Error('timeout'), undefined), false);
  });

  it('matches an Error object message against patterns', () => {
    const err = new Error('Connection timeout occurred');
    assert.strictEqual(isRetryableError(err, { retryableErrors: ['timeout'] }), true);
  });

  it('matches a string error against patterns', () => {
    assert.strictEqual(
      isRetryableError('rate limit exceeded', { retryableErrors: ['rate limit'] }),
      true,
    );
  });

  it('performs case-insensitive matching', () => {
    assert.strictEqual(
      isRetryableError(new Error('RATE LIMIT exceeded'), { retryableErrors: ['rate limit'] }),
      true,
    );
  });

  it('performs case-insensitive matching on patterns too', () => {
    assert.strictEqual(
      isRetryableError(new Error('rate limit'), { retryableErrors: ['RATE LIMIT'] }),
      true,
    );
  });

  it('returns true if any pattern matches', () => {
    const err = new Error('server overloaded');
    assert.strictEqual(
      isRetryableError(err, { retryableErrors: ['timeout', 'overloaded', 'rate limit'] }),
      true,
    );
  });

  it('returns false when no pattern matches', () => {
    const err = new Error('invalid input');
    assert.strictEqual(
      isRetryableError(err, { retryableErrors: ['timeout', 'overloaded'] }),
      false,
    );
  });

  it('handles Error with empty message', () => {
    const err = new Error('');
    assert.strictEqual(isRetryableError(err, { retryableErrors: ['timeout'] }), false);
  });

  it('handles Error with no message property', () => {
    const err = { code: 'ETIMEDOUT' }; // no .message
    assert.strictEqual(isRetryableError(err, { retryableErrors: ['timeout'] }), false);
  });

  it('matches substring within error message', () => {
    const err = new Error('HTTP 429: Too Many Requests - rate limit');
    assert.strictEqual(isRetryableError(err, { retryableErrors: ['429'] }), true);
  });

  it('converts non-string patterns to string', () => {
    const err = new Error('error code 503');
    assert.strictEqual(isRetryableError(err, { retryableErrors: [503] }), true);
  });
});

// ===========================================================================
// computeRetryDelay
// ===========================================================================

describe('computeRetryDelay', () => {
  it('returns base delay for attempt 1 with no jitter', () => {
    const delay = computeRetryDelay(1, { baseDelayMs: 500, maxDelayMs: 8000, jitter: 0 });
    assert.strictEqual(delay, 500);
  });

  it('doubles delay for each subsequent attempt', () => {
    const d1 = computeRetryDelay(1, { baseDelayMs: 500, maxDelayMs: 32000, jitter: 0 });
    const d2 = computeRetryDelay(2, { baseDelayMs: 500, maxDelayMs: 32000, jitter: 0 });
    const d3 = computeRetryDelay(3, { baseDelayMs: 500, maxDelayMs: 32000, jitter: 0 });
    const d4 = computeRetryDelay(4, { baseDelayMs: 500, maxDelayMs: 32000, jitter: 0 });
    assert.strictEqual(d1, 500);
    assert.strictEqual(d2, 1000);
    assert.strictEqual(d3, 2000);
    assert.strictEqual(d4, 4000);
  });

  it('caps delay at maxDelayMs', () => {
    const delay = computeRetryDelay(10, { baseDelayMs: 500, maxDelayMs: 8000, jitter: 0 });
    assert.strictEqual(delay, 8000);
  });

  it('uses default base of 500 when not specified', () => {
    const delay = computeRetryDelay(1, { maxDelayMs: 8000, jitter: 0 });
    assert.strictEqual(delay, 500);
  });

  it('uses default max of 8000 when not specified', () => {
    const delay = computeRetryDelay(20, { baseDelayMs: 500, jitter: 0 });
    assert.strictEqual(delay, 8000);
  });

  it('applies jitter within expected range', () => {
    // With jitter=0.5, the multiplier is (1 + rand) where rand is in [-0.5, 0.5]
    // So delay should be base * [0.5, 1.5]
    const settings = { baseDelayMs: 1000, maxDelayMs: 16000, jitter: 0.5 };
    const delays = [];
    for (let i = 0; i < 100; i++) {
      delays.push(computeRetryDelay(1, settings));
    }
    const min = Math.min(...delays);
    const max = Math.max(...delays);
    // base=1000, jitter=0.5 => range [500, 1500]
    assert.ok(min >= 0, `Min delay should be >= 0, got ${min}`);
    assert.ok(max <= 1500, `Max delay should be <= 1500, got ${max}`);
  });

  it('returns non-negative delay even with full jitter', () => {
    const settings = { baseDelayMs: 100, maxDelayMs: 16000, jitter: 1.0 };
    for (let i = 0; i < 50; i++) {
      const delay = computeRetryDelay(1, settings);
      assert.ok(delay >= 0, `Delay must be non-negative, got ${delay}`);
    }
  });

  it('handles attempt 0 gracefully', () => {
    // Math.max(0, 0-1) = 0 => 2^0 = 1 => base * 1
    const delay = computeRetryDelay(0, { baseDelayMs: 500, maxDelayMs: 8000, jitter: 0 });
    assert.strictEqual(delay, 500);
  });

  it('handles negative attempt gracefully', () => {
    // Math.max(0, -1-1) = 0 => 2^0 = 1 => base * 1
    const delay = computeRetryDelay(-1, { baseDelayMs: 500, maxDelayMs: 8000, jitter: 0 });
    assert.strictEqual(delay, 500);
  });

  it('returns integer delay when jitter is applied', () => {
    const settings = { baseDelayMs: 1000, maxDelayMs: 16000, jitter: 0.3 };
    const delay = computeRetryDelay(2, settings);
    assert.strictEqual(delay, Math.floor(delay));
  });

  it('does not apply jitter when jitter is 0', () => {
    const settings = { baseDelayMs: 1000, maxDelayMs: 16000, jitter: 0 };
    // Should always be deterministic
    const d1 = computeRetryDelay(3, settings);
    const d2 = computeRetryDelay(3, settings);
    assert.strictEqual(d1, d2);
    assert.strictEqual(d1, 4000);
  });
});

// ===========================================================================
// sleep
// ===========================================================================

describe('sleep', () => {
  it('returns a promise', () => {
    const result = sleep(0);
    assert.ok(result instanceof Promise);
  });

  it('resolves after the given delay', async () => {
    const start = Date.now();
    await sleep(50);
    const elapsed = Date.now() - start;
    // Allow some tolerance for timer imprecision
    assert.ok(elapsed >= 40, `Expected at least 40ms, got ${elapsed}ms`);
  });

  it('resolves with undefined', async () => {
    const result = await sleep(0);
    assert.strictEqual(result, undefined);
  });

  it('completes quickly for 0ms sleep', async () => {
    const start = Date.now();
    await sleep(0);
    const elapsed = Date.now() - start;
    assert.ok(elapsed < 50, `0ms sleep took ${elapsed}ms`);
  });
});
