/**
 * Lane-Based Command Queue for StateSet CLI
 *
 * Implements a serialization-by-default architecture inspired by Clawdbot.
 * Each session/channel gets its own "lane" where operations execute serially,
 * preventing race conditions in multi-session scenarios.
 *
 * Key insight: "Default to Serial, go for Parallel explicitly"
 *
 * Usage:
 *   const queue = new CommandQueue();
 *
 *   // Serial execution (default) - same session
 *   await queue.enqueue('session-123', () => processOrder());
 *   await queue.enqueue('session-123', () => updateInventory());
 *
 *   // Parallel execution - different sessions run concurrently
 *   await Promise.all([
 *     queue.enqueue('session-a', () => processOrderA()),
 *     queue.enqueue('session-b', () => processOrderB()),
 *   ]);
 *
 *   // Explicit parallel lane for background tasks
 *   await queue.enqueueParallel('cron', () => cleanupOldSessions());
 */

// ============================================================================
// Lane
// ============================================================================

/**
 * A Lane is a serial execution queue for a specific session/channel.
 * Operations within a lane execute one at a time in order.
 */
class Lane {
  constructor(id, options = {}) {
    this.id = id;
    this.queue = [];
    this.processing = false;
    this.createdAt = Date.now();
    this.maxQueueSize = options.maxQueueSize || 100;
    this.timeout = options.timeout || 300000; // 5 minutes default
    this.onError =
      options.onError || ((err, _task) => console.error(`[Lane ${id}] Error:`, err.message));

    // Metrics
    this.stats = {
      createdAt: this.createdAt,
      totalProcessed: 0,
      totalErrors: 0,
      avgDuration: 0,
      maxDuration: 0,
      lastActivity: null,
      lastActivityMs: null,
      queueHighWaterMark: 0,
    };
  }

  /**
   * Add a task to the lane's queue.
   * @param {Function} task - Async function to execute
   * @param {object} [meta] - Metadata for tracking/debugging
   * @returns {Promise<any>} - Resolves with task result
   */
  enqueue(task, meta = {}) {
    if (this.queue.length >= this.maxQueueSize) {
      return Promise.reject(new Error(`Lane ${this.id} queue full (max: ${this.maxQueueSize})`));
    }

    return new Promise((resolve, reject) => {
      const entry = {
        task,
        meta,
        resolve,
        reject,
        enqueuedAt: Date.now(),
      };

      this.queue.push(entry);
      this.stats.queueHighWaterMark = Math.max(this.stats.queueHighWaterMark, this.queue.length);

      // Start processing if not already running
      if (!this.processing) {
        this._processQueue();
      }
    });
  }

  /**
   * Get current queue length.
   */
  get length() {
    return this.queue.length;
  }

  /**
   * Check if lane is idle (no pending tasks).
   */
  get idle() {
    return this.queue.length === 0 && !this.processing;
  }

  /**
   * Process the queue serially.
   * @private
   */
  async _processQueue() {
    if (this.processing) return;
    this.processing = true;

    while (this.queue.length > 0) {
      const entry = this.queue.shift();
      const startTime = Date.now();

      try {
        // Execute with timeout
        const result = await this._executeWithTimeout(entry.task, this.timeout);

        const duration = Date.now() - startTime;
        this._updateStats(duration, false);
        entry.resolve(result);
      } catch (error) {
        const duration = Date.now() - startTime;
        this._updateStats(duration, true);

        this.onError(error, entry);
        entry.reject(error);
      }
    }

    this.processing = false;
  }

  /**
   * Execute a task with timeout.
   * @private
   */
  async _executeWithTimeout(task, timeout) {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        reject(new Error(`Task timed out after ${timeout}ms`));
      }, timeout);

      Promise.resolve(task())
        .then((result) => {
          clearTimeout(timer);
          resolve(result);
        })
        .catch((err) => {
          clearTimeout(timer);
          reject(err);
        });
    });
  }

  /**
   * Update lane statistics.
   * @private
   */
  _updateStats(duration, isError) {
    const now = Date.now();
    this.stats.totalProcessed++;
    if (isError) this.stats.totalErrors++;

    // Rolling average
    const prevTotal = this.stats.totalProcessed - 1;
    this.stats.avgDuration =
      (this.stats.avgDuration * prevTotal + duration) / this.stats.totalProcessed;
    this.stats.maxDuration = Math.max(this.stats.maxDuration, duration);
    this.stats.lastActivityMs = now;
    this.stats.lastActivity = new Date(now).toISOString();
  }

  /**
   * Get lane statistics.
   */
  getStats() {
    return {
      ...this.stats,
      currentQueueLength: this.queue.length,
      isProcessing: this.processing,
    };
  }
}

// ============================================================================
// ParallelLane
// ============================================================================

/**
 * A ParallelLane allows concurrent execution with optional concurrency limit.
 * Use for background tasks, cron jobs, etc.
 */
class ParallelLane extends Lane {
  constructor(id, options = {}) {
    super(id, options);
    this.maxConcurrency = options.maxConcurrency || 5;
    this.activeTasks = 0;
    this.waitingQueue = [];
  }

  /**
   * Execute task with concurrency limit.
   */
  enqueue(task, meta = {}) {
    return new Promise((resolve, reject) => {
      const entry = { task, meta, resolve, reject, enqueuedAt: Date.now() };

      if (this.activeTasks < this.maxConcurrency) {
        this._executeTask(entry);
      } else {
        this.waitingQueue.push(entry);
        this.stats.queueHighWaterMark = Math.max(
          this.stats.queueHighWaterMark,
          this.waitingQueue.length + this.activeTasks,
        );
      }
    });
  }

  /**
   * Execute a single task.
   * @private
   */
  async _executeTask(entry) {
    this.activeTasks++;
    const startTime = Date.now();

    try {
      const result = await this._executeWithTimeout(entry.task, this.timeout);
      const duration = Date.now() - startTime;
      this._updateStats(duration, false);
      entry.resolve(result);
    } catch (error) {
      const duration = Date.now() - startTime;
      this._updateStats(duration, true);
      this.onError(error, entry);
      entry.reject(error);
    } finally {
      this.activeTasks--;
      this._tryNextTask();
    }
  }

  /**
   * Start next waiting task if under concurrency limit.
   * @private
   */
  _tryNextTask() {
    if (this.waitingQueue.length > 0 && this.activeTasks < this.maxConcurrency) {
      const next = this.waitingQueue.shift();
      this._executeTask(next);
    }
  }

  get length() {
    return this.waitingQueue.length + this.activeTasks;
  }

  get idle() {
    return this.activeTasks === 0 && this.waitingQueue.length === 0;
  }
}

// ============================================================================
// CommandQueue
// ============================================================================

/**
 * Main CommandQueue manager.
 * Creates and manages lanes for different sessions/channels.
 */
export class CommandQueue {
  /**
   * @param {object} [options]
   * @param {number} [options.maxLanes=1000] - Maximum number of lanes to keep
   * @param {number} [options.laneTimeout=300000] - Default task timeout per lane (ms)
   * @param {number} [options.maxQueueSize=100] - Max queue size per lane
   * @param {number} [options.idleCleanupMs=3600000] - Cleanup idle lanes after (ms)
   * @param {number} [options.parallelConcurrency=5] - Concurrency limit for parallel lanes
   */
  constructor(options = {}) {
    this.options = {
      maxLanes: options.maxLanes || 1000,
      laneTimeout: options.laneTimeout || 300000,
      maxQueueSize: options.maxQueueSize || 100,
      idleCleanupMs: options.idleCleanupMs || 3600000,
      parallelConcurrency: options.parallelConcurrency || 5,
    };

    /** @type {Map<string, Lane>} */
    this.lanes = new Map();

    /** @type {Map<string, ParallelLane>} */
    this.parallelLanes = new Map();

    // Start idle lane cleanup
    this._cleanupInterval = setInterval(() => this._cleanupIdleLanes(), 60000);
    this._cleanupInterval.unref?.();
  }

  /**
   * Enqueue a task in a serial lane.
   * Tasks in the same lane execute one at a time.
   *
   * @param {string} laneId - Lane identifier (typically session ID)
   * @param {Function} task - Async function to execute
   * @param {object} [meta] - Optional metadata
   * @returns {Promise<any>}
   */
  enqueue(laneId, task, meta = {}) {
    const lane = this._getOrCreateLane(laneId);
    return lane.enqueue(task, meta);
  }

  /**
   * Enqueue a task in a parallel lane.
   * Tasks execute concurrently up to the concurrency limit.
   *
   * @param {string} laneId - Lane identifier
   * @param {Function} task - Async function to execute
   * @param {object} [meta] - Optional metadata
   * @returns {Promise<any>}
   */
  enqueueParallel(laneId, task, meta = {}) {
    const lane = this._getOrCreateParallelLane(laneId);
    return lane.enqueue(task, meta);
  }

  /**
   * Wait for a specific lane to become idle.
   *
   * @param {string} laneId - Lane identifier
   * @param {number} [timeout=30000] - Max wait time (ms)
   * @returns {Promise<void>}
   */
  async waitForLane(laneId, timeout = 30000) {
    const lane = this.lanes.get(laneId);
    if (!lane || lane.idle) return;

    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      if (lane.idle) return;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }

    throw new Error(`Timeout waiting for lane ${laneId} to become idle`);
  }

  /**
   * Wait for all lanes to become idle.
   *
   * @param {number} [timeout=60000] - Max wait time (ms)
   * @returns {Promise<void>}
   */
  async waitForAllLanes(timeout = 60000) {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      let allIdle = true;
      for (const lane of this.lanes.values()) {
        if (!lane.idle) {
          allIdle = false;
          break;
        }
      }
      for (const lane of this.parallelLanes.values()) {
        if (!lane.idle) {
          allIdle = false;
          break;
        }
      }
      if (allIdle) return;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }

    throw new Error('Timeout waiting for all lanes to become idle');
  }

  /**
   * Get or create a serial lane.
   * @private
   */
  _getOrCreateLane(laneId) {
    let lane = this.lanes.get(laneId);
    if (!lane) {
      if (this.lanes.size >= this.options.maxLanes) {
        const evicted = this._evictOldestLane();
        if (!evicted && this.lanes.size >= this.options.maxLanes) {
          throw new Error(
            `Cannot create new lane '${laneId}': max lanes reached (${this.options.maxLanes})`,
          );
        }
      }
      lane = new Lane(laneId, {
        timeout: this.options.laneTimeout,
        maxQueueSize: this.options.maxQueueSize,
      });
      this.lanes.set(laneId, lane);
    }
    return lane;
  }

  /**
   * Get or create a parallel lane.
   * @private
   */
  _getOrCreateParallelLane(laneId) {
    let lane = this.parallelLanes.get(laneId);
    if (!lane) {
      if (this.parallelLanes.size >= this.options.maxLanes) {
        const evicted = this._evictOldestParallelLane();
        if (!evicted && this.parallelLanes.size >= this.options.maxLanes) {
          throw new Error(
            `Cannot create parallel lane '${laneId}': max parallel lanes reached (${this.options.maxLanes})`,
          );
        }
      }
      lane = new ParallelLane(laneId, {
        timeout: this.options.laneTimeout,
        maxQueueSize: this.options.maxQueueSize,
        maxConcurrency: this.options.parallelConcurrency,
      });
      this.parallelLanes.set(laneId, lane);
    }
    return lane;
  }

  /**
   * Evict the oldest idle lane when at capacity.
   * @private
   */
  _evictOldestLane() {
    const oldestId = this._findOldestLaneId(this.lanes);
    if (oldestId) {
      this.lanes.delete(oldestId);
      return oldestId;
    }
    return null;
  }

  _evictOldestParallelLane() {
    const oldestId = this._findOldestLaneId(this.parallelLanes);
    if (oldestId) {
      this.parallelLanes.delete(oldestId);
      return oldestId;
    }
    return null;
  }

  _findOldestLaneId(laneMap) {
    let oldestTime = Infinity;
    let oldestId = null;

    for (const [id, lane] of laneMap.entries()) {
      if (lane.idle) {
        const time = this._getLaneLastActivityMs(lane);
        if (time < oldestTime) {
          oldestTime = time;
          oldestId = id;
        }
      }
    }

    return oldestId;
  }

  /**
   * Clean up idle lanes older than idleCleanupMs.
   * @private
   */
  _cleanupIdleLanes() {
    const cutoff = Date.now() - this.options.idleCleanupMs;

    for (const [id, lane] of this.lanes.entries()) {
      const lastActivity = this._getLaneLastActivityMs(lane);
      if (lane.idle && lastActivity < cutoff) {
        this.lanes.delete(id);
      }
    }

    for (const [id, lane] of this.parallelLanes.entries()) {
      const lastActivity = this._getLaneLastActivityMs(lane);
      if (lane.idle && lastActivity < cutoff) {
        this.parallelLanes.delete(id);
      }
    }
  }

  _getLaneLastActivityMs(lane) {
    if (lane.stats.lastActivityMs !== null) {
      return lane.stats.lastActivityMs;
    }
    return lane.stats.createdAt;
  }

  /**
   * Remove a specific lane by id.
   *
   * @param {string} laneId
   * @param {{ force?: boolean }} [options]
   * @returns {{
   *   laneId: string,
   *   found: boolean,
   *   removed: boolean,
   *   busy: boolean,
   *   type: 'serial' | 'parallel' | null,
   * }}
   */
  removeLane(laneId, options = {}) {
    const force = options.force === true;
    const target = this.lanes.get(laneId);
    if (target) {
      const busy = !target.idle;
      if (busy && !force) {
        return {
          laneId,
          found: true,
          removed: false,
          busy,
          type: 'serial',
        };
      }
      this.lanes.delete(laneId);
      return {
        laneId,
        found: true,
        removed: true,
        busy,
        type: 'serial',
      };
    }

    const parallelTarget = this.parallelLanes.get(laneId);
    if (parallelTarget) {
      const busy = !parallelTarget.idle;
      if (busy && !force) {
        return {
          laneId,
          found: true,
          removed: false,
          busy,
          type: 'parallel',
        };
      }
      this.parallelLanes.delete(laneId);
      return {
        laneId,
        found: true,
        removed: true,
        busy,
        type: 'parallel',
      };
    }

    return {
      laneId,
      found: false,
      removed: false,
      busy: false,
      type: null,
    };
  }

  /**
   * Clear lanes. By default clears only idle lanes; set force=true to clear active lanes too.
   *
   * @param {{ force?: boolean }} [options]
   * @returns {{
   *   serial: { removed: number, skipped: number },
   *   parallel: { removed: number, skipped: number },
   *   totalRemoved: number,
   *   force: boolean,
   * }}
   */
  clearLanes(options = {}) {
    const force = options.force === true;
    let serialRemoved = 0;
    let serialSkipped = 0;
    let parallelRemoved = 0;
    let parallelSkipped = 0;

    for (const [id, lane] of this.lanes.entries()) {
      if (!lane.idle && !force) {
        serialSkipped++;
        continue;
      }
      this.lanes.delete(id);
      serialRemoved++;
    }

    for (const [id, lane] of this.parallelLanes.entries()) {
      if (!lane.idle && !force) {
        parallelSkipped++;
        continue;
      }
      this.parallelLanes.delete(id);
      parallelRemoved++;
    }

    return {
      serial: { removed: serialRemoved, skipped: serialSkipped },
      parallel: { removed: parallelRemoved, skipped: parallelSkipped },
      totalRemoved: serialRemoved + parallelRemoved,
      force,
    };
  }

  /**
   * Get statistics for all lanes.
   */
  getStats() {
    const serialLanes = [];
    for (const [id, lane] of this.lanes.entries()) {
      serialLanes.push({ id, type: 'serial', ...lane.getStats() });
    }

    const parallelLanesStats = [];
    for (const [id, lane] of this.parallelLanes.entries()) {
      parallelLanesStats.push({ id, type: 'parallel', ...lane.getStats() });
    }

    return {
      serialLanes: {
        count: this.lanes.size,
        lanes: serialLanes,
      },
      parallelLanes: {
        count: this.parallelLanes.size,
        lanes: parallelLanesStats,
      },
      totalPending:
        serialLanes.reduce((sum, l) => sum + l.currentQueueLength, 0) +
        parallelLanesStats.reduce((sum, l) => sum + l.currentQueueLength, 0),
    };
  }

  /**
   * Get statistics for a specific lane.
   * @param {string} laneId
   */
  getLaneStats(laneId) {
    const lane = this.lanes.get(laneId) || this.parallelLanes.get(laneId);
    if (!lane) return null;

    if (this.lanes.has(laneId)) {
      return { type: 'serial', ...lane.getStats() };
    }

    return { type: 'parallel', ...lane.getStats() };
  }

  /**
   * Shutdown the queue manager.
   */
  shutdown() {
    if (this._cleanupInterval) {
      clearInterval(this._cleanupInterval);
      this._cleanupInterval = null;
    }
    this.lanes.clear();
    this.parallelLanes.clear();
  }
}

// ============================================================================
// Singleton
// ============================================================================

let _instance = null;

/**
 * Get the global CommandQueue singleton.
 * @param {object} [options]
 * @returns {CommandQueue}
 */
export function getCommandQueue(options) {
  if (!_instance) {
    _instance = new CommandQueue(options);
  }
  return _instance;
}

/**
 * Reset the singleton (for testing).
 */
export function resetCommandQueue() {
  if (_instance) {
    _instance.shutdown();
    _instance = null;
  }
}

export default {
  CommandQueue,
  Lane,
  ParallelLane,
  getCommandQueue,
  resetCommandQueue,
};
