/**
 * In-memory sliding-window rate limiter.
 *
 * Tracks request timestamps per key and enforces a maximum number of
 * requests within a rolling time window.
 */

interface RateLimitResult {
  allowed: boolean;
  remaining: number;
  limit: number;
  resetAt: number;
}

interface RateLimiterOptions {
  windowMs: number;
  maxRequests: number;
}

export class RateLimiter {
  private readonly windowMs: number;
  private readonly maxRequests: number;
  private readonly requests: Map<string, number[]>;

  constructor(options: RateLimiterOptions) {
    this.windowMs = options.windowMs;
    this.maxRequests = options.maxRequests;
    this.requests = new Map();
  }

  consume(key: string): RateLimitResult {
    const now = Date.now();
    const windowStart = now - this.windowMs;

    // Get existing timestamps, filter expired
    let timestamps = this.requests.get(key) || [];
    timestamps = timestamps.filter((t) => t > windowStart);

    const remaining = Math.max(0, this.maxRequests - timestamps.length);
    const resetAt = timestamps.length > 0 ? timestamps[0] + this.windowMs : now + this.windowMs;

    if (timestamps.length >= this.maxRequests) {
      this.requests.set(key, timestamps);
      return { allowed: false, remaining: 0, limit: this.maxRequests, resetAt };
    }

    timestamps.push(now);
    this.requests.set(key, timestamps);
    return { allowed: true, remaining: remaining - 1, limit: this.maxRequests, resetAt };
  }

  reset(key: string): void {
    this.requests.delete(key);
  }

  /** Remove all expired entries to prevent memory leaks */
  cleanup(): void {
    const now = Date.now();
    const windowStart = now - this.windowMs;
    for (const [key, timestamps] of this.requests) {
      const valid = timestamps.filter((t) => t > windowStart);
      if (valid.length === 0) {
        this.requests.delete(key);
      } else {
        this.requests.set(key, valid);
      }
    }
  }
}

/** General API rate limiter: 100 requests per minute */
export const apiRateLimiter = new RateLimiter({ windowMs: 60_000, maxRequests: 100 });

/** Auth endpoints: 10 requests per minute (stricter) */
export const authRateLimiter = new RateLimiter({ windowMs: 60_000, maxRequests: 10 });

// Periodic cleanup every 5 minutes
const cleanupInterval = setInterval(() => {
  apiRateLimiter.cleanup();
  authRateLimiter.cleanup();
}, 5 * 60_000);
cleanupInterval.unref?.();
