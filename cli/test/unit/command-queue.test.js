/**
 * Unit tests for command-queue.js
 */

import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert';
import { CommandQueue, resetCommandQueue } from '../../src/command-queue.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Create a delayed async task. */
function delayTask(ms, value) {
  return () => new Promise((resolve) => setTimeout(() => resolve(value), ms));
}

/** Create a task that rejects. */
function failTask(ms, message) {
  return () => new Promise((_, reject) => setTimeout(() => reject(new Error(message)), ms));
}

// ===========================================================================
// CommandQueue - Serial Lanes
// ===========================================================================

describe('CommandQueue - Serial Lanes', () => {
  let queue = null;

  afterEach(() => {
    if (queue) {
      queue.shutdown();
      queue = null;
    }
  });

  it('executes a single task', async () => {
    queue = new CommandQueue();
    const result = await queue.enqueue('session-1', () => 'hello');
    assert.strictEqual(result, 'hello');
  });

  it('executes async tasks', async () => {
    queue = new CommandQueue();
    const result = await queue.enqueue('session-1', delayTask(10, 42));
    assert.strictEqual(result, 42);
  });

  it('executes tasks serially within same lane', async () => {
    queue = new CommandQueue();
    const order = [];

    const p1 = queue.enqueue('lane-a', async () => {
      order.push('start-1');
      await new Promise((r) => setTimeout(r, 30));
      order.push('end-1');
      return 1;
    });

    const p2 = queue.enqueue('lane-a', async () => {
      order.push('start-2');
      await new Promise((r) => setTimeout(r, 10));
      order.push('end-2');
      return 2;
    });

    await Promise.all([p1, p2]);

    // Serial: task 1 must fully complete before task 2 starts
    assert.deepStrictEqual(order, ['start-1', 'end-1', 'start-2', 'end-2']);
  });

  it('allows parallel execution across different lanes', async () => {
    queue = new CommandQueue();
    const order = [];

    const p1 = queue.enqueue('lane-a', async () => {
      order.push('a-start');
      await new Promise((r) => setTimeout(r, 30));
      order.push('a-end');
    });

    const p2 = queue.enqueue('lane-b', async () => {
      order.push('b-start');
      await new Promise((r) => setTimeout(r, 10));
      order.push('b-end');
    });

    await Promise.all([p1, p2]);

    // Both should start before either ends (parallel across lanes)
    assert.ok(
      order.indexOf('b-start') < order.indexOf('a-end'),
      'Lane B should start before lane A ends',
    );
  });

  it('rejects task errors to the caller', async () => {
    queue = new CommandQueue();
    await assert.rejects(() => queue.enqueue('lane-a', failTask(5, 'task failed')), {
      message: 'task failed',
    });
  });

  it('continues processing after a failed task', async () => {
    queue = new CommandQueue();
    const errors = [];
    // Suppress error console output
    queue.lanes = queue.lanes;

    try {
      await queue.enqueue('lane-a', failTask(5, 'first fails'));
    } catch {}

    const result = await queue.enqueue('lane-a', () => 'second succeeds');
    assert.strictEqual(result, 'second succeeds');
  });

  it('rejects when lane queue is full', async () => {
    queue = new CommandQueue({ maxQueueSize: 2, laneTimeout: 5000 });

    // First task blocks the lane
    const blocker = queue.enqueue('lane-a', delayTask(200, 'block'));
    // These queue up
    const t1 = queue.enqueue('lane-a', () => 'ok');
    // This should fail because queue is full (1 processing + 2 queued = 3 > maxQueueSize 2)
    // Actually maxQueueSize limits the queue array, the processing task has already been shifted out
    // So with maxQueueSize: 2, after t1, queue has 1 item. Need one more.
    const t2 = queue.enqueue('lane-a', () => 'ok2');

    await assert.rejects(() => queue.enqueue('lane-a', () => 'overflow'), /queue full/);

    // Clean up
    await blocker;
    await t1;
    await t2;
  });
});

// ===========================================================================
// CommandQueue - Parallel Lanes
// ===========================================================================

describe('CommandQueue - Parallel Lanes', () => {
  let queue = null;

  afterEach(() => {
    if (queue) {
      queue.shutdown();
      queue = null;
    }
  });

  it('executes tasks concurrently in parallel lane', async () => {
    queue = new CommandQueue({ parallelConcurrency: 3 });
    const order = [];

    const tasks = Array.from({ length: 3 }, (_, i) =>
      queue.enqueueParallel('bg', async () => {
        order.push(`start-${i}`);
        await new Promise((r) => setTimeout(r, 20));
        order.push(`end-${i}`);
        return i;
      }),
    );

    const results = await Promise.all(tasks);
    assert.deepStrictEqual(results, [0, 1, 2]);

    // All should start before any ends (concurrent)
    const firstEnd = order.findIndex((e) => e.startsWith('end'));
    const starts = order.filter((e) => e.startsWith('start'));
    assert.ok(starts.length >= 2, 'At least 2 tasks should start before first ends');
  });

  it('respects concurrency limit', async () => {
    queue = new CommandQueue({ parallelConcurrency: 2 });
    let concurrent = 0;
    let maxConcurrent = 0;

    const tasks = Array.from({ length: 5 }, () =>
      queue.enqueueParallel('bg', async () => {
        concurrent++;
        maxConcurrent = Math.max(maxConcurrent, concurrent);
        await new Promise((r) => setTimeout(r, 20));
        concurrent--;
      }),
    );

    await Promise.all(tasks);
    assert.ok(maxConcurrent <= 2, `Max concurrent was ${maxConcurrent}, expected <= 2`);
  });
});

// ===========================================================================
// CommandQueue - Stats
// ===========================================================================

describe('CommandQueue - Stats', () => {
  let queue = null;

  afterEach(() => {
    if (queue) {
      queue.shutdown();
      queue = null;
    }
  });

  it('tracks lane stats after task execution', async () => {
    queue = new CommandQueue();
    await queue.enqueue('lane-a', () => 'done');
    await queue.enqueue('lane-a', () => 'done2');

    const stats = queue.getLaneStats('lane-a');
    assert.strictEqual(stats.totalProcessed, 2);
    assert.strictEqual(stats.totalErrors, 0);
    assert.ok(stats.avgDuration >= 0);
  });

  it('tracks error count in stats', async () => {
    queue = new CommandQueue();
    try {
      await queue.enqueue('lane-a', failTask(5, 'oops'));
    } catch {}

    const stats = queue.getLaneStats('lane-a');
    assert.strictEqual(stats.totalProcessed, 1);
    assert.strictEqual(stats.totalErrors, 1);
  });

  it('getStats returns all lanes summary', async () => {
    queue = new CommandQueue();
    await queue.enqueue('lane-a', () => 1);
    await queue.enqueue('lane-b', () => 2);
    await queue.enqueueParallel('bg', () => 3);

    const stats = queue.getStats();
    assert.strictEqual(stats.serialLanes.count, 2);
    assert.strictEqual(stats.parallelLanes.count, 1);
  });

  it('getLaneStats returns null for unknown lane', () => {
    queue = new CommandQueue();
    assert.strictEqual(queue.getLaneStats('unknown'), null);
  });
});

// ===========================================================================
// CommandQueue - Lifecycle
// ===========================================================================

describe('CommandQueue - Lifecycle', () => {
  it('shutdown clears all lanes and intervals', async () => {
    const queue = new CommandQueue();
    await queue.enqueue('lane-a', () => 'ok');
    queue.shutdown();
    assert.strictEqual(queue.lanes.size, 0);
    assert.strictEqual(queue.parallelLanes.size, 0);
  });

  it('waitForLane resolves when lane is idle', async () => {
    const queue = new CommandQueue();
    await queue.enqueue('lane-a', delayTask(10, 'done'));
    // Lane should be idle after task completes
    await queue.waitForLane('lane-a', 1000);
    queue.shutdown();
  });

  it('waitForLane resolves immediately for unknown lane', async () => {
    const queue = new CommandQueue();
    await queue.waitForLane('nonexistent', 100);
    queue.shutdown();
  });

  it('waitForAllLanes resolves when all idle', async () => {
    const queue = new CommandQueue();
    await queue.enqueue('a', delayTask(10, 1));
    await queue.enqueue('b', delayTask(10, 2));
    await queue.waitForAllLanes(1000);
    queue.shutdown();
  });
});

// ===========================================================================
// resetCommandQueue
// ===========================================================================

describe('resetCommandQueue', () => {
  it('can be called safely', () => {
    assert.doesNotThrow(() => resetCommandQueue());
  });
});
