/**
 * Unit tests for command-queue.js
 */

import { afterEach, describe, it } from 'node:test';
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

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
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
      await sleep(30);
      order.push('end-1');
      return 1;
    });

    const p2 = queue.enqueue('lane-a', async () => {
      order.push('start-2');
      await sleep(10);
      order.push('end-2');
      return 2;
    });

    await Promise.all([p1, p2]);

    assert.deepStrictEqual(order, ['start-1', 'end-1', 'start-2', 'end-2']);
  });

  it('allows parallel execution across different lanes', async () => {
    queue = new CommandQueue();
    const order = [];

    const p1 = queue.enqueue('lane-a', async () => {
      order.push('a-start');
      await sleep(30);
      order.push('a-end');
    });

    const p2 = queue.enqueue('lane-b', async () => {
      order.push('b-start');
      await sleep(10);
      order.push('b-end');
    });

    await Promise.all([p1, p2]);

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

    try {
      await queue.enqueue('lane-a', failTask(5, 'first fails'));
    } catch {
      // ignore expected failure
    }

    const result = await queue.enqueue('lane-a', () => 'second succeeds');
    assert.strictEqual(result, 'second succeeds');
  });

  it('rejects when lane queue is full', async () => {
    queue = new CommandQueue({ maxQueueSize: 2, laneTimeout: 5000 });

    const blocker = queue.enqueue('lane-a', delayTask(200, 'block'));
    const t1 = queue.enqueue('lane-a', () => 'ok');
    const t2 = queue.enqueue('lane-a', () => 'ok2');

    await assert.rejects(() => queue.enqueue('lane-a', () => 'overflow'), /queue full/);

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
        await sleep(20);
        order.push(`end-${i}`);
        return i;
      }),
    );

    const results = await Promise.all(tasks);
    assert.deepStrictEqual(results, [0, 1, 2]);

    const firstEnd = order.findIndex((event) => event.startsWith('end'));
    const starts = order.slice(0, firstEnd).filter((event) => event.startsWith('start'));
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
        await sleep(20);
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
    } catch {
      // ignore expected failure
    }

    const stats = queue.getLaneStats('lane-a');
    assert.strictEqual(stats.totalProcessed, 1);
    assert.strictEqual(stats.totalErrors, 1);
  });

  it('reports waiting and active metrics for serial lanes', async () => {
    queue = new CommandQueue();
    let release;
    let markStarted;
    const started = new Promise((resolve) => {
      markStarted = resolve;
    });

    const blocker = queue.enqueue('lane-a', async () => {
      markStarted();
      return new Promise((resolve) => {
        release = resolve;
      });
    });
    await started;

    const queued = queue.enqueue('lane-a', async () => 'next');
    await sleep(20);

    const stats = queue.getLaneStats('lane-a');
    assert.strictEqual(stats.type, 'serial');
    assert.strictEqual(stats.waitingTasks, 1);
    assert.strictEqual(stats.activeTasks, 1);
    assert.strictEqual(stats.currentQueueLength, 1);
    assert.strictEqual(stats.busy, true);
    assert.ok(stats.oldestPendingMs >= 0);
    assert.ok(stats.activeTaskAgeMs >= 0);

    release('done');
    await Promise.all([blocker, queued]);
  });

  it('reports waiting and active metrics for parallel lanes', async () => {
    queue = new CommandQueue({ parallelConcurrency: 1 });
    let release;
    let markStarted;
    const started = new Promise((resolve) => {
      markStarted = resolve;
    });

    const blocker = queue.enqueueParallel('bg', async () => {
      markStarted();
      return new Promise((resolve) => {
        release = resolve;
      });
    });
    await started;

    const queued = queue.enqueueParallel('bg', async () => 'next');
    await sleep(20);

    const stats = queue.getLaneStats('bg');
    assert.strictEqual(stats.type, 'parallel');
    assert.strictEqual(stats.waitingTasks, 1);
    assert.strictEqual(stats.activeTasks, 1);
    assert.strictEqual(stats.currentQueueLength, 1);
    assert.strictEqual(stats.busy, true);
    assert.ok(stats.oldestPendingMs >= 0);
    assert.ok(stats.activeTaskAgeMs >= 0);
    assert.strictEqual(stats.maxConcurrency, 1);

    release('done');
    await Promise.all([blocker, queued]);
  });

  it('getStats returns all lanes summary and active totals', async () => {
    queue = new CommandQueue({ parallelConcurrency: 1 });
    let release;
    let markStarted;
    const started = new Promise((resolve) => {
      markStarted = resolve;
    });

    const blocker = queue.enqueueParallel('bg', async () => {
      markStarted();
      return new Promise((resolve) => {
        release = resolve;
      });
    });
    await started;

    await queue.enqueue('lane-a', () => 1);
    const queued = queue.enqueueParallel('bg', () => 3);
    await sleep(20);

    const stats = queue.getStats();
    assert.strictEqual(stats.serialLanes.count, 1);
    assert.strictEqual(stats.parallelLanes.count, 1);
    assert.strictEqual(stats.totalPending, 1);
    assert.strictEqual(stats.totalActive, 1);
    assert.strictEqual(stats.busyLanes, 1);

    release('done');
    await Promise.all([blocker, queued]);
  });

  it('getLaneStats returns null for unknown lane', () => {
    queue = new CommandQueue();
    assert.strictEqual(queue.getLaneStats('unknown'), null);
  });
});

// ===========================================================================
// CommandQueue - Warnings
// ===========================================================================

describe('CommandQueue - Warnings', () => {
  let queue = null;

  afterEach(() => {
    if (queue) {
      queue.shutdown();
      queue = null;
    }
  });

  it('emits a throttled pending-wait warning', async () => {
    const warnings = [];
    queue = new CommandQueue({
      waitWarningMs: 20,
      runningWarningMs: 1000,
      warningThrottleMs: 1000,
      monitorIntervalMs: 5,
      onWarning: (warning) => warnings.push(warning),
    });

    let release;
    let markStarted;
    const started = new Promise((resolve) => {
      markStarted = resolve;
    });

    const blocker = queue.enqueue('lane-a', async () => {
      markStarted();
      return new Promise((resolve) => {
        release = resolve;
      });
    });
    await started;

    const queued = queue.enqueue('lane-a', async () => 'next');
    await sleep(60);

    const pendingWarnings = warnings.filter((warning) => warning.issue === 'pending_wait');
    assert.strictEqual(pendingWarnings.length, 1);
    assert.strictEqual(pendingWarnings[0].laneId, 'lane-a');
    assert.strictEqual(pendingWarnings[0].laneType, 'serial');
    assert.strictEqual(pendingWarnings[0].waitingTasks, 1);
    assert.strictEqual(pendingWarnings[0].activeTasks, 1);
    assert.ok(pendingWarnings[0].ageMs >= pendingWarnings[0].thresholdMs);

    release('done');
    await Promise.all([blocker, queued]);
  });

  it('emits a running-task warning for long parallel work', async () => {
    const warnings = [];
    queue = new CommandQueue({
      parallelConcurrency: 1,
      waitWarningMs: 1000,
      runningWarningMs: 20,
      warningThrottleMs: 1000,
      monitorIntervalMs: 5,
      onWarning: (warning) => warnings.push(warning),
    });

    let release;
    const blocker = queue.enqueueParallel('bg', async () =>
      new Promise((resolve) => {
        release = resolve;
      }),
    );

    await sleep(60);

    const runningWarnings = warnings.filter((warning) => warning.issue === 'running_task');
    assert.strictEqual(runningWarnings.length, 1);
    assert.strictEqual(runningWarnings[0].laneId, 'bg');
    assert.strictEqual(runningWarnings[0].laneType, 'parallel');
    assert.ok(runningWarnings[0].ageMs >= runningWarnings[0].thresholdMs);

    release('done');
    await blocker;
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
    await queue.waitForLane('lane-a', 1000);
    queue.shutdown();
  });

  it('waitForLane resolves immediately for unknown lane', async () => {
    const queue = new CommandQueue();
    await queue.waitForLane('nonexistent', 100);
    queue.shutdown();
  });

  it('waitForLane also supports parallel lanes', async () => {
    const queue = new CommandQueue({ parallelConcurrency: 1 });
    await queue.enqueueParallel('bg', delayTask(10, 'done'));
    await queue.waitForLane('bg', 1000);
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
