/**
 * Unit tests for rate-limiter.js
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert';
import { RateLimiter, createRateLimiter } from '../../src/channels/rate-limiter.js';

describe('RateLimiter', () => {
  /** @type {RateLimiter} */
  let limiter;

  afterEach(() => {
    if (limiter) limiter.destroy();
  });

  describe('constructor', () => {
    it('uses default options when none provided', () => {
      limiter = new RateLimiter();
      const s = limiter.stats();
      assert.strictEqual(s.windowMs, 60_000);
      assert.strictEqual(s.maxRequests, 60);
      assert.strictEqual(s.trackedKeys, 0);
    });

    it('accepts custom options', () => {
      limiter = new RateLimiter({ windowMs: 5000, maxRequests: 10 });
      const s = limiter.stats();
      assert.strictEqual(s.windowMs, 5000);
      assert.strictEqual(s.maxRequests, 10);
    });
  });

  describe('check()', () => {
    it('allows requests within limit', () => {
      limiter = new RateLimiter({ maxRequests: 5, windowMs: 60_000 });

      for (let i = 0; i < 5; i++) {
        const result = limiter.check('user-a');
        assert.strictEqual(result.allowed, true, `Request ${i + 1} should be allowed`);
        assert.strictEqual(result.remaining, 4 - i);
        assert.strictEqual(result.limit, 5);
        assert.strictEqual(result.retryAfterMs, 0);
      }
    });

    it('blocks requests over limit', () => {
      limiter = new RateLimiter({ maxRequests: 3, windowMs: 60_000 });

      limiter.check('user-b');
      limiter.check('user-b');
      limiter.check('user-b');

      const result = limiter.check('user-b');
      assert.strictEqual(result.allowed, false);
      assert.strictEqual(result.remaining, 0);
      assert.strictEqual(result.limit, 3);
      assert.ok(result.retryAfterMs > 0, 'retryAfterMs should be positive');
    });

    it('tracks different keys independently', () => {
      limiter = new RateLimiter({ maxRequests: 2, windowMs: 60_000 });

      limiter.check('user-x');
      limiter.check('user-x');

      const blockedX = limiter.check('user-x');
      assert.strictEqual(blockedX.allowed, false);

      const allowedY = limiter.check('user-y');
      assert.strictEqual(allowedY.allowed, true);
      assert.strictEqual(allowedY.remaining, 1);
    });

    it('allows requests again after window expires', async () => {
      limiter = new RateLimiter({ maxRequests: 2, windowMs: 50 });

      limiter.check('user-c');
      limiter.check('user-c');

      const blocked = limiter.check('user-c');
      assert.strictEqual(blocked.allowed, false);

      // Wait for the window to expire
      await new Promise((r) => setTimeout(r, 70));

      const allowed = limiter.check('user-c');
      assert.strictEqual(allowed.allowed, true);
    });

    it('returns correct remaining count', () => {
      limiter = new RateLimiter({ maxRequests: 5, windowMs: 60_000 });

      assert.strictEqual(limiter.check('user-d').remaining, 4);
      assert.strictEqual(limiter.check('user-d').remaining, 3);
      assert.strictEqual(limiter.check('user-d').remaining, 2);
      assert.strictEqual(limiter.check('user-d').remaining, 1);
      assert.strictEqual(limiter.check('user-d').remaining, 0);
    });

    it('handles single-request limit', () => {
      limiter = new RateLimiter({ maxRequests: 1, windowMs: 60_000 });

      const first = limiter.check('user-e');
      assert.strictEqual(first.allowed, true);
      assert.strictEqual(first.remaining, 0);

      const second = limiter.check('user-e');
      assert.strictEqual(second.allowed, false);
    });
  });

  describe('reset()', () => {
    it('clears rate limit for a specific key', () => {
      limiter = new RateLimiter({ maxRequests: 2, windowMs: 60_000 });

      limiter.check('user-f');
      limiter.check('user-f');

      const blocked = limiter.check('user-f');
      assert.strictEqual(blocked.allowed, false);

      limiter.reset('user-f');

      const allowed = limiter.check('user-f');
      assert.strictEqual(allowed.allowed, true);
      assert.strictEqual(allowed.remaining, 1);
    });

    it('does not affect other keys', () => {
      limiter = new RateLimiter({ maxRequests: 2, windowMs: 60_000 });

      limiter.check('user-g');
      limiter.check('user-h');

      limiter.reset('user-g');

      assert.strictEqual(limiter.stats().trackedKeys, 1);
    });

    it('is safe to reset non-existent key', () => {
      limiter = new RateLimiter();
      limiter.reset('nonexistent'); // should not throw
    });
  });

  describe('stats()', () => {
    it('reports tracked key count', () => {
      limiter = new RateLimiter({ maxRequests: 10, windowMs: 60_000 });

      assert.strictEqual(limiter.stats().trackedKeys, 0);

      limiter.check('a');
      assert.strictEqual(limiter.stats().trackedKeys, 1);

      limiter.check('b');
      assert.strictEqual(limiter.stats().trackedKeys, 2);

      limiter.check('a');
      assert.strictEqual(limiter.stats().trackedKeys, 2);
    });
  });

  describe('destroy()', () => {
    it('clears all data', () => {
      limiter = new RateLimiter({ maxRequests: 10, windowMs: 60_000 });

      limiter.check('x');
      limiter.check('y');
      assert.strictEqual(limiter.stats().trackedKeys, 2);

      limiter.destroy();
      assert.strictEqual(limiter.stats().trackedKeys, 0);
    });
  });

  describe('_cleanup()', () => {
    it('purges expired entries', async () => {
      limiter = new RateLimiter({ maxRequests: 10, windowMs: 50, cleanupIntervalMs: 100_000 });

      limiter.check('cleanup-test');
      assert.strictEqual(limiter.stats().trackedKeys, 1);

      await new Promise((r) => setTimeout(r, 70));

      // Manually trigger cleanup
      limiter._cleanup();
      assert.strictEqual(limiter.stats().trackedKeys, 0);
    });
  });
});

describe('createRateLimiter', () => {
  let rl;

  afterEach(() => {
    if (rl) rl.destroy();
  });

  it('creates rate limiter with default options', () => {
    rl = createRateLimiter();
    const stats = rl.stats();
    assert.strictEqual(stats.authenticated.maxRequests, 60);
    assert.strictEqual(stats.unauthenticated.maxRequests, 30);
  });

  it('accepts custom limits', () => {
    rl = createRateLimiter({ authenticatedMax: 100, unauthenticatedMax: 10 });
    const stats = rl.stats();
    assert.strictEqual(stats.authenticated.maxRequests, 100);
    assert.strictEqual(stats.unauthenticated.maxRequests, 10);
  });

  it('checkAuth limits authenticated requests', () => {
    rl = createRateLimiter({ authenticatedMax: 2 });

    assert.strictEqual(rl.checkAuth('admin').allowed, true);
    assert.strictEqual(rl.checkAuth('admin').allowed, true);
    assert.strictEqual(rl.checkAuth('admin').allowed, false);
  });

  it('checkIp limits IP-based requests', () => {
    rl = createRateLimiter({ unauthenticatedMax: 2 });

    assert.strictEqual(rl.checkIp('127.0.0.1').allowed, true);
    assert.strictEqual(rl.checkIp('127.0.0.1').allowed, true);
    assert.strictEqual(rl.checkIp('127.0.0.1').allowed, false);
  });

  it('auth and IP limits are independent', () => {
    rl = createRateLimiter({ authenticatedMax: 1, unauthenticatedMax: 1 });

    // Same identifier in different limiters
    assert.strictEqual(rl.checkAuth('test').allowed, true);
    assert.strictEqual(rl.checkAuth('test').allowed, false);

    assert.strictEqual(rl.checkIp('test').allowed, true);
    assert.strictEqual(rl.checkIp('test').allowed, false);
  });

  it('destroy cleans up both limiters', () => {
    rl = createRateLimiter();
    rl.checkAuth('user1');
    rl.checkIp('10.0.0.1');

    rl.destroy();

    const stats = rl.stats();
    assert.strictEqual(stats.authenticated.trackedKeys, 0);
    assert.strictEqual(stats.unauthenticated.trackedKeys, 0);
  });
});
