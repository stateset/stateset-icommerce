/**
 * Resilience tests for x402/client.js
 *
 * Covers:
 *   - SequencerCircuitBreaker state machine (CLOSED / OPEN / HALF_OPEN)
 *   - X402SequencerClient retry logic with exponential backoff
 *   - Fallback sequencer URL routing
 *   - Offline payment queue when circuit is open
 *   - getCircuitStatus() reporting
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import {
  X402SequencerClient,
  SequencerCircuitBreaker,
  CircuitState,
} from '../../src/x402/client.js';

// ===========================================================================
// Helpers
// ===========================================================================

const originalFetch = globalThis.fetch;

function mockFetch(handler) {
  globalThis.fetch = async (...args) => handler(...args);
}

function restoreFetch() {
  globalThis.fetch = originalFetch;
}

function okResponse(body) {
  return {
    ok: true,
    status: 200,
    json: async () => body,
    text: async () => JSON.stringify(body),
  };
}

function errorResponse(status, text) {
  return {
    ok: false,
    status,
    json: async () => ({ error: text }),
    text: async () => text,
  };
}

/**
 * Build a minimal client config with short retry/circuit-breaker timings.
 * @param {object} [overrides]
 */
function fastConfig(overrides = {}) {
  return {
    sequencerUrl: 'https://seq.example.com',
    retryOptions: { maxRetries: 2, baseDelayMs: 5, maxDelayMs: 50 },
    circuitBreaker: { failureThreshold: 3, resetTimeoutMs: 50, halfOpenMax: 2 },
    ...overrides,
  };
}

// ===========================================================================
// SequencerCircuitBreaker
// ===========================================================================

describe('SequencerCircuitBreaker', () => {
  // ---- 1. Starts in CLOSED state, allows requests ----
  describe('initial state', () => {
    it('starts in CLOSED state', () => {
      const cb = new SequencerCircuitBreaker();
      assert.strictEqual(cb.getState(), CircuitState.CLOSED);
    });

    it('allows requests when CLOSED', () => {
      const cb = new SequencerCircuitBreaker();
      assert.strictEqual(cb.canRequest(), true);
    });

    it('starts with zero failures', () => {
      const cb = new SequencerCircuitBreaker();
      assert.strictEqual(cb.failures, 0);
    });

    it('starts with zero halfOpenSuccesses', () => {
      const cb = new SequencerCircuitBreaker();
      assert.strictEqual(cb.halfOpenSuccesses, 0);
    });

    it('uses default failureThreshold of 5', () => {
      const cb = new SequencerCircuitBreaker();
      assert.strictEqual(cb.failureThreshold, 5);
    });

    it('uses default resetTimeoutMs of 30000', () => {
      const cb = new SequencerCircuitBreaker();
      assert.strictEqual(cb.resetTimeoutMs, 30_000);
    });

    it('uses default halfOpenMax of 2', () => {
      const cb = new SequencerCircuitBreaker();
      assert.strictEqual(cb.halfOpenMax, 2);
    });

    it('accepts custom options', () => {
      const cb = new SequencerCircuitBreaker({
        failureThreshold: 10,
        resetTimeoutMs: 5000,
        halfOpenMax: 3,
      });
      assert.strictEqual(cb.failureThreshold, 10);
      assert.strictEqual(cb.resetTimeoutMs, 5000);
      assert.strictEqual(cb.halfOpenMax, 3);
    });
  });

  // ---- 2. Records successes, stays closed ----
  describe('success recording in CLOSED state', () => {
    it('stays CLOSED after recording a success', () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 3 });
      cb.recordSuccess();
      assert.strictEqual(cb.getState(), CircuitState.CLOSED);
    });

    it('resets failure count on success', () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 5 });
      cb.recordFailure();
      cb.recordFailure();
      assert.strictEqual(cb.failures, 2);
      cb.recordSuccess();
      assert.strictEqual(cb.failures, 0);
    });

    it('stays CLOSED after many successes', () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 3 });
      for (let i = 0; i < 100; i++) cb.recordSuccess();
      assert.strictEqual(cb.getState(), CircuitState.CLOSED);
      assert.strictEqual(cb.canRequest(), true);
    });
  });

  // ---- 3. Opens after failureThreshold consecutive failures ----
  describe('CLOSED -> OPEN transition', () => {
    it('stays CLOSED when failures < threshold', () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 3 });
      cb.recordFailure();
      cb.recordFailure();
      assert.strictEqual(cb.getState(), CircuitState.CLOSED);
      assert.strictEqual(cb.failures, 2);
    });

    it('opens after exactly failureThreshold failures', () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 3 });
      cb.recordFailure();
      cb.recordFailure();
      cb.recordFailure();
      assert.strictEqual(cb.getState(), CircuitState.OPEN);
    });

    it('opens after more than failureThreshold failures', () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 2 });
      cb.recordFailure();
      cb.recordFailure();
      cb.recordFailure();
      assert.strictEqual(cb.getState(), CircuitState.OPEN);
    });

    it('records lastFailureTime on failure', () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 3 });
      const before = Date.now();
      cb.recordFailure();
      const after = Date.now();
      assert.ok(cb.lastFailureTime >= before);
      assert.ok(cb.lastFailureTime <= after);
    });

    it('interleaved success resets counter, prevents opening', () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 3 });
      cb.recordFailure();
      cb.recordFailure();
      cb.recordSuccess(); // resets failures to 0
      cb.recordFailure();
      cb.recordFailure();
      assert.strictEqual(cb.getState(), CircuitState.CLOSED);
      assert.strictEqual(cb.failures, 2);
    });
  });

  // ---- 4. OPEN state blocks requests ----
  describe('OPEN state behavior', () => {
    it('blocks requests when OPEN', () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 1, resetTimeoutMs: 60_000 });
      cb.recordFailure();
      assert.strictEqual(cb.getState(), CircuitState.OPEN);
      assert.strictEqual(cb.canRequest(), false);
    });

    it('remains OPEN before resetTimeoutMs elapses', () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 1, resetTimeoutMs: 60_000 });
      cb.recordFailure();
      assert.strictEqual(cb.getState(), CircuitState.OPEN);
      // Call getState again -- still open
      assert.strictEqual(cb.getState(), CircuitState.OPEN);
    });
  });

  // ---- 5. Transitions OPEN -> HALF_OPEN after resetTimeoutMs ----
  describe('OPEN -> HALF_OPEN transition', () => {
    it('transitions to HALF_OPEN after resetTimeoutMs via getState()', async () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 1, resetTimeoutMs: 30 });
      cb.recordFailure();
      assert.strictEqual(cb.getState(), CircuitState.OPEN);

      await new Promise((r) => setTimeout(r, 50));

      assert.strictEqual(cb.getState(), CircuitState.HALF_OPEN);
    });

    it('transitions to HALF_OPEN after resetTimeoutMs via canRequest()', async () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 1, resetTimeoutMs: 30 });
      cb.recordFailure();
      assert.strictEqual(cb.canRequest(), false);

      await new Promise((r) => setTimeout(r, 50));

      assert.strictEqual(cb.canRequest(), true);
      assert.strictEqual(cb.state, CircuitState.HALF_OPEN);
    });

    it('resets halfOpenSuccesses to 0 on transition', async () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 1, resetTimeoutMs: 30, halfOpenMax: 2 });
      cb.recordFailure();
      // Manually set a stale value to verify it gets cleared
      cb.halfOpenSuccesses = 99;

      await new Promise((r) => setTimeout(r, 50));

      cb.canRequest(); // triggers the transition
      assert.strictEqual(cb.halfOpenSuccesses, 0);
    });
  });

  // ---- 6. HALF_OPEN allows limited requests ----
  describe('HALF_OPEN behavior', () => {
    it('allows requests when HALF_OPEN', async () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 1, resetTimeoutMs: 30 });
      cb.recordFailure();

      await new Promise((r) => setTimeout(r, 50));

      assert.strictEqual(cb.canRequest(), true);
      assert.strictEqual(cb.state, CircuitState.HALF_OPEN);
    });

    it('tracks half-open successes', async () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 1, resetTimeoutMs: 30, halfOpenMax: 3 });
      cb.recordFailure();

      await new Promise((r) => setTimeout(r, 50));

      cb.canRequest(); // transitions to HALF_OPEN
      cb.recordSuccess();
      assert.strictEqual(cb.halfOpenSuccesses, 1);
      assert.strictEqual(cb.state, CircuitState.HALF_OPEN);
    });
  });

  // ---- 7. HALF_OPEN -> CLOSED after halfOpenMax successes ----
  describe('HALF_OPEN -> CLOSED transition', () => {
    it('closes after halfOpenMax successes', async () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 1, resetTimeoutMs: 30, halfOpenMax: 2 });
      cb.recordFailure();

      await new Promise((r) => setTimeout(r, 50));

      cb.canRequest(); // transitions to HALF_OPEN
      cb.recordSuccess();
      assert.strictEqual(cb.state, CircuitState.HALF_OPEN);
      cb.recordSuccess();
      assert.strictEqual(cb.state, CircuitState.CLOSED);
    });

    it('resets failures to 0 on HALF_OPEN -> CLOSED', async () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 1, resetTimeoutMs: 30, halfOpenMax: 1 });
      cb.recordFailure();

      await new Promise((r) => setTimeout(r, 50));

      cb.canRequest(); // transitions to HALF_OPEN
      cb.recordSuccess();
      assert.strictEqual(cb.state, CircuitState.CLOSED);
      assert.strictEqual(cb.failures, 0);
    });

    it('allows normal requests after closing from HALF_OPEN', async () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 2, resetTimeoutMs: 30, halfOpenMax: 1 });
      cb.recordFailure();
      cb.recordFailure();
      assert.strictEqual(cb.state, CircuitState.OPEN);

      await new Promise((r) => setTimeout(r, 50));

      cb.canRequest(); // transitions to HALF_OPEN
      cb.recordSuccess(); // transitions to CLOSED
      assert.strictEqual(cb.state, CircuitState.CLOSED);
      assert.strictEqual(cb.canRequest(), true);

      // Can take a failure without immediately opening
      cb.recordFailure();
      assert.strictEqual(cb.state, CircuitState.CLOSED);
    });
  });

  // ---- 8. HALF_OPEN -> OPEN on any failure ----
  describe('HALF_OPEN -> OPEN on failure', () => {
    it('re-opens on a single failure in HALF_OPEN', async () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 3, resetTimeoutMs: 30, halfOpenMax: 2 });
      // Drive to OPEN
      cb.recordFailure();
      cb.recordFailure();
      cb.recordFailure();
      assert.strictEqual(cb.state, CircuitState.OPEN);

      await new Promise((r) => setTimeout(r, 50));

      cb.canRequest(); // transitions to HALF_OPEN
      assert.strictEqual(cb.state, CircuitState.HALF_OPEN);

      cb.recordFailure();
      assert.strictEqual(cb.state, CircuitState.OPEN);
    });

    it('re-opens even after partial half-open successes', async () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 2, resetTimeoutMs: 30, halfOpenMax: 3 });
      cb.recordFailure();
      cb.recordFailure();

      await new Promise((r) => setTimeout(r, 50));

      cb.canRequest(); // transitions to HALF_OPEN
      cb.recordSuccess();
      cb.recordSuccess();
      // Two successes out of three needed, then a failure
      cb.recordFailure();
      assert.strictEqual(cb.state, CircuitState.OPEN);
    });

    it('updates lastFailureTime when re-opening from HALF_OPEN', async () => {
      const cb = new SequencerCircuitBreaker({ failureThreshold: 1, resetTimeoutMs: 30 });
      cb.recordFailure();
      const firstFailureTime = cb.lastFailureTime;

      await new Promise((r) => setTimeout(r, 50));

      cb.canRequest(); // transitions to HALF_OPEN
      cb.recordFailure();
      assert.strictEqual(cb.state, CircuitState.OPEN);
      assert.ok(cb.lastFailureTime >= firstFailureTime);
    });
  });

  // ---- CircuitState enum values ----
  describe('CircuitState enum', () => {
    it('has expected string values', () => {
      assert.strictEqual(CircuitState.CLOSED, 'closed');
      assert.strictEqual(CircuitState.OPEN, 'open');
      assert.strictEqual(CircuitState.HALF_OPEN, 'half_open');
    });
  });
});

// ===========================================================================
// X402SequencerClient — resilience features
// ===========================================================================

describe('X402SequencerClient — resilience', () => {
  afterEach(() => restoreFetch());

  // ---- 9. Constructor validates config ----
  describe('constructor validation', () => {
    it('throws on missing URL', () => {
      assert.throws(() => new X402SequencerClient({}), /Sequencer URL is required/);
    });

    it('throws on empty string URL', () => {
      assert.throws(() => new X402SequencerClient(''), /Sequencer URL is required/);
    });

    it('throws on whitespace-only URL', () => {
      assert.throws(() => new X402SequencerClient('   '), /Sequencer URL is required/);
    });

    it('throws on null config', () => {
      assert.throws(() => new X402SequencerClient(null), /Sequencer URL is required/);
    });

    it('throws on undefined config', () => {
      assert.throws(() => new X402SequencerClient(undefined), /Sequencer URL is required/);
    });

    it('throws on unsupported protocol', () => {
      assert.throws(() => new X402SequencerClient('ftp://seq.example.com'), /Unsupported sequencer protocol/);
    });

    it('applies default retry options', () => {
      const client = new X402SequencerClient('https://seq.example.com');
      assert.strictEqual(client.maxRetries, 3);
      assert.strictEqual(client.baseDelayMs, 500);
      assert.strictEqual(client.maxDelayMs, 10_000);
    });

    it('applies custom retry options', () => {
      const client = new X402SequencerClient(fastConfig());
      assert.strictEqual(client.maxRetries, 2);
      assert.strictEqual(client.baseDelayMs, 5);
      assert.strictEqual(client.maxDelayMs, 50);
    });

    it('initializes circuit breaker with custom options', () => {
      const client = new X402SequencerClient(fastConfig());
      assert.strictEqual(client._circuitBreaker.failureThreshold, 3);
      assert.strictEqual(client._circuitBreaker.resetTimeoutMs, 50);
      assert.strictEqual(client._circuitBreaker.halfOpenMax, 2);
    });

    it('initializes empty offline queue', () => {
      const client = new X402SequencerClient(fastConfig());
      assert.strictEqual(client._offlineQueue.length, 0);
    });

    it('stores fallback URL when provided', () => {
      const client = new X402SequencerClient({
        ...fastConfig(),
        fallbackSequencerUrl: 'https://fallback.example.com',
      });
      assert.strictEqual(client.fallbackBaseUrl, 'https://fallback.example.com');
    });

    it('fallbackBaseUrl is null when no fallback provided', () => {
      const client = new X402SequencerClient(fastConfig());
      assert.strictEqual(client.fallbackBaseUrl, null);
    });

    it('string config has no fallback URL', () => {
      const client = new X402SequencerClient('https://seq.example.com');
      assert.strictEqual(client.fallbackBaseUrl, null);
    });
  });

  // ---- 10. getCircuitStatus() returns correct state ----
  describe('getCircuitStatus()', () => {
    it('returns CLOSED state initially', () => {
      const client = new X402SequencerClient(fastConfig());
      const status = client.getCircuitStatus();
      assert.strictEqual(status.state, 'closed');
      assert.strictEqual(status.failures, 0);
      assert.strictEqual(status.queueDepth, 0);
    });

    it('reflects failure count', () => {
      const client = new X402SequencerClient(fastConfig());
      client._circuitBreaker.recordFailure();
      client._circuitBreaker.recordFailure();
      const status = client.getCircuitStatus();
      assert.strictEqual(status.failures, 2);
      assert.strictEqual(status.state, 'closed');
    });

    it('reflects OPEN state after threshold failures', () => {
      const client = new X402SequencerClient(fastConfig());
      for (let i = 0; i < 3; i++) client._circuitBreaker.recordFailure();
      const status = client.getCircuitStatus();
      assert.strictEqual(status.state, 'open');
      assert.strictEqual(status.failures, 3);
    });

    it('reflects queue depth', () => {
      const client = new X402SequencerClient(fastConfig());
      // Manually push to the offline queue for test
      client._offlineQueue.push({ payload: {}, resolve: () => {}, reject: () => {} });
      client._offlineQueue.push({ payload: {}, resolve: () => {}, reject: () => {} });
      const status = client.getCircuitStatus();
      assert.strictEqual(status.queueDepth, 2);
    });

    it('detects HALF_OPEN transition via getState()', async () => {
      const client = new X402SequencerClient(fastConfig());
      for (let i = 0; i < 3; i++) client._circuitBreaker.recordFailure();
      assert.strictEqual(client.getCircuitStatus().state, 'open');

      await new Promise((r) => setTimeout(r, 80));

      assert.strictEqual(client.getCircuitStatus().state, 'half_open');
    });
  });

  // ---- 11. submitPaymentIntent queues when circuit is open ----
  describe('submitPaymentIntent — offline queue', () => {
    it('queues payment when circuit is OPEN', async () => {
      const warnings = [];
      const origWarn = console.warn;
      console.warn = (...args) => warnings.push(args.join(' '));

      try {
        const client = new X402SequencerClient(fastConfig({
          circuitBreaker: { failureThreshold: 1, resetTimeoutMs: 60_000, halfOpenMax: 2 },
        }));

        // Force circuit open
        client._circuitBreaker.recordFailure();
        assert.strictEqual(client._circuitBreaker.getState(), CircuitState.OPEN);

        // Submit should queue, not throw
        const promise = client.submitPaymentIntent({ amount: 100 });

        // Verify it returned a promise (pending — not resolved)
        assert.ok(promise instanceof Promise);

        // Verify queue depth increased
        assert.strictEqual(client._offlineQueue.length, 1);
        assert.strictEqual(client.getCircuitStatus().queueDepth, 1);

        // Verify warning was logged
        assert.ok(warnings.some((w) => w.includes('circuit OPEN') && w.includes('queued')));
      } finally {
        console.warn = origWarn;
      }
    });

    it('queues multiple payments and reports correct depth', async () => {
      const origWarn = console.warn;
      console.warn = () => {};

      try {
        const client = new X402SequencerClient(fastConfig({
          circuitBreaker: { failureThreshold: 1, resetTimeoutMs: 60_000, halfOpenMax: 2 },
        }));
        client._circuitBreaker.recordFailure();

        client.submitPaymentIntent({ amount: 10 });
        client.submitPaymentIntent({ amount: 20 });
        client.submitPaymentIntent({ amount: 30 });

        assert.strictEqual(client.getCircuitStatus().queueDepth, 3);
      } finally {
        console.warn = origWarn;
      }
    });

    it('queued payment stores the correct payload', async () => {
      const origWarn = console.warn;
      console.warn = () => {};

      try {
        const client = new X402SequencerClient(fastConfig({
          circuitBreaker: { failureThreshold: 1, resetTimeoutMs: 60_000, halfOpenMax: 2 },
        }));
        client._circuitBreaker.recordFailure();

        const payload = { amount: 42, currency: 'USDC' };
        client.submitPaymentIntent(payload);

        assert.deepStrictEqual(client._offlineQueue[0].payload, payload);
      } finally {
        console.warn = origWarn;
      }
    });

    it('calls POST when circuit is CLOSED', async () => {
      let fetchCalled = false;
      mockFetch(() => {
        fetchCalled = true;
        return okResponse({ intent_id: 'INT-1' });
      });

      const client = new X402SequencerClient(fastConfig());
      const result = await client.submitPaymentIntent({ amount: 100 });

      assert.ok(fetchCalled);
      assert.strictEqual(result.intent_id, 'INT-1');
      assert.strictEqual(client._offlineQueue.length, 0);
    });
  });

  // ---- 12. Retry logic with exponential backoff ----
  describe('retry logic', () => {
    it('retries on failure and succeeds eventually', async () => {
      let callCount = 0;
      mockFetch(() => {
        callCount++;
        if (callCount <= 2) {
          throw new Error('network error');
        }
        return okResponse({ result: 'ok' });
      });

      const client = new X402SequencerClient(fastConfig());
      const result = await client._request('GET', '/test');

      assert.deepStrictEqual(result, { result: 'ok' });
      assert.strictEqual(callCount, 3); // 2 failures + 1 success
    });

    it('exhausts all retries and throws last error', async () => {
      let callCount = 0;
      mockFetch(() => {
        callCount++;
        throw new Error(`failure ${callCount}`);
      });

      const client = new X402SequencerClient(fastConfig({ retryOptions: { maxRetries: 2, baseDelayMs: 1, maxDelayMs: 10 } }));
      await assert.rejects(
        () => client._request('GET', '/test'),
        /failure/,
      );
      // maxRetries=2: initial attempt (0) + 2 retries = 3 total attempts
      assert.strictEqual(callCount, 3);
    });

    it('records failure in circuit breaker after all retries exhausted', async () => {
      mockFetch(() => {
        throw new Error('total failure');
      });

      const client = new X402SequencerClient(fastConfig({ retryOptions: { maxRetries: 1, baseDelayMs: 1, maxDelayMs: 5 } }));
      await assert.rejects(() => client._request('GET', '/test'));
      assert.strictEqual(client._circuitBreaker.failures, 1);
    });

    it('records success in circuit breaker after successful request', async () => {
      mockFetch(() => okResponse({ ok: true }));

      const client = new X402SequencerClient(fastConfig());
      // Record some failures first
      client._circuitBreaker.recordFailure();
      client._circuitBreaker.recordFailure();
      assert.strictEqual(client._circuitBreaker.failures, 2);

      await client._request('GET', '/test');
      // Success resets failures
      assert.strictEqual(client._circuitBreaker.failures, 0);
    });

    it('applies exponential backoff delays between retries', async () => {
      let callCount = 0;
      const callTimes = [];
      mockFetch(() => {
        callCount++;
        callTimes.push(Date.now());
        if (callCount <= 2) {
          throw new Error('fail');
        }
        return okResponse({ ok: true });
      });

      const client = new X402SequencerClient(fastConfig({
        retryOptions: { maxRetries: 3, baseDelayMs: 20, maxDelayMs: 500 },
      }));
      await client._request('GET', '/test');

      assert.strictEqual(callTimes.length, 3);
      // First retry delay: baseDelayMs * 2^0 = 20ms
      // Second retry delay: baseDelayMs * 2^1 = 40ms
      // Allow some slack for timer imprecision
      if (callTimes.length >= 2) {
        const firstGap = callTimes[1] - callTimes[0];
        assert.ok(firstGap >= 10, `First retry gap ${firstGap}ms too short (expected ~20ms)`);
      }
      if (callTimes.length >= 3) {
        const secondGap = callTimes[2] - callTimes[1];
        assert.ok(secondGap >= 20, `Second retry gap ${secondGap}ms too short (expected ~40ms)`);
      }
    });

    it('caps delay at maxDelayMs', async () => {
      let callCount = 0;
      const callTimes = [];
      mockFetch(() => {
        callCount++;
        callTimes.push(Date.now());
        if (callCount <= 4) {
          throw new Error('fail');
        }
        return okResponse({ ok: true });
      });

      const client = new X402SequencerClient(fastConfig({
        retryOptions: { maxRetries: 5, baseDelayMs: 10, maxDelayMs: 30 },
      }));
      await client._request('GET', '/test');

      // With baseDelayMs=10 and maxDelayMs=30:
      // Attempt 0: delay = min(10*2^0, 30) = 10ms
      // Attempt 1: delay = min(10*2^1, 30) = 20ms
      // Attempt 2: delay = min(10*2^2, 30) = 30ms
      // Attempt 3: delay = min(10*2^3, 30) = 30ms (capped)
      // Verify last gaps are not excessively long (capped at maxDelayMs + slack)
      if (callTimes.length >= 4) {
        const lastGap = callTimes[callTimes.length - 1] - callTimes[callTimes.length - 2];
        assert.ok(lastGap < 80, `Last gap ${lastGap}ms suggests delay was not capped`);
      }
    });

    it('retries on HTTP error responses (non-ok)', async () => {
      let callCount = 0;
      mockFetch(() => {
        callCount++;
        if (callCount <= 1) {
          return errorResponse(503, 'Service Unavailable');
        }
        return okResponse({ result: 'recovered' });
      });

      const client = new X402SequencerClient(fastConfig());
      const result = await client._request('GET', '/test');
      assert.deepStrictEqual(result, { result: 'recovered' });
      assert.strictEqual(callCount, 2);
    });

    it('blocks requests when circuit breaker is OPEN', async () => {
      const client = new X402SequencerClient(fastConfig({
        circuitBreaker: { failureThreshold: 1, resetTimeoutMs: 60_000, halfOpenMax: 2 },
      }));
      client._circuitBreaker.recordFailure();

      await assert.rejects(
        () => client._request('GET', '/test'),
        /circuit breaker is OPEN/,
      );
    });
  });

  // ---- 13. Fallback URL used on retry ----
  describe('fallback sequencer URL', () => {
    it('tries only primary URL on first attempt', async () => {
      const capturedUrls = [];
      let callCount = 0;
      mockFetch((url) => {
        capturedUrls.push(url);
        callCount++;
        return okResponse({ ok: true });
      });

      const client = new X402SequencerClient({
        ...fastConfig(),
        fallbackSequencerUrl: 'https://fallback.example.com',
      });
      await client._request('GET', '/api/test');

      // Only primary URL tried on first attempt (no retry needed)
      assert.strictEqual(capturedUrls.length, 1);
      assert.ok(capturedUrls[0].startsWith('https://seq.example.com'));
    });

    it('tries both primary and fallback URLs on retry attempts', async () => {
      const capturedUrls = [];
      let callCount = 0;
      mockFetch((url) => {
        capturedUrls.push(url);
        callCount++;
        if (callCount <= 2) {
          throw new Error('fail');
        }
        return okResponse({ ok: true });
      });

      const client = new X402SequencerClient({
        ...fastConfig(),
        fallbackSequencerUrl: 'https://fallback.example.com',
      });
      await client._request('GET', '/api/test');

      // Attempt 0: primary only (fails)
      // Attempt 1: primary + fallback (primary fails, fallback succeeds)
      assert.ok(capturedUrls.some((u) => u.startsWith('https://seq.example.com')));
      assert.ok(capturedUrls.some((u) => u.startsWith('https://fallback.example.com')));
    });

    it('succeeds via fallback when primary always fails', async () => {
      mockFetch((url) => {
        if (url.includes('seq.example.com')) {
          throw new Error('primary down');
        }
        return okResponse({ source: 'fallback' });
      });

      const client = new X402SequencerClient({
        ...fastConfig(),
        fallbackSequencerUrl: 'https://fallback.example.com',
      });
      const result = await client._request('GET', '/api/test');
      assert.strictEqual(result.source, 'fallback');
    });

    it('does not try fallback when no fallback URL configured', async () => {
      const capturedUrls = [];
      mockFetch((url) => {
        capturedUrls.push(url);
        throw new Error('fail');
      });

      const client = new X402SequencerClient(fastConfig({ retryOptions: { maxRetries: 1, baseDelayMs: 1, maxDelayMs: 5 } }));
      await assert.rejects(() => client._request('GET', '/test'));

      // All URLs should be to the primary
      const unique = [...new Set(capturedUrls)];
      assert.strictEqual(unique.length, 1);
      assert.ok(unique[0].startsWith('https://seq.example.com'));
    });

    it('preserves path when using fallback URL', async () => {
      const capturedUrls = [];
      mockFetch((url) => {
        capturedUrls.push(url);
        if (url.includes('fallback.example.com')) {
          return okResponse({ ok: true });
        }
        throw new Error('primary down');
      });

      const client = new X402SequencerClient({
        ...fastConfig(),
        fallbackSequencerUrl: 'https://fallback.example.com',
      });
      await client._request('POST', '/api/v1/x402/payments', { amount: 50 });

      const fallbackUrl = capturedUrls.find((u) => u.includes('fallback.example.com'));
      assert.ok(fallbackUrl);
      assert.ok(fallbackUrl.endsWith('/api/v1/x402/payments'));
    });
  });

  // ---- Offline queue flush on recovery ----
  describe('offline queue flush', () => {
    it('flushes queued payments when a subsequent request succeeds', async () => {
      const origWarn = console.warn;
      console.warn = () => {};

      try {
        const client = new X402SequencerClient(fastConfig({
          circuitBreaker: { failureThreshold: 1, resetTimeoutMs: 30, halfOpenMax: 1 },
        }));

        // Force circuit open
        client._circuitBreaker.recordFailure();
        assert.strictEqual(client._circuitBreaker.getState(), CircuitState.OPEN);

        // Queue a payment
        const queuedPromise = client.submitPaymentIntent({ amount: 99 });
        assert.strictEqual(client._offlineQueue.length, 1);

        // Wait for circuit to transition to HALF_OPEN
        await new Promise((r) => setTimeout(r, 50));

        // Mock fetch to succeed now
        mockFetch(() => okResponse({ intent_id: 'INT-FLUSHED' }));

        // Make a direct request that succeeds (triggers flush)
        const directResult = await client._request('GET', '/api/v1/health');
        assert.ok(directResult);

        // The queued promise should have resolved
        const queuedResult = await queuedPromise;
        assert.strictEqual(queuedResult.intent_id, 'INT-FLUSHED');
        assert.strictEqual(client._offlineQueue.length, 0);
      } finally {
        console.warn = origWarn;
        restoreFetch();
      }
    });
  });

  // ---- Full lifecycle: closed -> open -> half-open -> closed ----
  describe('full circuit breaker lifecycle through client', () => {
    it('transitions through all states during failures and recovery', async () => {
      let callCount = 0;
      mockFetch(() => {
        callCount++;
        throw new Error(`fail ${callCount}`);
      });

      const client = new X402SequencerClient(fastConfig({
        retryOptions: { maxRetries: 0, baseDelayMs: 1, maxDelayMs: 5 },
        circuitBreaker: { failureThreshold: 2, resetTimeoutMs: 30, halfOpenMax: 1 },
      }));

      // State: CLOSED
      assert.strictEqual(client.getCircuitStatus().state, 'closed');

      // First failure
      await assert.rejects(() => client._request('GET', '/test'));
      assert.strictEqual(client.getCircuitStatus().state, 'closed');
      assert.strictEqual(client.getCircuitStatus().failures, 1);

      // Second failure -> OPEN
      await assert.rejects(() => client._request('GET', '/test'));
      assert.strictEqual(client.getCircuitStatus().state, 'open');

      // Requests blocked while OPEN
      await assert.rejects(() => client._request('GET', '/test'), /circuit breaker is OPEN/);

      // Wait for reset timeout
      await new Promise((r) => setTimeout(r, 50));

      // Should be HALF_OPEN now
      assert.strictEqual(client.getCircuitStatus().state, 'half_open');

      // Mock fetch to succeed
      restoreFetch();
      mockFetch(() => okResponse({ recovered: true }));

      // Successful request in HALF_OPEN -> CLOSED
      const result = await client._request('GET', '/test');
      assert.deepStrictEqual(result, { recovered: true });
      assert.strictEqual(client.getCircuitStatus().state, 'closed');
      assert.strictEqual(client.getCircuitStatus().failures, 0);
    });
  });
});
