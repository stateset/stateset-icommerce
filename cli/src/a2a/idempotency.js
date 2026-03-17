/**
 * Idempotency Guard — Prevents Duplicate Execution of Operations
 *
 * Critical for AI agents that retry on failure. Ensures that the same
 * operation (identified by a unique key) is only executed once, even
 * under concurrent access.
 *
 * @example
 * ```javascript
 * const guard = createIdempotencyGuard({ ttlMs: 3600_000 });
 *
 * // First call executes fn
 * const result = await guard.execute('pay-abc-123', () => processPayment());
 *
 * // Second call returns cached result without executing fn again
 * const cached = await guard.execute('pay-abc-123', () => processPayment());
 * // cached === result
 * ```
 */

/**
 * @typedef {Object} IdempotencyEntry
 * @property {string} key
 * @property {'pending'|'completed'|'failed'} status
 * @property {*} result
 * @property {string} startedAt
 * @property {string|null} completedAt
 * @property {Error|null} error
 */

/**
 * @typedef {Object} IdempotencyMetrics
 * @property {number} hits    - Cache hits (key already existed)
 * @property {number} misses  - Cache misses (new execution)
 * @property {number} size    - Current number of entries
 * @property {number} evictions - Number of entries evicted due to maxSize
 */

/**
 * @typedef {Object} IdempotencyOptions
 * @property {number} [ttlMs=86400000] - Time-to-live for completed entries (default 24h)
 * @property {number} [maxSize=10000]  - Maximum number of entries before LRU eviction
 */

/**
 * Create an idempotency guard.
 *
 * @param {IdempotencyOptions} [options]
 * @returns {{ execute: Function, has: Function, invalidate: Function, getMetrics: Function, clear: Function }}
 */
export function createIdempotencyGuard(options = {}) {
  const ttlMs = options.ttlMs ?? 86_400_000; // 24 hours
  const maxSize = options.maxSize ?? 10_000;

  /**
   * Map of key -> entry.
   * Entry shape: { status, result, error, startedAt, completedAt, waiters }
   * `waiters` is an array of { resolve, reject } for concurrent callers.
   * @type {Map<string, Object>}
   */
  const _entries = new Map();

  let _hits = 0;
  let _misses = 0;
  let _evictions = 0;

  /**
   * Evict the oldest entry when maxSize is exceeded.
   * Uses insertion order (Map iterates in insertion order).
   */
  function _evictIfNeeded() {
    while (_entries.size > maxSize) {
      const firstKey = _entries.keys().next().value;
      _entries.delete(firstKey);
      _evictions++;
    }
  }

  /**
   * Remove expired entries (completed/failed entries older than TTL).
   */
  function _purgeExpired() {
    const now = Date.now();
    for (const [key, entry] of _entries) {
      if (entry.status === 'pending') continue;
      const completedTime = entry.completedAt ? new Date(entry.completedAt).getTime() : 0;
      if (now - completedTime > ttlMs) {
        _entries.delete(key);
      }
    }
  }

  /**
   * Execute a function with idempotency protection.
   *
   * - If `key` has never been seen, execute `fn` and cache the result.
   * - If `key` is currently executing (pending), wait for the first execution.
   * - If `key` has a completed/failed result that hasn't expired, return it.
   *
   * @param {string} key - Unique idempotency key
   * @param {Function} fn - The function to execute (called with no arguments)
   * @returns {Promise<*>} The result of `fn`
   */
  async function execute(key, fn) {
    if (!key || typeof key !== 'string') {
      throw new Error('Idempotency key must be a non-empty string');
    }
    if (typeof fn !== 'function') {
      throw new Error('fn must be a function');
    }

    // Purge expired entries periodically (cheap check)
    _purgeExpired();

    const existing = _entries.get(key);

    if (existing) {
      // Completed — return cached result
      if (existing.status === 'completed') {
        _hits++;
        return existing.result;
      }

      // Failed — return cached error (don't re-execute; caller must invalidate)
      if (existing.status === 'failed') {
        _hits++;
        throw existing.error;
      }

      // Pending — wait for the first execution to finish
      _hits++;
      return new Promise((resolve, reject) => {
        existing.waiters.push({ resolve, reject });
      });
    }

    // New key — execute
    _misses++;
    const now = new Date().toISOString();
    const entry = {
      status: 'pending',
      result: undefined,
      error: null,
      startedAt: now,
      completedAt: null,
      waiters: [],
    };
    _entries.set(key, entry);
    _evictIfNeeded();

    try {
      const result = await fn();
      entry.status = 'completed';
      entry.result = result;
      entry.completedAt = new Date().toISOString();

      // Notify waiters
      for (const waiter of entry.waiters) {
        waiter.resolve(result);
      }
      entry.waiters = [];

      return result;
    } catch (err) {
      entry.status = 'failed';
      entry.error = err;
      entry.completedAt = new Date().toISOString();

      // Notify waiters
      for (const waiter of entry.waiters) {
        waiter.reject(err);
      }
      entry.waiters = [];

      throw err;
    }
  }

  /**
   * Check if a key exists in the guard (regardless of status).
   *
   * @param {string} key
   * @returns {boolean}
   */
  function has(key) {
    _purgeExpired();
    return _entries.has(key);
  }

  /**
   * Manually invalidate (remove) a key, allowing re-execution.
   * Use after a permanent fix when a previous execution failed.
   *
   * @param {string} key
   * @returns {boolean} true if the key was found and removed
   */
  function invalidate(key) {
    return _entries.delete(key);
  }

  /**
   * Get guard metrics.
   *
   * @returns {IdempotencyMetrics}
   */
  function getMetrics() {
    _purgeExpired();
    return {
      hits: _hits,
      misses: _misses,
      size: _entries.size,
      evictions: _evictions,
    };
  }

  /**
   * Clear all entries and reset metrics.
   */
  function clear() {
    _entries.clear();
    _hits = 0;
    _misses = 0;
    _evictions = 0;
  }

  return { execute, has, invalidate, getMetrics, clear };
}

export default { createIdempotencyGuard };
