/**
 * Unit tests for channels/handoff.js — HandoffQueue
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import { HandoffQueue } from '../../src/channels/handoff.js';

// ===========================================================================
// escalate
// ===========================================================================

describe('HandoffQueue escalate', () => {
  it('creates a handoff entry', () => {
    const q = new HandoffQueue();
    const entry = q.escalate('telegram', 'user-1', 'chat-1', 'Need human');
    assert.strictEqual(entry.channel, 'telegram');
    assert.strictEqual(entry.senderId, 'user-1');
    assert.strictEqual(entry.targetId, 'chat-1');
    assert.strictEqual(entry.reason, 'Need human');
    assert.ok(entry.escalatedAt > 0);
    assert.deepStrictEqual(entry.messages, []);
    assert.strictEqual(entry.assignedTo, null);
  });

  it('uses default reason when not provided', () => {
    const q = new HandoffQueue();
    const entry = q.escalate('slack', 'u1', 't1');
    assert.ok(entry.reason.includes('Customer requested'));
  });
});

// ===========================================================================
// isHandedOff
// ===========================================================================

describe('HandoffQueue isHandedOff', () => {
  it('returns true for active handoff', () => {
    const q = new HandoffQueue();
    q.escalate('slack', 'user-1', 'chat-1');
    assert.strictEqual(q.isHandedOff('slack', 'user-1'), true);
  });

  it('returns false when not handed off', () => {
    const q = new HandoffQueue();
    assert.strictEqual(q.isHandedOff('slack', 'user-1'), false);
  });
});

// ===========================================================================
// getEntry
// ===========================================================================

describe('HandoffQueue getEntry', () => {
  it('returns the entry', () => {
    const q = new HandoffQueue();
    q.escalate('tg', 'u1', 't1', 'test');
    const entry = q.getEntry('tg', 'u1');
    assert.strictEqual(entry.reason, 'test');
  });

  it('returns null for missing entry', () => {
    const q = new HandoffQueue();
    assert.strictEqual(q.getEntry('tg', 'u1'), null);
  });
});

// ===========================================================================
// recordMessage / recordReply
// ===========================================================================

describe('HandoffQueue message recording', () => {
  it('recordMessage appends customer message', () => {
    const q = new HandoffQueue();
    q.escalate('tg', 'u1', 't1');
    q.recordMessage('tg', 'u1', 'Help me please');

    const entry = q.getEntry('tg', 'u1');
    assert.strictEqual(entry.messages.length, 1);
    assert.strictEqual(entry.messages[0].from, 'customer');
    assert.strictEqual(entry.messages[0].text, 'Help me please');
    assert.ok(entry.messages[0].timestamp > 0);
  });

  it('recordMessage is no-op for missing entry', () => {
    const q = new HandoffQueue();
    q.recordMessage('tg', 'nobody', 'ghost message'); // should not throw
  });

  it('recordReply appends agent reply', () => {
    const q = new HandoffQueue();
    q.escalate('tg', 'u1', 't1');
    q.recordReply('tg', 'u1', 'I will help you', 'Alice');

    const entry = q.getEntry('tg', 'u1');
    assert.strictEqual(entry.messages.length, 1);
    assert.strictEqual(entry.messages[0].from, 'Alice');
    assert.strictEqual(entry.messages[0].text, 'I will help you');
  });

  it('recordReply sets assignedTo on first reply', () => {
    const q = new HandoffQueue();
    q.escalate('tg', 'u1', 't1');
    q.recordReply('tg', 'u1', 'hi', 'Bob');

    assert.strictEqual(q.getEntry('tg', 'u1').assignedTo, 'Bob');
  });

  it('recordReply does not change assignedTo on subsequent replies', () => {
    const q = new HandoffQueue();
    q.escalate('tg', 'u1', 't1');
    q.recordReply('tg', 'u1', 'first', 'Alice');
    q.recordReply('tg', 'u1', 'second', 'Bob');

    assert.strictEqual(q.getEntry('tg', 'u1').assignedTo, 'Alice');
  });

  it('recordReply defaults agent name to "agent"', () => {
    const q = new HandoffQueue();
    q.escalate('tg', 'u1', 't1');
    q.recordReply('tg', 'u1', 'hi');

    assert.strictEqual(q.getEntry('tg', 'u1').messages[0].from, 'agent');
  });

  it('recordReply is no-op for missing entry', () => {
    const q = new HandoffQueue();
    q.recordReply('tg', 'nobody', 'ghost', 'Agent'); // should not throw
  });
});

// ===========================================================================
// release
// ===========================================================================

describe('HandoffQueue release', () => {
  it('releases an active handoff', () => {
    const q = new HandoffQueue();
    q.escalate('tg', 'u1', 't1');
    const result = q.release('tg', 'u1');
    assert.strictEqual(result.released, true);
    assert.ok(result.entry);
    assert.strictEqual(q.isHandedOff('tg', 'u1'), false);
  });

  it('returns released:false for missing entry', () => {
    const q = new HandoffQueue();
    const result = q.release('tg', 'nobody');
    assert.strictEqual(result.released, false);
    assert.strictEqual(result.entry, null);
  });
});

// ===========================================================================
// listActive
// ===========================================================================

describe('HandoffQueue listActive', () => {
  it('returns all active handoffs', () => {
    const q = new HandoffQueue();
    q.escalate('tg', 'u1', 't1');
    q.escalate('slack', 'u2', 't2');

    const active = q.listActive();
    assert.strictEqual(active.length, 2);
  });

  it('returns empty array when none active', () => {
    const q = new HandoffQueue();
    assert.deepStrictEqual(q.listActive(), []);
  });
});

// ===========================================================================
// getHistoryText
// ===========================================================================

describe('HandoffQueue getHistoryText', () => {
  it('returns formatted conversation', () => {
    const q = new HandoffQueue();
    q.escalate('tg', 'u1', 't1');
    q.recordMessage('tg', 'u1', 'Help!');
    q.recordReply('tg', 'u1', 'On it.', 'Alice');

    const text = q.getHistoryText('tg', 'u1');
    assert.ok(text.includes('customer: Help!'));
    assert.ok(text.includes('Alice: On it.'));
  });

  it('returns no-messages for empty conversation', () => {
    const q = new HandoffQueue();
    q.escalate('tg', 'u1', 't1');
    const text = q.getHistoryText('tg', 'u1');
    assert.ok(text.includes('No messages'));
  });

  it('returns no-messages for missing entry', () => {
    const q = new HandoffQueue();
    const text = q.getHistoryText('tg', 'nobody');
    assert.ok(text.includes('No messages'));
  });
});

// ===========================================================================
// exportHistory
// ===========================================================================

describe('HandoffQueue exportHistory', () => {
  it('exports structured history', () => {
    const q = new HandoffQueue();
    q.escalate('tg', 'u1', 't1', 'Urgent');
    q.recordMessage('tg', 'u1', 'Help');
    q.recordReply('tg', 'u1', 'Sure', 'Alice');

    const exported = q.exportHistory('tg', 'u1');
    assert.strictEqual(exported.channel, 'tg');
    assert.strictEqual(exported.senderId, 'u1');
    assert.strictEqual(exported.reason, 'Urgent');
    assert.strictEqual(exported.assignedTo, 'Alice');
    assert.strictEqual(exported.messageCount, 2);
    assert.strictEqual(exported.messages.length, 2);
    assert.ok(exported.escalatedAt); // ISO string
    assert.ok(exported.messages[0].timestamp); // ISO string
  });

  it('returns null for missing entry', () => {
    const q = new HandoffQueue();
    assert.strictEqual(q.exportHistory('tg', 'nobody'), null);
  });
});

// ===========================================================================
// setOpsRoute
// ===========================================================================

describe('HandoffQueue setOpsRoute', () => {
  it('sets ops route', () => {
    const q = new HandoffQueue();
    q.setOpsRoute('slack', '#support');
    assert.deepStrictEqual(q._opsRoute, { channel: 'slack', target: '#support' });
  });
});

// ===========================================================================
// Multiple concurrent handoffs
// ===========================================================================

describe('HandoffQueue concurrent handoffs', () => {
  it('tracks separate conversations independently', () => {
    const q = new HandoffQueue();
    q.escalate('tg', 'u1', 't1', 'reason-1');
    q.escalate('tg', 'u2', 't2', 'reason-2');

    q.recordMessage('tg', 'u1', 'msg from u1');
    q.recordMessage('tg', 'u2', 'msg from u2');

    assert.strictEqual(q.getEntry('tg', 'u1').messages.length, 1);
    assert.strictEqual(q.getEntry('tg', 'u2').messages.length, 1);
    assert.strictEqual(q.getEntry('tg', 'u1').messages[0].text, 'msg from u1');

    q.release('tg', 'u1');
    assert.strictEqual(q.isHandedOff('tg', 'u1'), false);
    assert.strictEqual(q.isHandedOff('tg', 'u2'), true);
  });
});
