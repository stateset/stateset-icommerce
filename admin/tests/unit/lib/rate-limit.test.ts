/**
 * Tests for In-Memory Sliding-Window Rate Limiter
 *
 * @module tests/unit/lib/rate-limit
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { RateLimiter, apiRateLimiter, authRateLimiter } from '@/lib/shared/rate-limit';

describe('RateLimiter', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe('consume', () => {
    it('returns allowed: true when under limit', () => {
      const limiter = new RateLimiter({ windowMs: 60_000, maxRequests: 5 });
      const result = limiter.consume('key-1');

      expect(result.allowed).toBe(true);
    });

    it('returns allowed: false after maxRequests exceeded', () => {
      const limiter = new RateLimiter({ windowMs: 60_000, maxRequests: 3 });

      limiter.consume('key-1');
      limiter.consume('key-1');
      limiter.consume('key-1');
      const result = limiter.consume('key-1');

      expect(result.allowed).toBe(false);
    });

    it('remaining decreases with each request', () => {
      const limiter = new RateLimiter({ windowMs: 60_000, maxRequests: 5 });

      const r1 = limiter.consume('key-1');
      expect(r1.remaining).toBe(4);

      const r2 = limiter.consume('key-1');
      expect(r2.remaining).toBe(3);

      const r3 = limiter.consume('key-1');
      expect(r3.remaining).toBe(2);

      const r4 = limiter.consume('key-1');
      expect(r4.remaining).toBe(1);

      const r5 = limiter.consume('key-1');
      expect(r5.remaining).toBe(0);
    });

    it('different keys are tracked independently', () => {
      const limiter = new RateLimiter({ windowMs: 60_000, maxRequests: 2 });

      limiter.consume('key-a');
      limiter.consume('key-a');
      const resultA = limiter.consume('key-a');

      const resultB = limiter.consume('key-b');

      expect(resultA.allowed).toBe(false);
      expect(resultB.allowed).toBe(true);
      expect(resultB.remaining).toBe(1);
    });

    it('expired entries do not count against limit', () => {
      const limiter = new RateLimiter({ windowMs: 60_000, maxRequests: 2 });

      limiter.consume('key-1');
      limiter.consume('key-1');

      // Advance past the window
      vi.advanceTimersByTime(61_000);

      const result = limiter.consume('key-1');
      expect(result.allowed).toBe(true);
      expect(result.remaining).toBe(1);
    });

    it('partially expired entries are handled correctly', () => {
      const limiter = new RateLimiter({ windowMs: 60_000, maxRequests: 3 });

      limiter.consume('key-1'); // t=0
      vi.advanceTimersByTime(30_000);
      limiter.consume('key-1'); // t=30s
      vi.advanceTimersByTime(35_000);
      // Now t=65s. First entry (t=0) is expired, second (t=30s) is still valid.
      const result = limiter.consume('key-1');
      expect(result.allowed).toBe(true);
      expect(result.remaining).toBe(1);
    });

    it('returns limit equal to maxRequests', () => {
      const limiter = new RateLimiter({ windowMs: 60_000, maxRequests: 42 });
      const result = limiter.consume('key-1');

      expect(result.limit).toBe(42);
    });

    it('returns remaining 0 when denied', () => {
      const limiter = new RateLimiter({ windowMs: 60_000, maxRequests: 1 });

      limiter.consume('key-1');
      const result = limiter.consume('key-1');

      expect(result.remaining).toBe(0);
    });

    it('resetAt is correctly calculated when entries exist', () => {
      vi.setSystemTime(new Date('2026-01-01T00:00:00.000Z'));
      const limiter = new RateLimiter({ windowMs: 60_000, maxRequests: 5 });

      const result = limiter.consume('key-1');

      // First timestamp is now; resetAt = now + windowMs
      expect(result.resetAt).toBe(Date.now() + 60_000);
    });

    it('resetAt points to earliest entry expiry when multiple exist', () => {
      const start = Date.now();
      const limiter = new RateLimiter({ windowMs: 60_000, maxRequests: 5 });

      limiter.consume('key-1'); // first entry at start
      vi.advanceTimersByTime(10_000);
      const result = limiter.consume('key-1');

      // resetAt should be first entry + windowMs
      expect(result.resetAt).toBe(start + 60_000);
    });

    it('handles single maxRequests correctly', () => {
      const limiter = new RateLimiter({ windowMs: 10_000, maxRequests: 1 });

      const first = limiter.consume('key-1');
      expect(first.allowed).toBe(true);
      expect(first.remaining).toBe(0);

      const second = limiter.consume('key-1');
      expect(second.allowed).toBe(false);
      expect(second.remaining).toBe(0);
    });

    it('allows requests again after window fully passes', () => {
      const limiter = new RateLimiter({ windowMs: 10_000, maxRequests: 1 });

      limiter.consume('key-1');
      vi.advanceTimersByTime(10_001);

      const result = limiter.consume('key-1');
      expect(result.allowed).toBe(true);
    });
  });

  describe('reset', () => {
    it('clears a specific key history', () => {
      const limiter = new RateLimiter({ windowMs: 60_000, maxRequests: 2 });

      limiter.consume('key-1');
      limiter.consume('key-1');
      expect(limiter.consume('key-1').allowed).toBe(false);

      limiter.reset('key-1');

      const result = limiter.consume('key-1');
      expect(result.allowed).toBe(true);
      expect(result.remaining).toBe(1);
    });

    it('does not affect other keys', () => {
      const limiter = new RateLimiter({ windowMs: 60_000, maxRequests: 2 });

      limiter.consume('key-a');
      limiter.consume('key-a');
      limiter.consume('key-b');
      limiter.consume('key-b');

      limiter.reset('key-a');

      expect(limiter.consume('key-a').allowed).toBe(true);
      expect(limiter.consume('key-b').allowed).toBe(false);
    });

    it('is safe to call on a non-existent key', () => {
      const limiter = new RateLimiter({ windowMs: 60_000, maxRequests: 5 });
      expect(() => limiter.reset('nonexistent')).not.toThrow();
    });
  });

  describe('cleanup', () => {
    it('removes entries whose timestamps are all expired', () => {
      const limiter = new RateLimiter({ windowMs: 60_000, maxRequests: 5 });

      limiter.consume('key-1');
      limiter.consume('key-2');

      vi.advanceTimersByTime(61_000);
      limiter.cleanup();

      // After cleanup, both keys should be gone — consuming returns full remaining
      const r1 = limiter.consume('key-1');
      expect(r1.remaining).toBe(4);

      const r2 = limiter.consume('key-2');
      expect(r2.remaining).toBe(4);
    });

    it('retains entries with valid timestamps', () => {
      const limiter = new RateLimiter({ windowMs: 60_000, maxRequests: 3 });

      limiter.consume('key-1');
      vi.advanceTimersByTime(30_000);
      limiter.consume('key-1');

      vi.advanceTimersByTime(35_000);
      // First entry expired, second still valid
      limiter.cleanup();

      const result = limiter.consume('key-1');
      // Should have 1 valid entry + 1 new = 2 total, so remaining = 1
      expect(result.allowed).toBe(true);
      expect(result.remaining).toBe(1);
    });

    it('is safe to call when there are no entries', () => {
      const limiter = new RateLimiter({ windowMs: 60_000, maxRequests: 5 });
      expect(() => limiter.cleanup()).not.toThrow();
    });
  });

  describe('pre-configured instances', () => {
    it('apiRateLimiter allows 100 requests per minute', () => {
      // Reset to clear any state from other tests
      apiRateLimiter.reset('test-ip');

      for (let i = 0; i < 100; i++) {
        const result = apiRateLimiter.consume('test-ip');
        expect(result.allowed).toBe(true);
        expect(result.limit).toBe(100);
      }

      const denied = apiRateLimiter.consume('test-ip');
      expect(denied.allowed).toBe(false);
      expect(denied.limit).toBe(100);

      // Cleanup
      apiRateLimiter.reset('test-ip');
    });

    it('authRateLimiter allows 10 requests per minute', () => {
      authRateLimiter.reset('test-ip');

      for (let i = 0; i < 10; i++) {
        const result = authRateLimiter.consume('test-ip');
        expect(result.allowed).toBe(true);
        expect(result.limit).toBe(10);
      }

      const denied = authRateLimiter.consume('test-ip');
      expect(denied.allowed).toBe(false);
      expect(denied.limit).toBe(10);

      // Cleanup
      authRateLimiter.reset('test-ip');
    });
  });
});
