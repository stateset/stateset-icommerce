/**
 * MCP Tool Rate Limiter — Per-Agent & Per-Tool Throttling
 *
 * Token-bucket rate limiter for MCP tool calls.
 * Prevents agent abuse and ensures fair resource allocation.
 *
 * @example
 * ```javascript
 * const limiter = createMcpRateLimiter({
 *   defaultLimits: { requestsPerMinute: 60 },
 *   toolOverrides: {
 *     a2a_pay: { requestsPerMinute: 10 },
 *     x402_sign_intent: { requestsPerMinute: 5 },
 *   },
 * });
 *
 * const result = limiter.checkLimit('agent-123', 'a2a_pay');
 * if (!result.allowed) {
 *   // Return 429 with retry-after
 *   return { error: 'Rate limited', retryAfterMs: result.retryAfterMs };
 * }
 * ```
 */

/**
 * @typedef {Object} RateLimitConfig
 * @property {{ requestsPerMinute: number }} [defaultLimits]
 * @property {Record<string, { requestsPerMinute: number }>} [toolOverrides]
 * @property {number} [cleanupIntervalMs] - Interval to purge stale buckets
 */

/**
 * Create an MCP rate limiter.
 *
 * @param {RateLimitConfig} [config]
 * @returns {Object} Rate limiter API
 */
export function createMcpRateLimiter(config = {}) {
  const defaultRpm = config.defaultLimits?.requestsPerMinute ?? 60;
  const toolOverrides = config.toolOverrides ?? {};
  const cleanupIntervalMs = config.cleanupIntervalMs ?? 60_000;

  /**
   * Sliding window counters.
   * Key format: `${agentId}:${toolName}`
   * Value: { count, windowStart }
   * @type {Map<string, { count: number, windowStart: number }>}
   */
  const _buckets = new Map();

  // Periodic cleanup of stale buckets
  const _cleanupTimer = setInterval(() => {
    const now = Date.now();
    for (const [key, bucket] of _buckets) {
      if (now - bucket.windowStart > 120_000) {
        _buckets.delete(key);
      }
    }
  }, cleanupIntervalMs);
  if (_cleanupTimer.unref) _cleanupTimer.unref();

  /**
   * Check whether a request is within rate limits.
   * If allowed, increments the counter.
   *
   * @param {string} agentId - Agent identifier
   * @param {string} toolName - MCP tool name
   * @returns {{ allowed: boolean, remaining: number, retryAfterMs: number, limit: number }}
   */
  function checkLimit(agentId, toolName) {
    const rpm = toolOverrides[toolName]?.requestsPerMinute ?? defaultRpm;
    const windowMs = 60_000;
    const now = Date.now();
    const key = `${agentId}:${toolName}`;

    let bucket = _buckets.get(key);
    if (!bucket || now - bucket.windowStart >= windowMs) {
      // New window
      bucket = { count: 0, windowStart: now };
      _buckets.set(key, bucket);
    }

    if (bucket.count >= rpm) {
      const retryAfterMs = bucket.windowStart + windowMs - now;
      return {
        allowed: false,
        remaining: 0,
        retryAfterMs: Math.max(retryAfterMs, 0),
        limit: rpm,
      };
    }

    bucket.count++;
    return {
      allowed: true,
      remaining: rpm - bucket.count,
      retryAfterMs: 0,
      limit: rpm,
    };
  }

  /**
   * Get rate limit headers for an HTTP response.
   * @param {string} agentId
   * @param {string} toolName
   * @returns {Record<string, string>}
   */
  function getHeaders(agentId, toolName) {
    const rpm = toolOverrides[toolName]?.requestsPerMinute ?? defaultRpm;
    const key = `${agentId}:${toolName}`;
    const bucket = _buckets.get(key);
    const remaining = bucket ? Math.max(rpm - bucket.count, 0) : rpm;

    return {
      'X-RateLimit-Limit': String(rpm),
      'X-RateLimit-Remaining': String(remaining),
      'X-RateLimit-Reset': String(
        bucket
          ? Math.ceil((bucket.windowStart + 60_000) / 1000)
          : Math.ceil(Date.now() / 1000) + 60,
      ),
    };
  }

  /**
   * Get current rate limit metrics.
   * @returns {{ activeBuckets: number, topAgents: Array }}
   */
  function getMetrics() {
    const agentCounts = new Map();
    for (const [key, bucket] of _buckets) {
      const agentId = key.split(':')[0];
      agentCounts.set(agentId, (agentCounts.get(agentId) || 0) + bucket.count);
    }

    const topAgents = [...agentCounts.entries()]
      .sort((a, b) => b[1] - a[1])
      .slice(0, 10)
      .map(([agentId, totalRequests]) => ({ agentId, totalRequests }));

    return {
      activeBuckets: _buckets.size,
      topAgents,
    };
  }

  /** Clean up the periodic timer. */
  function destroy() {
    clearInterval(_cleanupTimer);
    _buckets.clear();
  }

  return {
    checkLimit,
    getHeaders,
    getMetrics,
    destroy,
  };
}

export default { createMcpRateLimiter };
