/**
 * Tests for cli/src/a2a/rate-limiter.js
 *
 * Covers: createMcpRateLimiter — checkLimit, getHeaders, getMetrics, destroy,
 * per-agent isolation, tool-specific overrides, window reset behaviour.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';

import { createMcpRateLimiter } from '../../src/a2a/rate-limiter.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Create a limiter with very short cleanup interval (won't fire during tests). */
function makeLimiter(opts = {}) {
  return createMcpRateLimiter({
    cleanupIntervalMs: 600_000, // long enough to not fire
    ...opts,
  });
}

// ---------------------------------------------------------------------------
// 1. Allows requests within limit
// ---------------------------------------------------------------------------

describe('RateLimiter — allows requests within limit', () => {
  let limiter;

  afterEach(() => {
    if (limiter) limiter.destroy();
  });

  it('allows the first request', () => {
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: 10 } });
    const result = limiter.checkLimit('agent-1', 'tool_a');
    assert.equal(result.allowed, true);
    assert.equal(result.retryAfterMs, 0);
    assert.equal(result.limit, 10);
  });

  it('allows requests up to the exact limit', () => {
    const rpm = 5;
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: rpm } });

    for (let i = 0; i < rpm; i++) {
      const result = limiter.checkLimit('agent-1', 'tool_a');
      assert.equal(result.allowed, true, `request ${i + 1} should be allowed`);
    }
  });

  it('returns correct remaining count after each request', () => {
    const rpm = 5;
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: rpm } });

    for (let i = 0; i < rpm; i++) {
      const result = limiter.checkLimit('agent-1', 'tool_a');
      assert.equal(result.remaining, rpm - i - 1, `remaining after request ${i + 1}`);
    }
  });

  it('uses default 60 RPM when no config is provided', () => {
    limiter = makeLimiter();
    const result = limiter.checkLimit('agent-1', 'tool_a');
    assert.equal(result.limit, 60);
    assert.equal(result.remaining, 59);
  });
});

// ---------------------------------------------------------------------------
// 2. Blocks requests exceeding limit
// ---------------------------------------------------------------------------

describe('RateLimiter — blocks requests exceeding limit', () => {
  let limiter;

  afterEach(() => {
    if (limiter) limiter.destroy();
  });

  it('denies the request after limit is reached', () => {
    const rpm = 3;
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: rpm } });

    for (let i = 0; i < rpm; i++) {
      limiter.checkLimit('agent-1', 'tool_a');
    }

    const denied = limiter.checkLimit('agent-1', 'tool_a');
    assert.equal(denied.allowed, false);
    assert.equal(denied.remaining, 0);
    assert.ok(denied.retryAfterMs >= 0, 'should have non-negative retryAfterMs');
  });

  it('returns retryAfterMs > 0 for denied requests', () => {
    const rpm = 1;
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: rpm } });
    limiter.checkLimit('agent-1', 'tool_a');

    const denied = limiter.checkLimit('agent-1', 'tool_a');
    assert.equal(denied.allowed, false);
    assert.ok(denied.retryAfterMs > 0, 'retryAfterMs should be positive');
  });

  it('continues to deny after repeated attempts', () => {
    const rpm = 2;
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: rpm } });
    limiter.checkLimit('agent-1', 'tool_a');
    limiter.checkLimit('agent-1', 'tool_a');

    for (let i = 0; i < 5; i++) {
      const denied = limiter.checkLimit('agent-1', 'tool_a');
      assert.equal(denied.allowed, false, `attempt ${i + 1} past limit should be denied`);
    }
  });
});

// ---------------------------------------------------------------------------
// 3. Returns correct remaining count
// ---------------------------------------------------------------------------

describe('RateLimiter — remaining count', () => {
  let limiter;

  afterEach(() => {
    if (limiter) limiter.destroy();
  });

  it('decrements remaining by 1 for each allowed request', () => {
    const rpm = 10;
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: rpm } });

    const r1 = limiter.checkLimit('agent-1', 'tool_a');
    assert.equal(r1.remaining, 9);

    const r2 = limiter.checkLimit('agent-1', 'tool_a');
    assert.equal(r2.remaining, 8);

    const r3 = limiter.checkLimit('agent-1', 'tool_a');
    assert.equal(r3.remaining, 7);
  });

  it('returns remaining=0 when at the limit', () => {
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: 3 } });
    limiter.checkLimit('agent-1', 'tool_a');
    limiter.checkLimit('agent-1', 'tool_a');
    const last = limiter.checkLimit('agent-1', 'tool_a');
    assert.equal(last.remaining, 0);
    assert.equal(last.allowed, true);
  });

  it('returns remaining=0 when over the limit', () => {
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: 1 } });
    limiter.checkLimit('agent-1', 'tool_a');

    const denied = limiter.checkLimit('agent-1', 'tool_a');
    assert.equal(denied.remaining, 0);
    assert.equal(denied.allowed, false);
  });
});

// ---------------------------------------------------------------------------
// 4. Resets after window expires
// ---------------------------------------------------------------------------

describe('RateLimiter — window reset', () => {
  let limiter;

  afterEach(() => {
    if (limiter) limiter.destroy();
  });

  it('allows requests again after window expires (Date.now mock)', () => {
    const rpm = 2;
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: rpm } });

    // Exhaust the limit
    limiter.checkLimit('agent-1', 'tool_a');
    limiter.checkLimit('agent-1', 'tool_a');
    const denied = limiter.checkLimit('agent-1', 'tool_a');
    assert.equal(denied.allowed, false);

    // Advance time by 61 seconds (past the 60s window)
    const realNow = Date.now;
    Date.now = () => realNow() + 61_000;

    try {
      const afterWindow = limiter.checkLimit('agent-1', 'tool_a');
      assert.equal(afterWindow.allowed, true, 'should be allowed after window reset');
      assert.equal(afterWindow.remaining, rpm - 1);
    } finally {
      Date.now = realNow;
    }
  });

  it('starts a fresh window with full count after expiry', () => {
    const rpm = 3;
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: rpm } });

    // Exhaust all
    for (let i = 0; i < rpm; i++) {
      limiter.checkLimit('agent-1', 'tool_a');
    }

    // Advance past window
    const realNow = Date.now;
    Date.now = () => realNow() + 65_000;

    try {
      const fresh = limiter.checkLimit('agent-1', 'tool_a');
      assert.equal(fresh.allowed, true);
      assert.equal(fresh.remaining, rpm - 1);
      assert.equal(fresh.limit, rpm);
    } finally {
      Date.now = realNow;
    }
  });
});

// ---------------------------------------------------------------------------
// 5. Per-agent isolation
// ---------------------------------------------------------------------------

describe('RateLimiter — per-agent isolation', () => {
  let limiter;

  afterEach(() => {
    if (limiter) limiter.destroy();
  });

  it('does not share buckets between different agents', () => {
    const rpm = 2;
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: rpm } });

    // Exhaust agent-1
    limiter.checkLimit('agent-1', 'tool_a');
    limiter.checkLimit('agent-1', 'tool_a');
    const agent1Denied = limiter.checkLimit('agent-1', 'tool_a');
    assert.equal(agent1Denied.allowed, false);

    // agent-2 should still be able to make requests
    const agent2Result = limiter.checkLimit('agent-2', 'tool_a');
    assert.equal(agent2Result.allowed, true);
    assert.equal(agent2Result.remaining, rpm - 1);
  });

  it('does not share buckets between different tools for the same agent', () => {
    const rpm = 2;
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: rpm } });

    // Exhaust tool_a for agent-1
    limiter.checkLimit('agent-1', 'tool_a');
    limiter.checkLimit('agent-1', 'tool_a');
    assert.equal(limiter.checkLimit('agent-1', 'tool_a').allowed, false);

    // tool_b for agent-1 should still work
    const toolB = limiter.checkLimit('agent-1', 'tool_b');
    assert.equal(toolB.allowed, true);
    assert.equal(toolB.remaining, rpm - 1);
  });

  it('tracks multiple agents independently', () => {
    const rpm = 3;
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: rpm } });

    // Each agent uses 1 request
    limiter.checkLimit('agent-a', 'tool_x');
    limiter.checkLimit('agent-b', 'tool_x');
    limiter.checkLimit('agent-c', 'tool_x');

    // Each should have 2 remaining
    assert.equal(limiter.checkLimit('agent-a', 'tool_x').remaining, rpm - 2);
    assert.equal(limiter.checkLimit('agent-b', 'tool_x').remaining, rpm - 2);
    assert.equal(limiter.checkLimit('agent-c', 'tool_x').remaining, rpm - 2);
  });
});

// ---------------------------------------------------------------------------
// 6. Tool-specific overrides
// ---------------------------------------------------------------------------

describe('RateLimiter — tool-specific overrides', () => {
  let limiter;

  afterEach(() => {
    if (limiter) limiter.destroy();
  });

  it('applies lower limit for overridden tools', () => {
    limiter = makeLimiter({
      defaultLimits: { requestsPerMinute: 60 },
      toolOverrides: {
        a2a_pay: { requestsPerMinute: 3 },
      },
    });

    // a2a_pay should have limit=3
    const result = limiter.checkLimit('agent-1', 'a2a_pay');
    assert.equal(result.limit, 3);
    assert.equal(result.remaining, 2);
  });

  it('uses default limit for non-overridden tools', () => {
    limiter = makeLimiter({
      defaultLimits: { requestsPerMinute: 60 },
      toolOverrides: {
        a2a_pay: { requestsPerMinute: 3 },
      },
    });

    const result = limiter.checkLimit('agent-1', 'other_tool');
    assert.equal(result.limit, 60);
    assert.equal(result.remaining, 59);
  });

  it('blocks overridden tool at its specific limit', () => {
    limiter = makeLimiter({
      defaultLimits: { requestsPerMinute: 100 },
      toolOverrides: {
        x402_sign_intent: { requestsPerMinute: 2 },
      },
    });

    limiter.checkLimit('agent-1', 'x402_sign_intent');
    limiter.checkLimit('agent-1', 'x402_sign_intent');

    const denied = limiter.checkLimit('agent-1', 'x402_sign_intent');
    assert.equal(denied.allowed, false);
    assert.equal(denied.limit, 2);
  });

  it('override for one tool does not affect another', () => {
    limiter = makeLimiter({
      defaultLimits: { requestsPerMinute: 100 },
      toolOverrides: {
        a2a_pay: { requestsPerMinute: 1 },
      },
    });

    // Exhaust a2a_pay
    limiter.checkLimit('agent-1', 'a2a_pay');
    assert.equal(limiter.checkLimit('agent-1', 'a2a_pay').allowed, false);

    // other_tool should still work at default limit
    const other = limiter.checkLimit('agent-1', 'other_tool');
    assert.equal(other.allowed, true);
    assert.equal(other.limit, 100);
  });

  it('different overrides for different tools', () => {
    limiter = makeLimiter({
      defaultLimits: { requestsPerMinute: 50 },
      toolOverrides: {
        a2a_pay: { requestsPerMinute: 5 },
        x402_sign_intent: { requestsPerMinute: 10 },
      },
    });

    assert.equal(limiter.checkLimit('agent-1', 'a2a_pay').limit, 5);
    assert.equal(limiter.checkLimit('agent-1', 'x402_sign_intent').limit, 10);
    assert.equal(limiter.checkLimit('agent-1', 'list_orders').limit, 50);
  });
});

// ---------------------------------------------------------------------------
// 7. getHeaders()
// ---------------------------------------------------------------------------

describe('RateLimiter — getHeaders()', () => {
  let limiter;

  afterEach(() => {
    if (limiter) limiter.destroy();
  });

  it('returns correct headers before any requests', () => {
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: 30 } });
    const headers = limiter.getHeaders('agent-1', 'tool_a');

    assert.equal(headers['X-RateLimit-Limit'], '30');
    assert.equal(headers['X-RateLimit-Remaining'], '30');
    assert.ok(headers['X-RateLimit-Reset']);
    assert.ok(Number(headers['X-RateLimit-Reset']) > 0);
  });

  it('reflects decremented remaining after checkLimit calls', () => {
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: 10 } });

    limiter.checkLimit('agent-1', 'tool_a');
    limiter.checkLimit('agent-1', 'tool_a');
    limiter.checkLimit('agent-1', 'tool_a');

    const headers = limiter.getHeaders('agent-1', 'tool_a');
    assert.equal(headers['X-RateLimit-Limit'], '10');
    assert.equal(headers['X-RateLimit-Remaining'], '7');
  });

  it('returns 0 remaining when limit is exhausted', () => {
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: 2 } });
    limiter.checkLimit('agent-1', 'tool_a');
    limiter.checkLimit('agent-1', 'tool_a');

    const headers = limiter.getHeaders('agent-1', 'tool_a');
    assert.equal(headers['X-RateLimit-Remaining'], '0');
  });

  it('uses tool override limit in headers', () => {
    limiter = makeLimiter({
      defaultLimits: { requestsPerMinute: 60 },
      toolOverrides: { a2a_pay: { requestsPerMinute: 5 } },
    });

    const headers = limiter.getHeaders('agent-1', 'a2a_pay');
    assert.equal(headers['X-RateLimit-Limit'], '5');
    assert.equal(headers['X-RateLimit-Remaining'], '5');
  });

  it('all header values are strings', () => {
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: 10 } });
    limiter.checkLimit('agent-1', 'tool_a');

    const headers = limiter.getHeaders('agent-1', 'tool_a');
    for (const [key, value] of Object.entries(headers)) {
      assert.equal(typeof value, 'string', `${key} should be a string`);
    }
  });

  it('returns three standard rate-limit header keys', () => {
    limiter = makeLimiter();
    const headers = limiter.getHeaders('agent-1', 'tool_a');
    assert.ok('X-RateLimit-Limit' in headers);
    assert.ok('X-RateLimit-Remaining' in headers);
    assert.ok('X-RateLimit-Reset' in headers);
    assert.equal(Object.keys(headers).length, 3);
  });
});

// ---------------------------------------------------------------------------
// 8. getMetrics()
// ---------------------------------------------------------------------------

describe('RateLimiter — getMetrics()', () => {
  let limiter;

  afterEach(() => {
    if (limiter) limiter.destroy();
  });

  it('returns 0 activeBuckets and empty topAgents when no requests made', () => {
    limiter = makeLimiter();
    const metrics = limiter.getMetrics();
    assert.equal(metrics.activeBuckets, 0);
    assert.deepStrictEqual(metrics.topAgents, []);
  });

  it('shows active buckets after requests', () => {
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: 10 } });
    limiter.checkLimit('agent-1', 'tool_a');
    limiter.checkLimit('agent-1', 'tool_b');
    limiter.checkLimit('agent-2', 'tool_a');

    const metrics = limiter.getMetrics();
    assert.equal(metrics.activeBuckets, 3);
  });

  it('topAgents are sorted by total requests descending', () => {
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: 100 } });

    // agent-heavy: 5 requests across 2 tools
    for (let i = 0; i < 3; i++) limiter.checkLimit('agent-heavy', 'tool_a');
    for (let i = 0; i < 2; i++) limiter.checkLimit('agent-heavy', 'tool_b');

    // agent-light: 1 request
    limiter.checkLimit('agent-light', 'tool_a');

    const metrics = limiter.getMetrics();
    assert.equal(metrics.topAgents.length, 2);
    assert.equal(metrics.topAgents[0].agentId, 'agent-heavy');
    assert.equal(metrics.topAgents[0].totalRequests, 5);
    assert.equal(metrics.topAgents[1].agentId, 'agent-light');
    assert.equal(metrics.topAgents[1].totalRequests, 1);
  });

  it('topAgents is limited to 10 entries', () => {
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: 100 } });

    for (let i = 0; i < 15; i++) {
      limiter.checkLimit(`agent-${i}`, 'tool_a');
    }

    const metrics = limiter.getMetrics();
    assert.ok(metrics.topAgents.length <= 10, 'should cap at 10 agents');
  });

  it('correctly aggregates counts across tools for the same agent', () => {
    limiter = makeLimiter({ defaultLimits: { requestsPerMinute: 100 } });
    limiter.checkLimit('agent-1', 'tool_a');
    limiter.checkLimit('agent-1', 'tool_a');
    limiter.checkLimit('agent-1', 'tool_b');

    const metrics = limiter.getMetrics();
    const agent1 = metrics.topAgents.find((a) => a.agentId === 'agent-1');
    assert.ok(agent1);
    assert.equal(agent1.totalRequests, 3);
  });
});

// ---------------------------------------------------------------------------
// 9. destroy()
// ---------------------------------------------------------------------------

describe('RateLimiter — destroy()', () => {
  it('clears all buckets', () => {
    const limiter = makeLimiter({ defaultLimits: { requestsPerMinute: 10 } });
    limiter.checkLimit('agent-1', 'tool_a');
    limiter.checkLimit('agent-2', 'tool_b');

    assert.equal(limiter.getMetrics().activeBuckets, 2);

    limiter.destroy();

    assert.equal(limiter.getMetrics().activeBuckets, 0);
  });

  it('allows requests again after destroy (fresh buckets)', () => {
    const limiter = makeLimiter({ defaultLimits: { requestsPerMinute: 1 } });
    limiter.checkLimit('agent-1', 'tool_a');
    assert.equal(limiter.checkLimit('agent-1', 'tool_a').allowed, false);

    limiter.destroy();

    // After destroy, buckets are cleared — new request starts fresh
    const result = limiter.checkLimit('agent-1', 'tool_a');
    assert.equal(result.allowed, true);
  });

  it('can be called multiple times without error', () => {
    const limiter = makeLimiter();
    limiter.destroy();
    limiter.destroy();
    limiter.destroy();
    // No error is expected
  });
});
