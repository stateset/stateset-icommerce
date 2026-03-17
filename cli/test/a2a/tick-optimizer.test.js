/**
 * Unit tests for a2a/tick-optimizer.js — Tick Loop Optimizer
 *
 * Covers: wrapTick (duration tracking, overlap prevention), adaptive polling
 * (idle backoff, activity reset), getMetrics (p50/p95/p99), ProcessedIdTracker
 * (add/has, LRU eviction), reset(), and tick-duration warnings.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import {
  createTickOptimizer,
  createProcessedIdTracker,
} from '../../src/a2a/tick-optimizer.js';

// ─── Helpers ─────────────────────────────────────────────────────────────────

/** Create a tick function that resolves after `ms` with a given item count */
function delayedTick(ms, itemsProcessed = 0) {
  return () =>
    new Promise((resolve) => {
      setTimeout(() => resolve(itemsProcessed), ms);
    });
}

/** Create a tick function that fails after `ms` */
function failingTick(ms, message = 'boom') {
  return () =>
    new Promise((_resolve, reject) => {
      setTimeout(() => reject(new Error(message)), ms);
    });
}

// ─── wrapTick ────────────────────────────────────────────────────────────────

describe('wrapTick', () => {
  it('tracks tick duration correctly', async () => {
    const optimizer = createTickOptimizer({ baseIntervalMs: 5000 });
    const tick = optimizer.wrapTick(delayedTick(50, 1));

    const result = await tick();
    assert.ok(result.durationMs >= 40, `Expected >=40ms, got ${result.durationMs}`);
    assert.ok(result.durationMs < 500, `Expected <500ms, got ${result.durationMs}`);
    assert.equal(result.itemsProcessed, 1);
    assert.equal(result.skipped, undefined);
  });

  it('prevents overlapping ticks', async () => {
    const optimizer = createTickOptimizer({ baseIntervalMs: 5000 });
    // Slow tick that takes 200ms
    const tick = optimizer.wrapTick(delayedTick(200, 1));

    // Fire two ticks simultaneously
    const [r1, r2] = await Promise.all([tick(), tick()]);

    // One should complete, one should be skipped
    const completed = [r1, r2].filter((r) => r.durationMs !== undefined);
    const skipped = [r1, r2].filter((r) => r.skipped === true);

    assert.equal(completed.length, 1, 'Exactly one tick should complete');
    assert.equal(skipped.length, 1, 'Exactly one tick should be skipped');

    const metrics = optimizer.getMetrics();
    assert.equal(metrics.overlappingTicksSkipped, 1);
  });

  it('handles tick function returning an object with itemsProcessed', async () => {
    const optimizer = createTickOptimizer();
    const tick = optimizer.wrapTick(async () => ({ itemsProcessed: 5 }));

    const result = await tick();
    assert.equal(result.itemsProcessed, 5);
  });

  it('treats tick errors gracefully and increments error count', async () => {
    const optimizer = createTickOptimizer();
    const tick = optimizer.wrapTick(failingTick(10, 'test failure'));

    const result = await tick();
    assert.equal(result.itemsProcessed, 0);
    assert.ok(result.error.includes('test failure'));

    const metrics = optimizer.getMetrics();
    assert.equal(metrics.errors, 1);
    assert.equal(metrics.totalTicks, 1);
  });
});

// ─── Adaptive polling ────────────────────────────────────────────────────────

describe('adaptive polling', () => {
  it('doubles interval after 3 consecutive idle ticks', async () => {
    const optimizer = createTickOptimizer({ baseIntervalMs: 1000, maxIntervalMs: 8000 });
    const tick = optimizer.wrapTick(async () => 0); // always idle

    await tick(); // idle 1
    assert.equal(optimizer.getAdaptiveInterval(), 1000);
    await tick(); // idle 2
    assert.equal(optimizer.getAdaptiveInterval(), 1000);
    await tick(); // idle 3 → backoff
    assert.equal(optimizer.getAdaptiveInterval(), 2000);
    await tick(); // idle 4 → double again
    assert.equal(optimizer.getAdaptiveInterval(), 4000);
  });

  it('caps interval at maxIntervalMs', async () => {
    const optimizer = createTickOptimizer({ baseIntervalMs: 1000, maxIntervalMs: 4000 });
    const tick = optimizer.wrapTick(async () => 0);

    for (let i = 0; i < 10; i++) await tick();

    assert.ok(optimizer.getAdaptiveInterval() <= 4000);
  });

  it('resets interval to base on activity', async () => {
    const optimizer = createTickOptimizer({ baseIntervalMs: 1000, maxIntervalMs: 16000 });

    // Idle phase
    const idleTick = optimizer.wrapTick(async () => 0);
    for (let i = 0; i < 5; i++) await idleTick();
    assert.ok(optimizer.getAdaptiveInterval() > 1000);

    // Activity phase — need a new wrapped tick that returns items
    const activeTick = optimizer.wrapTick(async () => 3);
    await activeTick();

    assert.equal(optimizer.getAdaptiveInterval(), 1000);
    assert.equal(optimizer.getMetrics().consecutiveIdleTicks, 0);
  });
});

// ─── getMetrics ──────────────────────────────────────────────────────────────

describe('getMetrics', () => {
  it('computes p50, p95, p99 duration from samples', async () => {
    const optimizer = createTickOptimizer({ baseIntervalMs: 10000 });
    // We'll run 20 fast ticks to accumulate duration samples
    const tick = optimizer.wrapTick(async () => 1);
    for (let i = 0; i < 20; i++) await tick();

    const metrics = optimizer.getMetrics();

    assert.equal(metrics.totalTicks, 20);
    assert.ok(metrics.avgDurationMs >= 0);
    assert.ok(metrics.p50DurationMs >= 0);
    assert.ok(metrics.p95DurationMs >= metrics.p50DurationMs);
    assert.ok(metrics.p99DurationMs >= metrics.p95DurationMs);
    assert.ok(metrics.maxDurationMs >= metrics.minDurationMs);
    assert.ok(metrics.ticksPerMinute > 0);
  });

  it('returns zeros when no ticks have run', () => {
    const optimizer = createTickOptimizer();
    const metrics = optimizer.getMetrics();

    assert.equal(metrics.totalTicks, 0);
    assert.equal(metrics.avgDurationMs, 0);
    assert.equal(metrics.p50DurationMs, 0);
    assert.equal(metrics.p95DurationMs, 0);
    assert.equal(metrics.p99DurationMs, 0);
    assert.equal(metrics.maxDurationMs, 0);
    assert.equal(metrics.minDurationMs, 0);
    assert.equal(metrics.overlappingTicksSkipped, 0);
    assert.equal(metrics.errors, 0);
  });
});

// ─── Overlap detection ───────────────────────────────────────────────────────

describe('overlap skip counter', () => {
  it('increments skip counter for each overlapping attempt', async () => {
    const optimizer = createTickOptimizer();
    const tick = optimizer.wrapTick(delayedTick(200, 1));

    // Launch one real tick, then 3 overlapping attempts
    const results = await Promise.all([tick(), tick(), tick(), tick()]);

    const skipped = results.filter((r) => r.skipped);
    assert.equal(skipped.length, 3);
    assert.equal(optimizer.getMetrics().overlappingTicksSkipped, 3);
  });
});

// ─── Warning when tick > 80% of interval ─────────────────────────────────────

describe('tick duration warning', () => {
  it('produces a warning when tick takes > 80% of interval', async () => {
    // Base interval 100ms, tick takes ~90ms → >80%
    const optimizer = createTickOptimizer({ baseIntervalMs: 100 });
    const tick = optimizer.wrapTick(delayedTick(90, 1));

    await tick();

    const metrics = optimizer.getMetrics();
    assert.ok(metrics.warnings.length >= 1, 'Expected at least one warning');
    assert.ok(metrics.warnings[0].message.includes('80%'));
  });

  it('does NOT produce a warning when tick is fast relative to interval', async () => {
    const optimizer = createTickOptimizer({ baseIntervalMs: 10000 });
    const tick = optimizer.wrapTick(async () => 1); // near-instant

    await tick();

    const metrics = optimizer.getMetrics();
    assert.equal(metrics.warnings.length, 0);
  });
});

// ─── reset() ─────────────────────────────────────────────────────────────────

describe('reset', () => {
  it('clears all metrics and interval state', async () => {
    const optimizer = createTickOptimizer({ baseIntervalMs: 1000 });
    const tick = optimizer.wrapTick(async () => 0);

    // Accumulate some state
    for (let i = 0; i < 5; i++) await tick();
    assert.ok(optimizer.getMetrics().totalTicks > 0);

    optimizer.reset();

    const metrics = optimizer.getMetrics();
    assert.equal(metrics.totalTicks, 0);
    assert.equal(metrics.avgDurationMs, 0);
    assert.equal(metrics.p50DurationMs, 0);
    assert.equal(metrics.overlappingTicksSkipped, 0);
    assert.equal(metrics.consecutiveIdleTicks, 0);
    assert.equal(metrics.errors, 0);
    assert.equal(metrics.warnings.length, 0);
    assert.equal(optimizer.getAdaptiveInterval(), 1000);
  });
});

// ─── ProcessedIdTracker ──────────────────────────────────────────────────────

describe('createProcessedIdTracker', () => {
  it('add/has works for basic operations', () => {
    const tracker = createProcessedIdTracker(100);

    tracker.add('id-1');
    tracker.add('id-2');
    tracker.add('id-3');

    assert.equal(tracker.has('id-1'), true);
    assert.equal(tracker.has('id-2'), true);
    assert.equal(tracker.has('id-3'), true);
    assert.equal(tracker.has('id-999'), false);
    assert.equal(tracker.size, 3);
  });

  it('evicts oldest IDs when maxSize is exceeded (LRU)', () => {
    const tracker = createProcessedIdTracker(3);

    tracker.add('a');
    tracker.add('b');
    tracker.add('c');
    assert.equal(tracker.size, 3);

    // Adding a 4th should evict 'a' (oldest)
    tracker.add('d');
    assert.equal(tracker.size, 3);
    assert.equal(tracker.has('a'), false, '"a" should have been evicted');
    assert.equal(tracker.has('b'), true);
    assert.equal(tracker.has('c'), true);
    assert.equal(tracker.has('d'), true);
  });

  it('refreshes an existing key so it is not evicted next', () => {
    const tracker = createProcessedIdTracker(3);

    tracker.add('a');
    tracker.add('b');
    tracker.add('c');

    // Refresh 'a' — moves it to the end of the LRU queue
    tracker.add('a');
    assert.equal(tracker.size, 3);

    // Now add 'd' — should evict 'b' (the oldest non-refreshed entry)
    tracker.add('d');
    assert.equal(tracker.has('b'), false, '"b" should have been evicted');
    assert.equal(tracker.has('a'), true, '"a" should survive (refreshed)');
    assert.equal(tracker.has('c'), true);
    assert.equal(tracker.has('d'), true);
  });

  it('clear() removes all entries', () => {
    const tracker = createProcessedIdTracker(100);
    tracker.add('x');
    tracker.add('y');
    assert.equal(tracker.size, 2);

    tracker.clear();
    assert.equal(tracker.size, 0);
    assert.equal(tracker.has('x'), false);
    assert.equal(tracker.has('y'), false);
  });

  it('handles maxSize of 1', () => {
    const tracker = createProcessedIdTracker(1);

    tracker.add('first');
    assert.equal(tracker.has('first'), true);
    assert.equal(tracker.size, 1);

    tracker.add('second');
    assert.equal(tracker.has('first'), false);
    assert.equal(tracker.has('second'), true);
    assert.equal(tracker.size, 1);
  });

  it('defaults to 100,000 maxSize', () => {
    const tracker = createProcessedIdTracker();
    // Just verify it works — we don't actually add 100k entries
    tracker.add('test');
    assert.equal(tracker.has('test'), true);
  });
});
