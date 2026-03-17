/**
 * A2A Tick Loop Optimizer
 *
 * Middleware that wraps the agent runtime tick loop with performance
 * optimizations: overlap prevention, adaptive polling, duration metrics
 * (p50/p95/p99), and a size-limited processed-ID tracker with LRU eviction.
 *
 * @example
 * ```javascript
 * import { createTickOptimizer, createProcessedIdTracker } from './tick-optimizer.js';
 *
 * const optimizer = createTickOptimizer({ baseIntervalMs: 5000 });
 * const tick = optimizer.wrapTick(async () => {
 *   const items = await pollQueue();
 *   for (const item of items) await process(item);
 *   return items.length;                   // items processed
 * });
 *
 * setInterval(() => tick(), optimizer.getAdaptiveInterval());
 *
 * console.log(optimizer.getMetrics());
 * ```
 */

// ─── Constants ───────────────────────────────────────────────────────────────

/** Default base polling interval (5 s) */
const DEFAULT_BASE_INTERVAL_MS = 5_000;

/** Maximum interval ceiling when backing off (30 s) */
const DEFAULT_MAX_INTERVAL_MS = 30_000;

/** Consecutive idle ticks before we start doubling the interval */
const IDLE_TICKS_BEFORE_BACKOFF = 3;

/** Warn when a tick consumes more than this fraction of the interval */
const WARN_THRESHOLD = 0.8;

/** Default maximum number of tracked processed IDs (LRU tracker) */
const DEFAULT_MAX_TRACKED_IDS = 100_000;

// ─── Percentile helper ──────────────────────────────────────────────────────

/**
 * Compute a percentile value from a **sorted** array of numbers.
 *
 * @param {number[]} sorted – Sorted (ascending) array
 * @param {number}   p      – Percentile in [0, 1]
 * @returns {number}
 */
function percentile(sorted, p) {
  if (sorted.length === 0) return 0;
  if (sorted.length === 1) return sorted[0];
  const idx = (sorted.length - 1) * p;
  const lo = Math.floor(idx);
  const hi = Math.ceil(idx);
  if (lo === hi) return sorted[lo];
  return sorted[lo] + (sorted[hi] - sorted[lo]) * (idx - lo);
}

// ─── Tick Optimizer ──────────────────────────────────────────────────────────

/**
 * Create a tick-loop optimizer.
 *
 * @param {Object} [options]
 * @param {number} [options.baseIntervalMs=5000]  – Base polling interval (ms)
 * @param {number} [options.maxIntervalMs=30000]  – Maximum back-off interval (ms)
 * @param {number} [options.idleThreshold=3]      – Consecutive idle ticks before back-off
 * @returns {{ wrapTick: Function, getMetrics: Function, getAdaptiveInterval: Function, reset: Function }}
 */
export function createTickOptimizer(options = {}) {
  const {
    baseIntervalMs = DEFAULT_BASE_INTERVAL_MS,
    maxIntervalMs = DEFAULT_MAX_INTERVAL_MS,
    idleThreshold = IDLE_TICKS_BEFORE_BACKOFF,
  } = options;

  // ── Metrics state ──────────────────────────────────────────────────────

  /** @type {number[]} Duration samples (ms) for percentile calculation */
  let durations = [];
  let totalTicks = 0;
  let overlappingTicksSkipped = 0;
  let consecutiveIdleTicks = 0;
  let currentIntervalMs = baseIntervalMs;
  let errors = 0;
  /** @type {{ message: string, tick: number }[]} */
  let warnings = [];
  let running = false;

  // ── Adaptive interval logic ────────────────────────────────────────────

  /**
   * Recalculate `currentIntervalMs` based on recent tick activity.
   *
   * @param {number} itemsProcessed – Number of items processed in the latest tick
   */
  function updateInterval(itemsProcessed) {
    if (itemsProcessed > 0) {
      // Activity detected → snap back to base interval
      consecutiveIdleTicks = 0;
      currentIntervalMs = baseIntervalMs;
    } else {
      consecutiveIdleTicks += 1;
      if (consecutiveIdleTicks >= idleThreshold) {
        currentIntervalMs = Math.min(currentIntervalMs * 2, maxIntervalMs);
      }
    }
  }

  // ── Wrap tick ──────────────────────────────────────────────────────────

  /**
   * Wrap a tick function with overlap prevention, duration tracking,
   * and adaptive polling.
   *
   * The wrapped function returns a result object:
   * ```
   * { skipped?: boolean, durationMs?: number, itemsProcessed?: number, error?: string }
   * ```
   *
   * @param {Function} tickFn – The original tick function.
   *   If it returns a number, that is treated as items-processed count.
   *   If it returns an object with `{ itemsProcessed }`, that value is used.
   * @returns {Function} Wrapped async tick function
   */
  function wrapTick(tickFn) {
    return async function wrappedTick() {
      // ─ Overlap guard ─────────────────────────────────────────────────
      if (running) {
        overlappingTicksSkipped += 1;
        return { skipped: true };
      }

      running = true;
      const start = Date.now();

      try {
        const result = await tickFn();

        const durationMs = Date.now() - start;
        durations.push(durationMs);
        totalTicks += 1;

        // Determine items processed from return value
        let itemsProcessed = 0;
        if (typeof result === 'number') {
          itemsProcessed = result;
        } else if (
          result &&
          typeof result === 'object' &&
          typeof result.itemsProcessed === 'number'
        ) {
          itemsProcessed = result.itemsProcessed;
        }

        updateInterval(itemsProcessed);

        // Warning when tick consumed >80 % of the interval
        if (durationMs > currentIntervalMs * WARN_THRESHOLD) {
          warnings.push({
            message: `Tick #${totalTicks} took ${durationMs}ms (>${Math.round(WARN_THRESHOLD * 100)}% of ${currentIntervalMs}ms interval)`,
            tick: totalTicks,
          });
        }

        return { durationMs, itemsProcessed };
      } catch (err) {
        const durationMs = Date.now() - start;
        durations.push(durationMs);
        totalTicks += 1;
        errors += 1;
        updateInterval(0);
        return { durationMs, itemsProcessed: 0, error: err.message || String(err) };
      } finally {
        running = false;
      }
    };
  }

  // ── Metrics ────────────────────────────────────────────────────────────

  /**
   * Return current tick-loop metrics.
   *
   * @returns {Object}
   */
  function getMetrics() {
    const sorted = [...durations].sort((a, b) => a - b);
    const sum = sorted.reduce((s, d) => s + d, 0);

    return {
      totalTicks,
      avgDurationMs: totalTicks > 0 ? Math.round(sum / totalTicks) : 0,
      p50DurationMs: percentile(sorted, 0.5),
      p95DurationMs: percentile(sorted, 0.95),
      p99DurationMs: percentile(sorted, 0.99),
      maxDurationMs: sorted.length > 0 ? sorted[sorted.length - 1] : 0,
      minDurationMs: sorted.length > 0 ? sorted[0] : 0,
      ticksPerMinute:
        totalTicks > 0 && durations.length > 0 ? Math.round((totalTicks / (sum / 1000)) * 60) : 0,
      overlappingTicksSkipped,
      consecutiveIdleTicks,
      currentIntervalMs,
      errors,
      warnings: [...warnings],
    };
  }

  /**
   * Return the current adaptive polling interval (ms).
   *
   * @returns {number}
   */
  function getAdaptiveInterval() {
    return currentIntervalMs;
  }

  /**
   * Reset all metrics and interval state.
   */
  function reset() {
    durations = [];
    totalTicks = 0;
    overlappingTicksSkipped = 0;
    consecutiveIdleTicks = 0;
    currentIntervalMs = baseIntervalMs;
    errors = 0;
    warnings = [];
    running = false;
  }

  return { wrapTick, getMetrics, getAdaptiveInterval, reset };
}

// ─── Processed-ID LRU Tracker ────────────────────────────────────────────────

/**
 * Create a size-limited processed-ID tracker with LRU eviction.
 *
 * Internally uses a `Map` to maintain insertion order — the oldest entries are
 * evicted first when `maxSize` is exceeded.
 *
 * @param {number} [maxSize=100000] – Maximum number of IDs to track
 * @returns {{ add: (id: string) => void, has: (id: string) => boolean, size: number, clear: () => void }}
 */
export function createProcessedIdTracker(maxSize = DEFAULT_MAX_TRACKED_IDS) {
  /** @type {Map<string, true>} */
  const map = new Map();

  /**
   * Add an ID to the tracker.  If the ID already exists it is refreshed
   * (moved to the end).  If the tracker is at capacity the oldest entry
   * is evicted.
   *
   * @param {string} id
   */
  function add(id) {
    // Refresh: delete + re-insert moves the entry to the end
    if (map.has(id)) {
      map.delete(id);
    }
    map.set(id, true);

    // Evict oldest (first inserted) when over capacity
    if (map.size > maxSize) {
      const oldest = map.keys().next().value;
      map.delete(oldest);
    }
  }

  /**
   * Check whether an ID has been tracked.
   *
   * @param {string} id
   * @returns {boolean}
   */
  function has(id) {
    return map.has(id);
  }

  /**
   * Clear all tracked IDs.
   */
  function clear() {
    map.clear();
  }

  return {
    add,
    has,
    /** Current number of tracked IDs */
    get size() {
      return map.size;
    },
    clear,
  };
}

export default {
  createTickOptimizer,
  createProcessedIdTracker,
};
