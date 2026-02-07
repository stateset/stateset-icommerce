/**
 * In-memory sliding window rate limiter for HTTP Gateway.
 *
 * Uses a simple sliding window counter approach with automatic cleanup.
 * No external dependencies.
 *
 * @module rate-limiter
 */

/**
 * @typedef {Object} RateLimiterOptions
 * @property {number} [windowMs=60000] - Time window in milliseconds
 * @property {number} [maxRequests=60] - Max requests per window
 * @property {number} [cleanupIntervalMs=60000] - How often to purge expired entries
 */

/**
 * @typedef {Object} RateLimitResult
 * @property {boolean} allowed - Whether the request is allowed
 * @property {number} remaining - Remaining requests in the window
 * @property {number} limit - The limit for this window
 * @property {number} retryAfterMs - Milliseconds until the oldest request expires (0 if allowed)
 */

export class RateLimiter {
  /**
   * @param {RateLimiterOptions} [options]
   */
  constructor(options = {}) {
    this._windowMs = options.windowMs || 60_000;
    this._maxRequests = options.maxRequests || 60;
    this._cleanupIntervalMs = options.cleanupIntervalMs || 60_000;

    /** @type {Map<string, number[]>} key → sorted array of timestamps */
    this._buckets = new Map();

    this._cleanupTimer = setInterval(() => this._cleanup(), this._cleanupIntervalMs);
    // Don't keep the process alive just for cleanup
    if (this._cleanupTimer.unref) {
      this._cleanupTimer.unref();
    }
  }

  /**
   * Check and consume a request for the given key.
   *
   * @param {string} key - Identifier (API key name, IP address, etc.)
   * @returns {RateLimitResult}
   */
  check(key) {
    const now = Date.now();
    const windowStart = now - this._windowMs;

    let timestamps = this._buckets.get(key);
    if (!timestamps) {
      timestamps = [];
      this._buckets.set(key, timestamps);
    }

    // Remove expired timestamps from the front
    while (timestamps.length > 0 && timestamps[0] <= windowStart) {
      timestamps.shift();
    }

    if (timestamps.length >= this._maxRequests) {
      // Oldest request in the window determines when the next slot opens
      const retryAfterMs = timestamps[0] - windowStart;
      return {
        allowed: false,
        remaining: 0,
        limit: this._maxRequests,
        retryAfterMs: Math.max(retryAfterMs, 1),
      };
    }

    timestamps.push(now);
    return {
      allowed: true,
      remaining: this._maxRequests - timestamps.length,
      limit: this._maxRequests,
      retryAfterMs: 0,
    };
  }

  /**
   * Reset the limiter for a specific key (e.g. after ban expires).
   * @param {string} key
   */
  reset(key) {
    this._buckets.delete(key);
  }

  /**
   * Purge all expired entries.
   * @private
   */
  _cleanup() {
    const windowStart = Date.now() - this._windowMs;
    for (const [key, timestamps] of this._buckets) {
      while (timestamps.length > 0 && timestamps[0] <= windowStart) {
        timestamps.shift();
      }
      if (timestamps.length === 0) {
        this._buckets.delete(key);
      }
    }
  }

  /**
   * Stop the cleanup timer and clear all data.
   */
  destroy() {
    clearInterval(this._cleanupTimer);
    this._buckets.clear();
  }

  /**
   * Get current stats for monitoring.
   * @returns {{ trackedKeys: number, windowMs: number, maxRequests: number }}
   */
  stats() {
    return {
      trackedKeys: this._buckets.size,
      windowMs: this._windowMs,
      maxRequests: this._maxRequests,
    };
  }
}

/**
 * Create a rate limiter middleware object for the HTTP gateway.
 *
 * @param {Object} [options]
 * @param {number} [options.authenticatedMax=60] - Max requests/min for authenticated clients
 * @param {number} [options.unauthenticatedMax=30] - Max requests/min for unauthenticated clients
 * @param {number} [options.windowMs=60000] - Time window in milliseconds
 * @returns {{ checkAuth: (identityName: string) => RateLimitResult, checkIp: (ip: string) => RateLimitResult, destroy: () => void, stats: () => Object }}
 */
export function createRateLimiter(options = {}) {
  const windowMs = options.windowMs || 60_000;

  const authLimiter = new RateLimiter({
    windowMs,
    maxRequests: options.authenticatedMax || 60,
  });

  const ipLimiter = new RateLimiter({
    windowMs,
    maxRequests: options.unauthenticatedMax || 30,
  });

  return {
    checkAuth(identityName) {
      return authLimiter.check(identityName);
    },
    checkIp(ip) {
      return ipLimiter.check(ip);
    },
    destroy() {
      authLimiter.destroy();
      ipLimiter.destroy();
    },
    stats() {
      return {
        authenticated: authLimiter.stats(),
        unauthenticated: ipLimiter.stats(),
      };
    },
  };
}
