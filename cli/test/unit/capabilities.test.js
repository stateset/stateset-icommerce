/**
 * Unit tests for channels/capabilities.js
 */

import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert';
import {
  getCapabilities,
  registerCapabilities,
  getAllCapabilities,
  hasCapability,
  getChannelsWithCapability,
  resetCapabilities,
} from '../../src/channels/capabilities.js';

afterEach(() => {
  resetCapabilities();
});

// ===========================================================================
// getCapabilities
// ===========================================================================

describe('getCapabilities', () => {
  it('returns known channel capabilities', () => {
    const caps = getCapabilities('telegram');
    assert.strictEqual(caps.richMessages, true);
    assert.strictEqual(caps.buttons, true);
    assert.strictEqual(caps.typing, true);
  });

  it('returns discord capabilities', () => {
    const caps = getCapabilities('discord');
    assert.strictEqual(caps.richMessages, true);
    assert.strictEqual(caps.reactions, true);
    assert.strictEqual(caps.threading, true);
  });

  it('returns slack capabilities', () => {
    const caps = getCapabilities('slack');
    assert.strictEqual(caps.richMessages, true);
    assert.strictEqual(caps.threading, true);
    assert.strictEqual(caps.typing, false);
  });

  it('returns all-false for unknown channels', () => {
    const caps = getCapabilities('unknown-channel');
    assert.strictEqual(caps.richMessages, false);
    assert.strictEqual(caps.buttons, false);
    assert.strictEqual(caps.media, false);
    assert.strictEqual(caps.threading, false);
  });

  it('whatsapp supports media but not rich messages', () => {
    const caps = getCapabilities('whatsapp');
    assert.strictEqual(caps.media, true);
    assert.strictEqual(caps.richMessages, false);
    assert.strictEqual(caps.buttons, false);
  });

  it('signal has minimal capabilities', () => {
    const caps = getCapabilities('signal');
    const allFalse = Object.values(caps).every((v) => v === false);
    assert.strictEqual(allFalse, true);
  });
});

// ===========================================================================
// registerCapabilities
// ===========================================================================

describe('registerCapabilities', () => {
  it('overrides specific capabilities', () => {
    registerCapabilities('telegram', { streaming: true });
    const caps = getCapabilities('telegram');
    assert.strictEqual(caps.streaming, true);
    // Other defaults preserved
    assert.strictEqual(caps.richMessages, true);
    assert.strictEqual(caps.buttons, true);
  });

  it('registers capabilities for new channels', () => {
    registerCapabilities('matrix', { richMessages: true, threading: true });
    const caps = getCapabilities('matrix');
    assert.strictEqual(caps.richMessages, true);
    assert.strictEqual(caps.threading, true);
    // Non-overridden defaults are false (from empty capabilities)
    assert.strictEqual(caps.buttons, false);
  });

  it('merges multiple registrations', () => {
    registerCapabilities('telegram', { streaming: true });
    registerCapabilities('telegram', { polls: true });
    const caps = getCapabilities('telegram');
    assert.strictEqual(caps.streaming, true);
    assert.strictEqual(caps.polls, true);
  });
});

// ===========================================================================
// getAllCapabilities
// ===========================================================================

describe('getAllCapabilities', () => {
  it('includes all default channels', () => {
    const all = getAllCapabilities();
    assert.ok('telegram' in all);
    assert.ok('discord' in all);
    assert.ok('slack' in all);
    assert.ok('whatsapp' in all);
    assert.ok('signal' in all);
    assert.ok('google-chat' in all);
  });

  it('includes registered custom channels', () => {
    registerCapabilities('matrix', { richMessages: true });
    const all = getAllCapabilities();
    assert.ok('matrix' in all);
    assert.strictEqual(all.matrix.richMessages, true);
  });
});

// ===========================================================================
// hasCapability
// ===========================================================================

describe('hasCapability', () => {
  it('returns true for supported capabilities', () => {
    assert.strictEqual(hasCapability('discord', 'reactions'), true);
    assert.strictEqual(hasCapability('telegram', 'media'), true);
  });

  it('returns false for unsupported capabilities', () => {
    assert.strictEqual(hasCapability('signal', 'richMessages'), false);
    assert.strictEqual(hasCapability('whatsapp', 'buttons'), false);
  });

  it('returns false for unknown channels', () => {
    assert.strictEqual(hasCapability('unknown', 'media'), false);
  });
});

// ===========================================================================
// getChannelsWithCapability
// ===========================================================================

describe('getChannelsWithCapability', () => {
  it('returns channels supporting richMessages', () => {
    const channels = getChannelsWithCapability('richMessages');
    assert.ok(channels.includes('telegram'));
    assert.ok(channels.includes('discord'));
    assert.ok(channels.includes('slack'));
    assert.ok(!channels.includes('signal'));
    assert.ok(!channels.includes('whatsapp'));
  });

  it('returns channels supporting threading', () => {
    const channels = getChannelsWithCapability('threading');
    assert.ok(channels.includes('discord'));
    assert.ok(channels.includes('slack'));
    assert.ok(!channels.includes('telegram'));
  });

  it('returns empty for unsupported capability', () => {
    const channels = getChannelsWithCapability('streaming');
    assert.strictEqual(channels.length, 0);
  });

  it('includes custom channels with registered capability', () => {
    registerCapabilities('matrix', { threading: true });
    const channels = getChannelsWithCapability('threading');
    assert.ok(channels.includes('matrix'));
  });
});

// ===========================================================================
// resetCapabilities
// ===========================================================================

describe('resetCapabilities', () => {
  it('clears all overrides', () => {
    registerCapabilities('telegram', { streaming: true });
    assert.strictEqual(getCapabilities('telegram').streaming, true);

    resetCapabilities();
    assert.strictEqual(getCapabilities('telegram').streaming, false);
  });

  it('removes custom channels', () => {
    registerCapabilities('matrix', { richMessages: true });
    resetCapabilities();
    // Matrix should now be unknown → all false
    const caps = getCapabilities('matrix');
    assert.strictEqual(caps.richMessages, false);
  });
});
