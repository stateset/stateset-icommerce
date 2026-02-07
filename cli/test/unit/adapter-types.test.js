/**
 * Unit tests for channels/adapter-types.js — capability detection utilities
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import {
  hasAdapterCapability,
  getAdapterCapabilities,
  composeAdapters,
  withAdapterLogging,
} from '../../src/channels/adapter-types.js';

// ---------------------------------------------------------------------------
// Helpers — mock adapters with specific capabilities
// ---------------------------------------------------------------------------

function fullAdapter() {
  return {
    send: async () => {},
    sendRichMessage: async () => {},
    startStream: () => ({}),
    getThreadId: () => null,
    sendToThread: async () => {},
    getSupportedActions: () => ['editMessage'],
    getUser: async () => null,
    formatMention: (id) => `<@${id}>`,
    sendMedia: async () => {},
    checkHealth: async () => ({ status: 'connected' }),
    registerNativeCommands: async () => {},
  };
}

function minimalAdapter() {
  return {
    send: async () => {},
  };
}

// ===========================================================================
// hasAdapterCapability
// ===========================================================================

describe('hasAdapterCapability', () => {
  it('detects outbound capability', () => {
    assert.strictEqual(hasAdapterCapability({ send: () => {} }, 'outbound'), true);
    assert.strictEqual(hasAdapterCapability({}, 'outbound'), false);
  });

  it('detects richMessage capability', () => {
    assert.strictEqual(hasAdapterCapability({ sendRichMessage: () => {} }, 'richMessage'), true);
    assert.strictEqual(hasAdapterCapability({}, 'richMessage'), false);
  });

  it('detects streaming capability', () => {
    assert.strictEqual(hasAdapterCapability({ startStream: () => {} }, 'streaming'), true);
    assert.strictEqual(hasAdapterCapability({}, 'streaming'), false);
  });

  it('detects threading — requires both getThreadId and sendToThread', () => {
    assert.strictEqual(
      hasAdapterCapability({ getThreadId: () => {}, sendToThread: () => {} }, 'threading'),
      true,
    );
    assert.strictEqual(hasAdapterCapability({ getThreadId: () => {} }, 'threading'), false);
    assert.strictEqual(hasAdapterCapability({ sendToThread: () => {} }, 'threading'), false);
  });

  it('detects actions capability', () => {
    assert.strictEqual(hasAdapterCapability({ getSupportedActions: () => [] }, 'actions'), true);
    assert.strictEqual(hasAdapterCapability({}, 'actions'), false);
  });

  it('detects directory capability', () => {
    assert.strictEqual(hasAdapterCapability({ getUser: () => {} }, 'directory'), true);
    assert.strictEqual(hasAdapterCapability({}, 'directory'), false);
  });

  it('detects mentions capability', () => {
    assert.strictEqual(hasAdapterCapability({ formatMention: () => {} }, 'mentions'), true);
    assert.strictEqual(hasAdapterCapability({}, 'mentions'), false);
  });

  it('detects media capability', () => {
    assert.strictEqual(hasAdapterCapability({ sendMedia: () => {} }, 'media'), true);
    assert.strictEqual(hasAdapterCapability({}, 'media'), false);
  });

  it('detects heartbeat capability', () => {
    assert.strictEqual(hasAdapterCapability({ checkHealth: () => {} }, 'heartbeat'), true);
    assert.strictEqual(hasAdapterCapability({}, 'heartbeat'), false);
  });

  it('detects commands capability', () => {
    assert.strictEqual(
      hasAdapterCapability({ registerNativeCommands: () => {} }, 'commands'),
      true,
    );
    assert.strictEqual(hasAdapterCapability({}, 'commands'), false);
  });

  it('returns false for unknown type', () => {
    assert.strictEqual(hasAdapterCapability(fullAdapter(), 'nonexistent'), false);
  });
});

// ===========================================================================
// getAdapterCapabilities
// ===========================================================================

describe('getAdapterCapabilities', () => {
  it('returns all capabilities for full adapter', () => {
    const caps = getAdapterCapabilities(fullAdapter());
    assert.ok(caps.includes('outbound'));
    assert.ok(caps.includes('richMessage'));
    assert.ok(caps.includes('streaming'));
    assert.ok(caps.includes('threading'));
    assert.ok(caps.includes('actions'));
    assert.ok(caps.includes('directory'));
    assert.ok(caps.includes('mentions'));
    assert.ok(caps.includes('media'));
    assert.ok(caps.includes('heartbeat'));
    assert.ok(caps.includes('commands'));
    assert.strictEqual(caps.length, 10);
  });

  it('returns only outbound for minimal adapter', () => {
    const caps = getAdapterCapabilities(minimalAdapter());
    assert.deepStrictEqual(caps, ['outbound']);
  });

  it('returns empty array for empty object', () => {
    assert.deepStrictEqual(getAdapterCapabilities({}), []);
  });
});

// ===========================================================================
// composeAdapters
// ===========================================================================

describe('composeAdapters', () => {
  it('merges capabilities from multiple adapters', () => {
    const outbound = { send: async () => 'sent' };
    const rich = { sendRichMessage: async () => 'rich' };
    const composed = composeAdapters(outbound, rich);

    assert.ok(typeof composed.send === 'function');
    assert.ok(typeof composed.sendRichMessage === 'function');
  });

  it('later adapters override earlier', () => {
    const a = { send: () => 'a' };
    const b = { send: () => 'b' };
    const composed = composeAdapters(a, b);
    assert.strictEqual(composed.send(), 'b');
  });

  it('handles empty compose', () => {
    const composed = composeAdapters();
    assert.deepStrictEqual(composed, {});
  });

  it('preserves non-function properties', () => {
    const a = { maxMessageLength: 4096 };
    const b = { rateLimitMs: 100 };
    const composed = composeAdapters(a, b);
    assert.strictEqual(composed.maxMessageLength, 4096);
    assert.strictEqual(composed.rateLimitMs, 100);
  });
});

// ===========================================================================
// withAdapterLogging
// ===========================================================================

describe('withAdapterLogging', () => {
  it('returns original adapter when verbose is false', () => {
    const adapter = minimalAdapter();
    const result = withAdapterLogging(adapter, 'test', false);
    assert.strictEqual(result, adapter);
  });

  it('returns original adapter when verbose is omitted', () => {
    const adapter = minimalAdapter();
    const result = withAdapterLogging(adapter, 'test');
    assert.strictEqual(result, adapter);
  });

  it('wraps functions in proxy when verbose is true', async () => {
    const calls = [];
    const adapter = { send: async (target, text) => calls.push({ target, text }) };
    const logged = withAdapterLogging(adapter, 'slack', true);

    await logged.send('ch1', 'hello');
    assert.strictEqual(calls.length, 1);
    assert.strictEqual(calls[0].target, 'ch1');
  });

  it('preserves non-function properties through proxy', () => {
    const adapter = { maxMessageLength: 4096, send: () => {} };
    const logged = withAdapterLogging(adapter, 'test', true);
    assert.strictEqual(logged.maxMessageLength, 4096);
  });

  it('logged adapter has same capabilities', () => {
    const adapter = fullAdapter();
    const logged = withAdapterLogging(adapter, 'test', true);
    const origCaps = getAdapterCapabilities(adapter);
    const loggedCaps = getAdapterCapabilities(logged);
    assert.deepStrictEqual(loggedCaps, origCaps);
  });
});
