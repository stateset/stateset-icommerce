/**
 * Tests for cli/src/a2a/idempotency.js
 *
 * Covers: createIdempotencyGuard — execute, has, invalidate, getMetrics, clear,
 * concurrent execution, TTL expiry, maxSize eviction, failed execution caching.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import { createIdempotencyGuard } from '../../src/a2a/idempotency.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Tiny async delay. */
function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Create a spy function that tracks calls and returns a value. */
function createSpy(returnValue) {
  let callCount = 0;
  const fn = async () => {
    callCount++;
    return returnValue;
  };
  fn.callCount = () => callCount;
  return fn;
}

/** Create a spy that resolves after a delay (simulates async work). */
function createSlowSpy(returnValue, delayMs) {
  let callCount = 0;
  const fn = async () => {
    callCount++;
    await delay(delayMs);
    return returnValue;
  };
  fn.callCount = () => callCount;
  return fn;
}

// ---------------------------------------------------------------------------
// 1. First execution runs fn and returns result
// ---------------------------------------------------------------------------

describe('IdempotencyGuard — first execution', () => {
  let guard;

  beforeEach(() => {
    guard = createIdempotencyGuard();
  });

  it('executes fn and returns its result', async () => {
    const spy = createSpy({ id: 'pay-1', status: 'ok' });
    const result = await guard.execute('key-1', spy);

    assert.deepEqual(result, { id: 'pay-1', status: 'ok' });
    assert.equal(spy.callCount(), 1);
  });

  it('marks the key as existing after execution', async () => {
    const spy = createSpy(42);
    await guard.execute('key-2', spy);

    assert.equal(guard.has('key-2'), true);
  });

  it('tracks the execution as a miss', async () => {
    const spy = createSpy('result');
    await guard.execute('key-3', spy);

    const metrics = guard.getMetrics();
    assert.equal(metrics.misses, 1);
    assert.equal(metrics.hits, 0);
  });
});

// ---------------------------------------------------------------------------
// 2. Second execution returns cached result without calling fn
// ---------------------------------------------------------------------------

describe('IdempotencyGuard — cached result', () => {
  let guard;

  beforeEach(() => {
    guard = createIdempotencyGuard();
  });

  it('returns cached result on second call', async () => {
    const spy = createSpy({ amount: 100 });

    const first = await guard.execute('pay-abc', spy);
    const second = await guard.execute('pay-abc', spy);

    assert.deepEqual(first, { amount: 100 });
    assert.deepEqual(second, { amount: 100 });
    assert.equal(spy.callCount(), 1, 'fn should only be called once');
  });

  it('counts second call as a hit', async () => {
    const spy = createSpy('val');
    await guard.execute('k', spy);
    await guard.execute('k', spy);

    const metrics = guard.getMetrics();
    assert.equal(metrics.hits, 1);
    assert.equal(metrics.misses, 1);
  });

  it('returns cached result on third, fourth, fifth call', async () => {
    const spy = createSpy('stable');
    for (let i = 0; i < 5; i++) {
      const result = await guard.execute('multi', spy);
      assert.equal(result, 'stable');
    }
    assert.equal(spy.callCount(), 1);
    assert.equal(guard.getMetrics().hits, 4);
  });
});

// ---------------------------------------------------------------------------
// 3. Concurrent executions with same key only run fn once
// ---------------------------------------------------------------------------

describe('IdempotencyGuard — concurrent execution', () => {
  it('runs fn only once for concurrent calls with the same key', async () => {
    const guard = createIdempotencyGuard();
    const spy = createSlowSpy({ paid: true }, 50);

    // Fire three concurrent calls
    const [r1, r2, r3] = await Promise.all([
      guard.execute('concurrent-key', spy),
      guard.execute('concurrent-key', spy),
      guard.execute('concurrent-key', spy),
    ]);

    assert.deepEqual(r1, { paid: true });
    assert.deepEqual(r2, { paid: true });
    assert.deepEqual(r3, { paid: true });
    assert.equal(spy.callCount(), 1, 'fn should execute exactly once');
  });

  it('concurrent waiters all receive the same error if fn fails', async () => {
    const guard = createIdempotencyGuard();
    let callCount = 0;
    const failingSpy = async () => {
      callCount++;
      await delay(30);
      throw new Error('network timeout');
    };

    const results = await Promise.allSettled([
      guard.execute('fail-concurrent', failingSpy),
      guard.execute('fail-concurrent', failingSpy),
    ]);

    assert.equal(results[0].status, 'rejected');
    assert.equal(results[1].status, 'rejected');
    assert.equal(results[0].reason.message, 'network timeout');
    assert.equal(results[1].reason.message, 'network timeout');
    assert.equal(callCount, 1, 'failing fn should only be called once');
  });
});

// ---------------------------------------------------------------------------
// 4. Different keys run independently
// ---------------------------------------------------------------------------

describe('IdempotencyGuard — key isolation', () => {
  it('executes fn independently for different keys', async () => {
    const guard = createIdempotencyGuard();
    const spy1 = createSpy('result-a');
    const spy2 = createSpy('result-b');

    const r1 = await guard.execute('key-a', spy1);
    const r2 = await guard.execute('key-b', spy2);

    assert.equal(r1, 'result-a');
    assert.equal(r2, 'result-b');
    assert.equal(spy1.callCount(), 1);
    assert.equal(spy2.callCount(), 1);
    assert.equal(guard.getMetrics().misses, 2);
  });

  it('does not cross-contaminate between keys', async () => {
    const guard = createIdempotencyGuard();
    await guard.execute('alpha', async () => 'alpha-val');
    await guard.execute('beta', async () => 'beta-val');

    // Re-execute — should return cached
    const a = await guard.execute('alpha', async () => 'wrong');
    const b = await guard.execute('beta', async () => 'wrong');

    assert.equal(a, 'alpha-val');
    assert.equal(b, 'beta-val');
  });
});

// ---------------------------------------------------------------------------
// 5. TTL expiry allows re-execution
// ---------------------------------------------------------------------------

describe('IdempotencyGuard — TTL expiry', () => {
  it('allows re-execution after TTL expires', async () => {
    const guard = createIdempotencyGuard({ ttlMs: 50 });
    let callCount = 0;

    const result1 = await guard.execute('ttl-key', async () => {
      callCount++;
      return `call-${callCount}`;
    });
    assert.equal(result1, 'call-1');

    // Wait for TTL to expire
    await delay(80);

    const result2 = await guard.execute('ttl-key', async () => {
      callCount++;
      return `call-${callCount}`;
    });
    assert.equal(result2, 'call-2');
    assert.equal(callCount, 2, 'fn should be called again after TTL expiry');
  });

  it('has() returns false after TTL expiry', async () => {
    const guard = createIdempotencyGuard({ ttlMs: 30 });
    await guard.execute('ephemeral', async () => 'temp');
    assert.equal(guard.has('ephemeral'), true);

    await delay(60);
    assert.equal(guard.has('ephemeral'), false);
  });

  it('metrics size decreases after TTL expiry', async () => {
    const guard = createIdempotencyGuard({ ttlMs: 30 });
    await guard.execute('a', async () => 1);
    await guard.execute('b', async () => 2);
    assert.equal(guard.getMetrics().size, 2);

    await delay(60);
    assert.equal(guard.getMetrics().size, 0);
  });
});

// ---------------------------------------------------------------------------
// 6. invalidate() removes key and allows re-execution
// ---------------------------------------------------------------------------

describe('IdempotencyGuard — invalidate', () => {
  it('removes a key so fn runs again', async () => {
    const guard = createIdempotencyGuard();
    let callCount = 0;

    await guard.execute('inv-key', async () => {
      callCount++;
      return 'first';
    });
    assert.equal(callCount, 1);

    guard.invalidate('inv-key');
    assert.equal(guard.has('inv-key'), false);

    const result = await guard.execute('inv-key', async () => {
      callCount++;
      return 'second';
    });
    assert.equal(result, 'second');
    assert.equal(callCount, 2);
  });

  it('returns true when key existed', async () => {
    const guard = createIdempotencyGuard();
    await guard.execute('exists', async () => 'val');

    assert.equal(guard.invalidate('exists'), true);
  });

  it('returns false when key did not exist', () => {
    const guard = createIdempotencyGuard();
    assert.equal(guard.invalidate('nonexistent'), false);
  });

  it('allows re-execution of a failed key after invalidation', async () => {
    const guard = createIdempotencyGuard();
    let callCount = 0;

    // First call fails
    await assert.rejects(
      () => guard.execute('fail-then-fix', async () => {
        callCount++;
        throw new Error('db down');
      }),
      { message: 'db down' },
    );
    assert.equal(callCount, 1);

    // Without invalidation, the error is cached
    await assert.rejects(
      () => guard.execute('fail-then-fix', async () => {
        callCount++;
        return 'should not run';
      }),
      { message: 'db down' },
    );
    assert.equal(callCount, 1, 'fn should not re-run for cached failure');

    // After invalidation, fn runs again
    guard.invalidate('fail-then-fix');
    const result = await guard.execute('fail-then-fix', async () => {
      callCount++;
      return 'fixed';
    });
    assert.equal(result, 'fixed');
    assert.equal(callCount, 2);
  });
});

// ---------------------------------------------------------------------------
// 7. Failed execution is cached
// ---------------------------------------------------------------------------

describe('IdempotencyGuard — failed execution caching', () => {
  it('caches the error and re-throws on subsequent calls', async () => {
    const guard = createIdempotencyGuard();
    let callCount = 0;

    const failFn = async () => {
      callCount++;
      throw new Error('payment gateway timeout');
    };

    await assert.rejects(() => guard.execute('fail-key', failFn), {
      message: 'payment gateway timeout',
    });

    // Second call should throw same error without calling fn
    await assert.rejects(() => guard.execute('fail-key', failFn), {
      message: 'payment gateway timeout',
    });

    assert.equal(callCount, 1, 'fn should only be called once');
  });

  it('has() returns true for failed keys', async () => {
    const guard = createIdempotencyGuard();
    await assert.rejects(
      () => guard.execute('failed', async () => { throw new Error('oops'); }),
    );
    assert.equal(guard.has('failed'), true);
  });

  it('counts retries of failed keys as hits', async () => {
    const guard = createIdempotencyGuard();
    await assert.rejects(
      () => guard.execute('f', async () => { throw new Error('err'); }),
    );
    await assert.rejects(
      () => guard.execute('f', async () => { throw new Error('err'); }),
    );

    const metrics = guard.getMetrics();
    assert.equal(metrics.misses, 1);
    assert.equal(metrics.hits, 1);
  });
});

// ---------------------------------------------------------------------------
// 8. getMetrics() tracks hits/misses/size
// ---------------------------------------------------------------------------

describe('IdempotencyGuard — getMetrics', () => {
  it('starts with zero metrics', () => {
    const guard = createIdempotencyGuard();
    const m = guard.getMetrics();
    assert.equal(m.hits, 0);
    assert.equal(m.misses, 0);
    assert.equal(m.size, 0);
    assert.equal(m.evictions, 0);
  });

  it('tracks misses for new keys', async () => {
    const guard = createIdempotencyGuard();
    await guard.execute('a', async () => 1);
    await guard.execute('b', async () => 2);
    await guard.execute('c', async () => 3);

    const m = guard.getMetrics();
    assert.equal(m.misses, 3);
    assert.equal(m.size, 3);
  });

  it('tracks hits for repeated keys', async () => {
    const guard = createIdempotencyGuard();
    await guard.execute('x', async () => 'val');
    await guard.execute('x', async () => 'val');
    await guard.execute('x', async () => 'val');

    const m = guard.getMetrics();
    assert.equal(m.hits, 2);
    assert.equal(m.misses, 1);
  });

  it('clear() resets all metrics', async () => {
    const guard = createIdempotencyGuard();
    await guard.execute('a', async () => 1);
    await guard.execute('a', async () => 1);

    guard.clear();
    const m = guard.getMetrics();
    assert.equal(m.hits, 0);
    assert.equal(m.misses, 0);
    assert.equal(m.size, 0);
    assert.equal(m.evictions, 0);
  });
});

// ---------------------------------------------------------------------------
// 9. maxSize eviction
// ---------------------------------------------------------------------------

describe('IdempotencyGuard — maxSize eviction', () => {
  it('evicts oldest entries when maxSize is exceeded', async () => {
    const guard = createIdempotencyGuard({ maxSize: 3 });

    await guard.execute('k1', async () => 'v1');
    await guard.execute('k2', async () => 'v2');
    await guard.execute('k3', async () => 'v3');
    assert.equal(guard.getMetrics().size, 3);

    // Adding a 4th should evict k1
    await guard.execute('k4', async () => 'v4');
    assert.equal(guard.getMetrics().size, 3);
    assert.equal(guard.has('k1'), false, 'k1 should be evicted');
    assert.equal(guard.has('k2'), true);
    assert.equal(guard.has('k3'), true);
    assert.equal(guard.has('k4'), true);
  });

  it('tracks eviction count in metrics', async () => {
    const guard = createIdempotencyGuard({ maxSize: 2 });

    await guard.execute('a', async () => 1);
    await guard.execute('b', async () => 2);
    await guard.execute('c', async () => 3);
    await guard.execute('d', async () => 4);

    const m = guard.getMetrics();
    assert.equal(m.evictions, 2);
    assert.equal(m.size, 2);
  });

  it('evicted key can be re-executed', async () => {
    const guard = createIdempotencyGuard({ maxSize: 2 });
    let callCount = 0;

    await guard.execute('first', async () => {
      callCount++;
      return 'original';
    });
    await guard.execute('second', async () => 'x');
    await guard.execute('third', async () => 'y'); // evicts 'first'

    const result = await guard.execute('first', async () => {
      callCount++;
      return 'new';
    });
    assert.equal(result, 'new');
    assert.equal(callCount, 2, 'fn should run again after eviction');
  });
});

// ---------------------------------------------------------------------------
// 10. Input validation
// ---------------------------------------------------------------------------

describe('IdempotencyGuard — input validation', () => {
  it('rejects empty string key', async () => {
    const guard = createIdempotencyGuard();
    await assert.rejects(
      () => guard.execute('', async () => 'val'),
      { message: 'Idempotency key must be a non-empty string' },
    );
  });

  it('rejects non-string key', async () => {
    const guard = createIdempotencyGuard();
    await assert.rejects(
      () => guard.execute(123, async () => 'val'),
      { message: 'Idempotency key must be a non-empty string' },
    );
  });

  it('rejects non-function fn', async () => {
    const guard = createIdempotencyGuard();
    await assert.rejects(
      () => guard.execute('key', 'not a function'),
      { message: 'fn must be a function' },
    );
  });
});
