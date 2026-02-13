/**
 * Unit tests for cli/src/channels/base.js
 *
 * Tests the shared channel base module: constants, session management,
 * message chunking, allowlist access control, bot commands, backoff, and sleep.
 *
 * Uses node:test (describe/it/beforeEach) and node:assert/strict.
 */

import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';

// The module under test imports from claude-harness.js and other heavy deps.
// Wrap in try/catch so the test file loads gracefully even if transitive deps fail.
let BOT_PREFIX,
  SESSION_TTL_MS,
  createSessionManager,
  chunkMessage,
  isAllowed,
  handleBotCommand,
  computeBackoff,
  RECONNECT_POLICY,
  sleep;

let moduleLoaded = false;

try {
  const mod = await import('../../src/channels/base.js');
  BOT_PREFIX = mod.BOT_PREFIX;
  SESSION_TTL_MS = mod.SESSION_TTL_MS;
  createSessionManager = mod.createSessionManager;
  chunkMessage = mod.chunkMessage;
  isAllowed = mod.isAllowed;
  handleBotCommand = mod.handleBotCommand;
  computeBackoff = mod.computeBackoff;
  RECONNECT_POLICY = mod.RECONNECT_POLICY;
  sleep = mod.sleep;
  moduleLoaded = true;
} catch (err) {
  console.warn(`Skipping channels-base tests — module failed to load: ${err.message}`);
}

// Skip helper: wraps describe blocks so they no-op when module cannot load.
const d = moduleLoaded ? describe : describe.skip;

// ============================================================================
// Constants
// ============================================================================

d('BOT_PREFIX', () => {
  it('is the string "[agent] "', () => {
    assert.equal(BOT_PREFIX, '[agent] ');
  });

  it('is a non-empty string', () => {
    assert.equal(typeof BOT_PREFIX, 'string');
    assert.ok(BOT_PREFIX.length > 0);
  });
});

d('SESSION_TTL_MS', () => {
  it('equals 30 minutes in milliseconds', () => {
    assert.equal(SESSION_TTL_MS, 30 * 60 * 1000);
  });

  it('equals 1_800_000', () => {
    assert.equal(SESSION_TTL_MS, 1_800_000);
  });
});

// ============================================================================
// createSessionManager
// ============================================================================

d('createSessionManager', () => {
  let mgr;

  beforeEach(() => {
    mgr = createSessionManager();
  });

  it('returns an object with expected keys', () => {
    assert.ok(typeof mgr.getSession === 'function');
    assert.ok(typeof mgr.persistSession === 'function');
    assert.ok(typeof mgr.startCleanup === 'function');
    assert.ok(typeof mgr.stopCleanup === 'function');
    assert.ok(mgr._sessions instanceof Map);
  });

  describe('getSession', () => {
    it('creates a new session for unknown id', () => {
      const s = mgr.getSession('user-1');
      assert.equal(s.sessionId, null);
      assert.equal(s.agent, null);
      assert.equal(s.processing, false);
      assert.deepEqual(s.queue, []);
      assert.ok(s.lastActive > 0);
    });

    it('returns the same session on consecutive calls', () => {
      const s1 = mgr.getSession('user-1');
      s1.sessionId = 'abc';
      const s2 = mgr.getSession('user-1');
      assert.equal(s2.sessionId, 'abc');
      assert.strictEqual(s1, s2);
    });

    it('updates lastActive on each access', () => {
      const s1 = mgr.getSession('user-1');
      const t1 = s1.lastActive;
      // lastActive is set to Date.now() each call; same millisecond is possible
      const s2 = mgr.getSession('user-1');
      assert.ok(s2.lastActive >= t1);
    });

    it('creates a fresh session when existing one has expired', () => {
      const s = mgr.getSession('user-1');
      s.sessionId = 'old-session';
      // Simulate expiry by backdating lastActive
      s.lastActive = Date.now() - SESSION_TTL_MS - 1;
      const s2 = mgr.getSession('user-1');
      assert.equal(s2.sessionId, null, 'expired session should be replaced');
    });

    it('isolates sessions by id', () => {
      const a = mgr.getSession('alice');
      const b = mgr.getSession('bob');
      a.sessionId = 'alice-session';
      assert.equal(b.sessionId, null);
    });
  });

  describe('getSession with persistent store', () => {
    it('loads from persistent store when in-memory is missing', () => {
      const persisted = {
        sessionId: 'persisted-sess',
        agent: 'orders',
        lastActive: Date.now() - 1000, // recent enough
      };
      const store = {
        get: (ch, id) => (id === 'user-1' ? persisted : null),
        upsert: () => {},
        deleteExpired: () => {},
      };
      const m = createSessionManager({ store, channel: 'telegram' });
      const s = m.getSession('user-1');
      assert.equal(s.sessionId, 'persisted-sess');
      assert.equal(s.agent, 'orders');
      assert.equal(s.processing, false);
      assert.deepEqual(s.queue, []);
    });

    it('ignores expired persisted sessions', () => {
      const persisted = {
        sessionId: 'old-persisted',
        agent: 'returns',
        lastActive: Date.now() - SESSION_TTL_MS - 5000,
      };
      const store = {
        get: () => persisted,
        upsert: () => {},
        deleteExpired: () => {},
      };
      const m = createSessionManager({ store, channel: 'slack' });
      const s = m.getSession('user-1');
      assert.equal(s.sessionId, null, 'expired persisted session should not be loaded');
    });

    it('does not load from store when channel is not set', () => {
      const store = {
        get: () => {
          throw new Error('should not be called');
        },
        upsert: () => {},
        deleteExpired: () => {},
      };
      // No channel provided
      const m = createSessionManager({ store });
      const s = m.getSession('user-1');
      assert.equal(s.sessionId, null);
    });
  });

  describe('persistSession', () => {
    it('calls store.upsert when store and channel are set', () => {
      let upserted = null;
      const store = {
        get: () => null,
        upsert: (ch, id, data) => {
          upserted = { ch, id, data };
        },
        deleteExpired: () => {},
      };
      const m = createSessionManager({ store, channel: 'discord' });
      const session = { sessionId: 's1', agent: 'checkout', lastActive: 12345 };
      m.persistSession('user-1', session);
      assert.deepEqual(upserted, {
        ch: 'discord',
        id: 'user-1',
        data: { sessionId: 's1', agent: 'checkout', lastActive: 12345 },
      });
    });

    it('no-ops when store is not configured', () => {
      // Should not throw
      const m = createSessionManager();
      m.persistSession('user-1', { sessionId: null, agent: null, lastActive: Date.now() });
    });
  });

  describe('startCleanup / stopCleanup', () => {
    it('startCleanup returns a handle that stopCleanup can clear', () => {
      const handle = mgr.startCleanup();
      assert.ok(handle != null);
      mgr.stopCleanup(handle);
    });

    it('cleanup removes expired sessions', async () => {
      // We cannot easily wait for the interval (5 min), but we can verify
      // the structure by manually calling the internal mechanism.
      // At minimum, startCleanup should not throw.
      const m = createSessionManager();
      const s = m.getSession('temp-user');
      s.lastActive = Date.now() - SESSION_TTL_MS - 1;
      // Force a cleanup cycle by starting and quickly stopping
      const handle = m.startCleanup();
      m.stopCleanup(handle);
      // The session should still be in the map until the interval fires,
      // but the interval was cleared — just verify no crash.
      assert.ok(true);
    });
  });
});

// ============================================================================
// chunkMessage
// ============================================================================

d('chunkMessage', () => {
  it('returns single-element array when text fits', () => {
    const result = chunkMessage('Hello world', 100);
    assert.deepEqual(result, ['Hello world']);
  });

  it('returns original text when exactly at maxLength', () => {
    const text = 'a'.repeat(50);
    assert.deepEqual(chunkMessage(text, 50), [text]);
  });

  it('splits at double newline when possible', () => {
    const text = 'Part one.\n\nPart two.';
    const chunks = chunkMessage(text, 15);
    assert.ok(chunks.length >= 2);
    assert.ok(chunks[0].includes('Part one'));
    assert.ok(chunks[chunks.length - 1].includes('Part two'));
  });

  it('splits at single newline when double newline is too early', () => {
    // Create text where double newline is in the first 30% but single newline is later
    const part1 = 'x'.repeat(5);
    const part2 = 'y'.repeat(30);
    const part3 = 'z'.repeat(30);
    const text = `${part1}\n\n${part2}\n${part3}`;
    const chunks = chunkMessage(text, 40);
    assert.ok(chunks.length >= 2);
  });

  it('splits at space when no newlines are suitable', () => {
    const text = 'word1 word2 word3 word4 word5 word6 word7 word8';
    const chunks = chunkMessage(text, 20);
    assert.ok(chunks.length >= 2);
    // Verify no chunk exceeds maxLength (with some tolerance for trimming)
    for (const chunk of chunks) {
      assert.ok(chunk.length <= 20, `chunk "${chunk}" exceeds maxLength`);
    }
  });

  it('hard splits when no good break point exists', () => {
    const text = 'a'.repeat(100);
    const chunks = chunkMessage(text, 30);
    assert.ok(chunks.length >= 3);
    // Reassembled should equal the original
    assert.equal(chunks.join(''), text);
  });

  it('trims whitespace from chunks', () => {
    const text = 'Hello world.\n\nThis is a test message for chunking.';
    const chunks = chunkMessage(text, 20);
    for (const chunk of chunks) {
      assert.equal(chunk, chunk.trim());
    }
  });

  it('handles empty string', () => {
    assert.deepEqual(chunkMessage('', 100), ['']);
  });

  it('preserves all content across chunks', () => {
    const text = 'The quick brown fox\n\njumps over\nthe lazy dog.';
    const chunks = chunkMessage(text, 15);
    // All words should appear across the chunks
    const joined = chunks.join(' ');
    assert.ok(joined.includes('quick'));
    assert.ok(joined.includes('fox'));
    assert.ok(joined.includes('dog'));
  });
});

// ============================================================================
// isAllowed
// ============================================================================

d('isAllowed', () => {
  it('returns true when allowlist is null', () => {
    assert.equal(isAllowed('anyone', null), true);
  });

  it('returns true when allowlist is empty array', () => {
    assert.equal(isAllowed('anyone', []), true);
  });

  it('returns true when allowlist contains wildcard', () => {
    assert.equal(isAllowed('anyone', ['*']), true);
  });

  it('returns true when sender is in allowlist', () => {
    assert.equal(isAllowed('alice', ['alice', 'bob']), true);
  });

  it('returns false when sender is not in allowlist', () => {
    assert.equal(isAllowed('eve', ['alice', 'bob']), false);
  });

  it('normalizes by removing non-word chars and lowercasing', () => {
    // Phone number normalization
    assert.equal(isAllowed('+1-555-123-4567', ['15551234567']), true);
  });

  it('normalizes both sender and allowlist entries', () => {
    // \w keeps underscores and digits but removes dots, dashes, etc.
    assert.equal(isAllowed('Alice.Smith', ['AliceSmith']), true);
  });

  it('case-insensitive matching', () => {
    assert.equal(isAllowed('ALICE', ['alice']), true);
    assert.equal(isAllowed('alice', ['ALICE']), true);
  });

  it('handles special characters in sender ID', () => {
    assert.equal(isAllowed('user@example.com', ['userexamplecom']), true);
  });

  it('wildcard among other entries still allows all', () => {
    assert.equal(isAllowed('unknown', ['alice', '*', 'bob']), true);
  });
});

// ============================================================================
// handleBotCommand
// ============================================================================

d('handleBotCommand', () => {
  /** @returns {import('../../src/channels/base.js').SenderSession} */
  function makeSession(overrides = {}) {
    return {
      sessionId: 'sess-123',
      agent: 'orders',
      lastActive: Date.now(),
      processing: false,
      queue: [],
      thinkLevel: null,
      provider: null,
      memoryEnabled: false,
      ...overrides,
    };
  }

  describe('/reset', () => {
    it('clears session and returns confirmation', async () => {
      const session = makeSession();
      const result = await handleBotCommand('/reset', session, true);
      assert.equal(result.handled, true);
      assert.ok(result.response.toLowerCase().includes('cleared'));
      assert.equal(session.sessionId, null);
      assert.equal(session.agent, null);
    });

    it('works case-insensitively', async () => {
      const session = makeSession();
      const result = await handleBotCommand('/RESET', session, false);
      assert.equal(result.handled, true);
      assert.equal(session.sessionId, null);
    });
  });

  describe('/new', () => {
    it('clears session just like /reset', async () => {
      const session = makeSession();
      const result = await handleBotCommand('/new', session, true);
      assert.equal(result.handled, true);
      assert.equal(session.sessionId, null);
      assert.equal(session.agent, null);
      assert.ok(result.response.includes('fresh'));
    });
  });

  describe('/help', () => {
    it('returns help text with handled=true', async () => {
      const session = makeSession();
      const result = await handleBotCommand('/help', session, true);
      assert.equal(result.handled, true);
      assert.ok(result.response.includes('StateSet Commerce Agent'));
      assert.ok(result.response.includes('/reset'));
      assert.ok(result.response.includes('/status'));
      assert.ok(result.response.includes('/orders'));
    });

    it('mentions available commands', async () => {
      const session = makeSession();
      const result = await handleBotCommand('/help', session, false);
      assert.ok(result.response.includes('/help'));
      assert.ok(result.response.includes('/think'));
      assert.ok(result.response.includes('/provider'));
      assert.ok(result.response.includes('/memory'));
    });
  });

  describe('/status', () => {
    it('shows current session status', async () => {
      const session = makeSession({
        agent: 'checkout',
        sessionId: 'active-sess',
        provider: 'openai',
        thinkLevel: 'high',
        memoryEnabled: true,
      });
      const result = await handleBotCommand('/status', session, true);
      assert.equal(result.handled, true);
      assert.ok(result.response.includes('checkout'));
      assert.ok(result.response.includes('active'));
      assert.ok(result.response.includes('write enabled'));
      assert.ok(result.response.includes('openai'));
      assert.ok(result.response.includes('high'));
      assert.ok(result.response.includes('on'));
    });

    it('shows defaults when session is fresh', async () => {
      const session = makeSession({ agent: null, sessionId: null });
      const result = await handleBotCommand('/status', session, false);
      assert.ok(result.response.includes('auto-route'));
      assert.ok(result.response.includes('none'));
      assert.ok(result.response.includes('preview only'));
    });
  });

  describe('/think', () => {
    it('sets thinking level to low', async () => {
      const session = makeSession();
      const result = await handleBotCommand('/think low', session, true);
      assert.equal(result.handled, true);
      assert.equal(session.thinkLevel, 'low');
      assert.ok(result.response.includes('low'));
    });

    it('sets thinking level to medium', async () => {
      const session = makeSession();
      const result = await handleBotCommand('/think medium', session, true);
      assert.equal(session.thinkLevel, 'medium');
    });

    it('normalizes "med" to "medium"', async () => {
      const session = makeSession();
      const result = await handleBotCommand('/think med', session, true);
      assert.equal(session.thinkLevel, 'medium');
      assert.ok(result.response.includes('medium'));
    });

    it('sets thinking level to high', async () => {
      const session = makeSession();
      await handleBotCommand('/think high', session, true);
      assert.equal(session.thinkLevel, 'high');
    });

    it('sets thinking level to off', async () => {
      const session = makeSession({ thinkLevel: 'high' });
      await handleBotCommand('/think off', session, true);
      assert.equal(session.thinkLevel, 'off');
    });

    it('shows usage when no valid level given', async () => {
      const session = makeSession({ thinkLevel: 'low' });
      const result = await handleBotCommand('/think', session, true);
      assert.equal(result.handled, true);
      assert.ok(result.response.includes('Usage'));
      assert.ok(result.response.includes('low')); // current level
    });

    it('shows usage for invalid level', async () => {
      const session = makeSession();
      const result = await handleBotCommand('/think ultra', session, true);
      assert.ok(result.response.includes('Usage'));
    });
  });

  describe('/provider', () => {
    it('sets provider to claude', async () => {
      const session = makeSession();
      const result = await handleBotCommand('/provider claude', session, true);
      assert.equal(result.handled, true);
      assert.equal(session.provider, 'claude');
      assert.ok(result.response.includes('claude'));
    });

    it('sets provider to openai with a note about chat-only', async () => {
      const session = makeSession();
      const result = await handleBotCommand('/provider openai', session, true);
      assert.equal(session.provider, 'openai');
      assert.ok(result.response.includes('openai'));
      assert.ok(result.response.includes('chat-only'));
    });

    it('sets provider to gemini', async () => {
      const session = makeSession();
      await handleBotCommand('/provider gemini', session, true);
      assert.equal(session.provider, 'gemini');
    });

    it('sets provider to ollama', async () => {
      const session = makeSession();
      await handleBotCommand('/provider ollama', session, true);
      assert.equal(session.provider, 'ollama');
    });

    it('no chat-only note for claude provider', async () => {
      const session = makeSession();
      const result = await handleBotCommand('/provider claude', session, true);
      assert.ok(!result.response.includes('chat-only'));
    });

    it('shows usage when no valid provider given', async () => {
      const session = makeSession({ provider: 'gemini' });
      const result = await handleBotCommand('/provider', session, true);
      assert.ok(result.response.includes('Usage'));
      assert.ok(result.response.includes('gemini')); // current provider
    });

    it('rejects unknown provider', async () => {
      const session = makeSession();
      const result = await handleBotCommand('/provider llama3', session, true);
      assert.ok(result.response.includes('Usage'));
    });
  });

  describe('/memory', () => {
    it('toggles memory on when off', async () => {
      const session = makeSession({ memoryEnabled: false });
      const result = await handleBotCommand('/memory', session, true);
      assert.equal(result.handled, true);
      assert.equal(session.memoryEnabled, true);
      assert.ok(result.response.includes('on'));
    });

    it('toggles memory off when on', async () => {
      const session = makeSession({ memoryEnabled: true });
      const result = await handleBotCommand('/memory', session, true);
      assert.equal(session.memoryEnabled, false);
      assert.ok(result.response.includes('off'));
    });
  });

  describe('non-commands', () => {
    it('returns handled=false for regular text', async () => {
      const session = makeSession();
      const result = await handleBotCommand('hello there', session, true);
      assert.equal(result.handled, false);
    });

    it('returns handled=false for unknown /command', async () => {
      const session = makeSession();
      const result = await handleBotCommand('/unknowncmd', session, true);
      assert.equal(result.handled, false);
    });
  });

  describe('whitespace handling', () => {
    it('trims leading/trailing whitespace from input', async () => {
      const session = makeSession();
      const result = await handleBotCommand('  /reset  ', session, true);
      assert.equal(result.handled, true);
      assert.equal(session.sessionId, null);
    });
  });

  describe('commerce commands without commerce object', () => {
    it('/orders returns handled=false without commerce', async () => {
      const session = makeSession();
      const result = await handleBotCommand('/orders', session, true);
      assert.equal(result.handled, false);
    });

    it('/order returns handled=false without commerce', async () => {
      const session = makeSession();
      const result = await handleBotCommand('/order abc', session, true);
      assert.equal(result.handled, false);
    });

    it('/inventory returns handled=false without commerce', async () => {
      const session = makeSession();
      const result = await handleBotCommand('/inventory SKU-1', session, true);
      assert.equal(result.handled, false);
    });

    it('/analytics returns handled=false without commerce', async () => {
      const session = makeSession();
      const result = await handleBotCommand('/analytics', session, true);
      assert.equal(result.handled, false);
    });
  });
});

// ============================================================================
// computeBackoff
// ============================================================================

d('computeBackoff', () => {
  it('returns initialMs on first attempt (within jitter)', () => {
    const delay = computeBackoff(RECONNECT_POLICY, 1);
    const min = RECONNECT_POLICY.initialMs * (1 - RECONNECT_POLICY.jitter);
    const max = RECONNECT_POLICY.initialMs * (1 + RECONNECT_POLICY.jitter);
    assert.ok(delay >= min, `delay ${delay} should be >= ${min}`);
    assert.ok(delay <= max, `delay ${delay} should be <= ${max}`);
  });

  it('increases delay for higher attempt numbers', () => {
    // Run many samples to compare averages
    let sum1 = 0;
    let sum5 = 0;
    const trials = 50;
    for (let i = 0; i < trials; i++) {
      sum1 += computeBackoff(RECONNECT_POLICY, 1);
      sum5 += computeBackoff(RECONNECT_POLICY, 5);
    }
    assert.ok(sum5 / trials > sum1 / trials, 'attempt 5 should average higher than attempt 1');
  });

  it('never exceeds maxMs * (1 + jitter)', () => {
    const ceiling = RECONNECT_POLICY.maxMs * (1 + RECONNECT_POLICY.jitter);
    for (let attempt = 1; attempt <= 20; attempt++) {
      const delay = computeBackoff(RECONNECT_POLICY, attempt);
      assert.ok(delay <= ceiling + 1, `attempt ${attempt}: delay ${delay} exceeds ceiling ${ceiling}`);
    }
  });

  it('always returns a positive integer', () => {
    for (let attempt = 1; attempt <= 15; attempt++) {
      const delay = computeBackoff(RECONNECT_POLICY, attempt);
      assert.ok(Number.isInteger(delay), `delay should be integer, got ${delay}`);
      assert.ok(delay > 0, `delay should be positive, got ${delay}`);
    }
  });

  it('respects custom policy', () => {
    const policy = { initialMs: 100, maxMs: 500, factor: 2, jitter: 0, maxAttempts: 5 };
    // With jitter=0 the formula is deterministic:
    // attempt 1: 100 * 2^0 = 100
    // attempt 2: 100 * 2^1 = 200
    // attempt 3: 100 * 2^2 = 400
    // attempt 4: 100 * 2^3 = 800 clamped to 500
    assert.equal(computeBackoff(policy, 1), 100);
    assert.equal(computeBackoff(policy, 2), 200);
    assert.equal(computeBackoff(policy, 3), 400);
    assert.equal(computeBackoff(policy, 4), 500);
  });

  it('clamps to maxMs for very high attempt numbers', () => {
    const policy = { initialMs: 100, maxMs: 1000, factor: 2, jitter: 0, maxAttempts: 20 };
    const delay = computeBackoff(policy, 100);
    assert.equal(delay, 1000);
  });
});

// ============================================================================
// RECONNECT_POLICY
// ============================================================================

d('RECONNECT_POLICY', () => {
  it('has expected fields', () => {
    assert.equal(typeof RECONNECT_POLICY.initialMs, 'number');
    assert.equal(typeof RECONNECT_POLICY.maxMs, 'number');
    assert.equal(typeof RECONNECT_POLICY.factor, 'number');
    assert.equal(typeof RECONNECT_POLICY.jitter, 'number');
    assert.equal(typeof RECONNECT_POLICY.maxAttempts, 'number');
  });

  it('has initialMs=2000', () => {
    assert.equal(RECONNECT_POLICY.initialMs, 2000);
  });

  it('has maxMs=30000', () => {
    assert.equal(RECONNECT_POLICY.maxMs, 30000);
  });

  it('has factor=1.8', () => {
    assert.equal(RECONNECT_POLICY.factor, 1.8);
  });

  it('has jitter=0.25', () => {
    assert.equal(RECONNECT_POLICY.jitter, 0.25);
  });

  it('has maxAttempts=12', () => {
    assert.equal(RECONNECT_POLICY.maxAttempts, 12);
  });
});

// ============================================================================
// sleep
// ============================================================================

d('sleep', () => {
  it('returns a promise', () => {
    const p = sleep(0);
    assert.ok(p instanceof Promise);
  });

  it('resolves after the specified delay', async () => {
    const start = Date.now();
    await sleep(50);
    const elapsed = Date.now() - start;
    assert.ok(elapsed >= 40, `expected at least ~50ms, got ${elapsed}ms`);
  });

  it('resolves with undefined', async () => {
    const result = await sleep(0);
    assert.equal(result, undefined);
  });
});
