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
// Constants
// ============================================================================

const POLL_INTERVAL_MS = 100;
const CLEANUP_CHECK_INTERVAL_MS = 60_000;
const DEFAULT_MONITOR_INTERVAL_MS = 5_000;
const DEFAULT_WAIT_WARNING_MS = 30_000;
const DEFAULT_RUNNING_WARNING_MS = 120_000;
const DEFAULT_WARNING_THROTTLE_MS = 30_000;

function normalizePositiveNumber(value, fallback) {
  const numeric = Number(value);
  return Number.isFinite(numeric) && numeric > 0 ? numeric : fallback;
}

function normalizeThreshold(value, fallback) {
  const numeric = Number(value);
  return Number.isFinite(numeric) && numeric >= 0 ? numeric : fallback;
}

function createWarningState() {
  return {
    pending: { key: null, lastAt: 0 },
    running: { key: null, lastAt: 0 },
  };
}

function formatQueueWarning(payload) {
  if (payload.issue === 'pending_wait') {
    return (
      `[CommandQueue] Lane ${payload.laneId} has ${payload.waitingTasks} waiting task(s) ` +
      `for ${payload.ageMs}ms (threshold ${payload.thresholdMs}ms)`
    );
  }

  return (
    `[CommandQueue] Lane ${payload.laneId} has ${payload.activeTasks} active task(s) ` +
    `running for ${payload.ageMs}ms (threshold ${payload.thresholdMs}ms)`
  );
}

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
    this.maxQueueSize = normalizePositiveNumber(options.maxQueueSize, 100);
    this.timeout = normalizePositiveNumber(options.timeout ?? options.laneTimeout, 300000);
    this.onError =
      typeof options.onError === 'function'
        ? options.onError
        : (err, _task) => console.error(`[Lane ${id}] Error:`, err.message);
    this._entryCounter = 0;
    this.activeEntry = null;
    this.warningState = createWarningState();

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
        entryId: `${this.id}:${++this._entryCounter}`,
        task,
        meta,
        resolve,
        reject,
        enqueuedAt: Date.now(),
      };

      this.queue.push(entry);
      this.stats.queueHighWaterMark = Math.max(this.stats.queueHighWaterMark, this.queue.length);

      if (!this.processing) {
        void this._processQueue();
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
      this.activeEntry = {
        entryId: entry.entryId,
        enqueuedAt: entry.enqueuedAt,
        startedAt: startTime,
        meta: entry.meta,
      };

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
        this.activeEntry = null;
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

    const prevTotal = this.stats.totalProcessed - 1;
    this.stats.avgDuration =
      (this.stats.avgDuration * prevTotal + duration) / this.stats.totalProcessed;
    this.stats.maxDuration = Math.max(this.stats.maxDuration, duration);
    this.stats.lastActivityMs = now;
    this.stats.lastActivity = new Date(now).toISOString();
  }

  getWaitingTaskCount() {
    return this.queue.length;
  }

  getActiveTaskCount() {
    return this.activeEntry ? 1 : 0;
  }

  getOldestPendingEntry() {
    return this.queue[0] || null;
  }

  getOldestPendingMs(now = Date.now()) {
    const oldest = this.getOldestPendingEntry();
    return oldest ? Math.max(0, now - oldest.enqueuedAt) : null;
  }

  getLongestRunningEntry() {
    return this.activeEntry;
  }

  getActiveTaskAgeMs(now = Date.now()) {
    const active = this.getLongestRunningEntry();
    return active ? Math.max(0, now - active.startedAt) : null;
  }

  _resetWarning(kind) {
    this.warningState[kind] = { key: null, lastAt: 0 };
  }

  _maybeCollectWarning(kind, details, options, laneType, now, warnings) {
    const thresholdMs = details.thresholdMs;
    if (!Number.isFinite(thresholdMs) || thresholdMs <= 0) {
      this._resetWarning(kind);
      return;
    }

    const key = details.key;
    const ageMs = details.ageMs;
    if (!key || !Number.isFinite(ageMs) || ageMs < thresholdMs) {
      this._resetWarning(kind);
      return;
    }

    const state = this.warningState[kind];
    if (state.key !== key) {
      state.key = key;
      state.lastAt = 0;
    }

    const throttleMs = normalizePositiveNumber(
      options.warningThrottleMs,
      DEFAULT_WARNING_THROTTLE_MS,
    );
    if (state.lastAt > 0 && now - state.lastAt < throttleMs) {
      return;
    }

    state.lastAt = now;
    warnings.push({
      laneId: this.id,
      laneType,
      issue: details.issue,
      ageMs,
      thresholdMs,
      waitingTasks: this.getWaitingTaskCount(),
      activeTasks: this.getActiveTaskCount(),
      queueHighWaterMark: this.stats.queueHighWaterMark,
      timestamp: new Date(now).toISOString(),
    });
  }

  collectWarnings(now, options = {}, laneType = 'serial') {
    const warnings = [];
    const oldestPending = this.getOldestPendingEntry();
    this._maybeCollectWarning(
      'pending',
      {
        key: oldestPending?.entryId || null,
        ageMs: oldestPending ? Math.max(0, now - oldestPending.enqueuedAt) : null,
        thresholdMs: options.waitWarningMs,
        issue: 'pending_wait',
      },
      options,
      laneType,
      now,
      warnings,
    );

    const runningEntry = this.getLongestRunningEntry();
    this._maybeCollectWarning(
      'running',
      {
        key: runningEntry?.entryId || null,
        ageMs: runningEntry ? Math.max(0, now - runningEntry.startedAt) : null,
        thresholdMs: options.runningWarningMs,
        issue: 'running_task',
      },
      options,
      laneType,
      now,
      warnings,
    );

    return warnings;
  }

  /**
   * Get lane statistics.
   */
  getStats(now = Date.now()) {
    const waitingTasks = this.getWaitingTaskCount();
    const activeTasks = this.getActiveTaskCount();
    return {
      ...this.stats,
      currentQueueLength: waitingTasks,
      waitingTasks,
      activeTasks,
      isProcessing: this.processing,
      busy: !this.idle,
      oldestPendingMs: this.getOldestPendingMs(now),
      activeTaskAgeMs: this.getActiveTaskAgeMs(now),
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
    this.maxConcurrency = normalizePositiveNumber(
      options.maxConcurrency ?? options.parallelConcurrency,
      5,
    );
    this.activeTasks = 0;
    this.waitingQueue = [];
    this.activeEntries = new Map();
  }

  /**
   * Execute task with concurrency limit.
   */
  enqueue(task, meta = {}) {
    if (this.waitingQueue.length >= this.maxQueueSize) {
      return Promise.reject(new Error(`Lane ${this.id} queue full (max: ${this.maxQueueSize})`));
    }

    return new Promise((resolve, reject) => {
      const entry = {
        entryId: `${this.id}:${++this._entryCounter}`,
        task,
        meta,
        resolve,
        reject,
        enqueuedAt: Date.now(),
      };

      if (this.activeTasks < this.maxConcurrency) {
        void this._executeTask(entry);
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
    this.activeEntries.set(entry.entryId, {
      entryId: entry.entryId,
      enqueuedAt: entry.enqueuedAt,
      startedAt: startTime,
      meta: entry.meta,
    });

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
      this.activeEntries.delete(entry.entryId);
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
      void this._executeTask(next);
    }
  }

  get length() {
    return this.waitingQueue.length + this.activeTasks;
  }

  get idle() {
    return this.activeTasks === 0 && this.waitingQueue.length === 0;
  }

  getWaitingTaskCount() {
    return this.waitingQueue.length;
  }

  getActiveTaskCount() {
    return this.activeTasks;
  }

  getOldestPendingEntry() {
    return this.waitingQueue[0] || null;
  }

  getLongestRunningEntry() {
    let longest = null;
    for (const entry of this.activeEntries.values()) {
      if (!longest || entry.startedAt < longest.startedAt) {
        longest = entry;
      }
    }
    return longest;
  }

  getStats(now = Date.now()) {
    const base = super.getStats(now);
    return {
      ...base,
      currentQueueLength: this.waitingQueue.length,
      waitingTasks: this.waitingQueue.length,
      activeTasks: this.activeTasks,
      isProcessing: this.activeTasks > 0,
      maxConcurrency: this.maxConcurrency,
    };
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
   * @param {number} [options.laneTimeoutMs=300000] - Alias for laneTimeout
   * @param {number} [options.maxQueueSize=100] - Max queue size per lane
   * @param {number} [options.idleCleanupMs=3600000] - Cleanup idle lanes after (ms)
   * @param {number} [options.parallelConcurrency=5] - Concurrency limit for parallel lanes
   * @param {number} [options.waitWarningMs=30000] - Warn when a task waits longer than this
   * @param {number} [options.runningWarningMs=120000] - Warn when an active task runs longer than this
   * @param {number} [options.warningThrottleMs=30000] - Minimum gap between warnings per task
   * @param {number} [options.monitorIntervalMs=5000] - Warning scan interval
   * @param {boolean} [options.emitWarnings=true] - Enable operator warnings
   * @param {(warning: object) => void} [options.onWarning] - Custom warning sink
   */
  constructor(options = {}) {
    this.options = {
      maxLanes: normalizePositiveNumber(options.maxLanes, 1000),
      laneTimeout: normalizePositiveNumber(options.laneTimeoutMs ?? options.laneTimeout, 300000),
      maxQueueSize: normalizePositiveNumber(options.maxQueueSize, 100),
      idleCleanupMs: normalizePositiveNumber(options.idleCleanupMs, 3600000),
      parallelConcurrency: normalizePositiveNumber(options.parallelConcurrency, 5),
      waitWarningMs: normalizeThreshold(options.waitWarningMs, DEFAULT_WAIT_WARNING_MS),
      runningWarningMs: normalizeThreshold(options.runningWarningMs, DEFAULT_RUNNING_WARNING_MS),
      warningThrottleMs: normalizePositiveNumber(
        options.warningThrottleMs,
        DEFAULT_WARNING_THROTTLE_MS,
      ),
      monitorIntervalMs: normalizePositiveNumber(
        options.monitorIntervalMs,
        DEFAULT_MONITOR_INTERVAL_MS,
      ),
      emitWarnings: options.emitWarnings !== false,
      onWarning: typeof options.onWarning === 'function' ? options.onWarning : null,
    };

    /** @type {Map<string, Lane>} */
    this.lanes = new Map();

    /** @type {Map<string, ParallelLane>} */
    this.parallelLanes = new Map();

    this._cleanupInterval = setInterval(() => this._cleanupIdleLanes(), CLEANUP_CHECK_INTERVAL_MS);
    this._cleanupInterval.unref?.();

    if (
      this.options.emitWarnings &&
      (this.options.waitWarningMs > 0 || this.options.runningWarningMs > 0)
    ) {
      this._warningInterval = setInterval(
        () => this._checkWarnings(),
        this.options.monitorIntervalMs,
      );
      this._warningInterval.unref?.();
    } else {
      this._warningInterval = null;
    }
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
    const lane = this.lanes.get(laneId) || this.parallelLanes.get(laneId);
    if (!lane || lane.idle) return;

    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      if (lane.idle) return;
      await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL_MS));
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
      await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL_MS));
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

  _checkWarnings() {
    const now = Date.now();

    for (const lane of this.lanes.values()) {
      for (const warning of lane.collectWarnings(now, this.options, 'serial')) {
        this._emitWarning(warning);
      }
    }

    for (const lane of this.parallelLanes.values()) {
      for (const warning of lane.collectWarnings(now, this.options, 'parallel')) {
        this._emitWarning(warning);
      }
    }
  }

  _emitWarning(warning) {
    if (this.options.onWarning) {
      this.options.onWarning(warning);
      return;
    }

    console.warn(formatQueueWarning(warning));
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
  getStats(now = Date.now()) {
    const serialLanes = [];
    for (const [id, lane] of this.lanes.entries()) {
      serialLanes.push({ id, type: 'serial', ...lane.getStats(now) });
    }

    const parallelLanesStats = [];
    for (const [id, lane] of this.parallelLanes.entries()) {
      parallelLanesStats.push({ id, type: 'parallel', ...lane.getStats(now) });
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
        serialLanes.reduce((sum, lane) => sum + lane.waitingTasks, 0) +
        parallelLanesStats.reduce((sum, lane) => sum + lane.waitingTasks, 0),
      totalActive:
        serialLanes.reduce((sum, lane) => sum + lane.activeTasks, 0) +
        parallelLanesStats.reduce((sum, lane) => sum + lane.activeTasks, 0),
      busyLanes:
        serialLanes.filter((lane) => lane.busy).length +
        parallelLanesStats.filter((lane) => lane.busy).length,
      collectedAt: new Date(now).toISOString(),
    };
  }

  /**
   * Get statistics for a specific lane.
   * @param {string} laneId
   */
  getLaneStats(laneId, now = Date.now()) {
    const lane = this.lanes.get(laneId) || this.parallelLanes.get(laneId);
    if (!lane) return null;

    if (this.lanes.has(laneId)) {
      return { type: 'serial', ...lane.getStats(now) };
    }

    return { type: 'parallel', ...lane.getStats(now) };
  }

  /**
   * Shutdown the queue manager.
   */
  shutdown() {
    if (this._cleanupInterval) {
      clearInterval(this._cleanupInterval);
      this._cleanupInterval = null;
    }
    if (this._warningInterval) {
      clearInterval(this._warningInterval);
      this._warningInterval = null;
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
