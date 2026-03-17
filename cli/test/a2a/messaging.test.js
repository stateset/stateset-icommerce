/**
 * Tests for cli/src/a2a/messaging.js
 *
 * Covers: createMessagingService — sendMessage, getInbox, getOutbox,
 * getMessage, markRead, delegateTask, respondToTask, queryStatus,
 * getThread, getMetrics, purgeExpired, priority ordering, TTL expiry,
 * per-agent inbox isolation.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import { createMessagingService } from '../../src/a2a/messaging.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const ALICE = '0xAlice';
const BOB = '0xBob';
const CHARLIE = '0xCharlie';

function makeService() {
  return createMessagingService();
}

function sendText(svc, from, to, body, opts = {}) {
  return svc.sendMessage({
    from,
    to,
    type: 'text',
    payload: { body },
    ...opts,
  });
}

// ---------------------------------------------------------------------------
// 1. sendMessage creates message in sender outbox and receiver inbox
// ---------------------------------------------------------------------------

describe('Messaging — sendMessage stores in outbox and inbox', () => {
  let svc;
  beforeEach(() => {
    svc = makeService();
  });

  it('appears in receiver inbox', () => {
    const msg = sendText(svc, ALICE, BOB, 'hello');
    const inbox = svc.getInbox(BOB);
    assert.equal(inbox.length, 1);
    assert.equal(inbox[0].id, msg.id);
  });

  it('appears in sender outbox', () => {
    const msg = sendText(svc, ALICE, BOB, 'hello');
    const outbox = svc.getOutbox(ALICE);
    assert.equal(outbox.length, 1);
    assert.equal(outbox[0].id, msg.id);
  });

  it('sets correct fields on the message', () => {
    const msg = sendText(svc, ALICE, BOB, 'hello');
    assert.equal(msg.from, ALICE);
    assert.equal(msg.to, BOB);
    assert.equal(msg.type, 'text');
    assert.deepStrictEqual(msg.payload, { body: 'hello' });
    assert.equal(msg.read, false);
    assert.equal(msg.taskStatus, null);
    assert.ok(msg.id);
    assert.ok(msg.createdAt);
    assert.ok(msg.expiresAt);
  });

  it('does not appear in other agents inboxes', () => {
    sendText(svc, ALICE, BOB, 'hello');
    const inbox = svc.getInbox(CHARLIE);
    assert.equal(inbox.length, 0);
  });

  it('rejects invalid message type', () => {
    assert.throws(
      () => svc.sendMessage({ from: ALICE, to: BOB, type: 'invalid', payload: {} }),
      /Invalid message type/,
    );
  });

  it('rejects missing from', () => {
    assert.throws(
      () => svc.sendMessage({ from: '', to: BOB, type: 'text', payload: {} }),
      /from must be a non-empty string/,
    );
  });

  it('rejects null payload', () => {
    assert.throws(
      () => svc.sendMessage({ from: ALICE, to: BOB, type: 'text', payload: null }),
      /payload must be a non-null object/,
    );
  });

  it('rejects non-positive ttlMs', () => {
    assert.throws(
      () => svc.sendMessage({ from: ALICE, to: BOB, type: 'text', payload: {}, ttlMs: 0 }),
      /ttlMs must be a positive number/,
    );
  });
});

// ---------------------------------------------------------------------------
// 2. getInbox returns messages sorted by priority then timestamp
// ---------------------------------------------------------------------------

describe('Messaging — getInbox sorts by priority then timestamp', () => {
  let svc;
  beforeEach(() => {
    svc = makeService();
  });

  it('higher priority messages come first', () => {
    sendText(svc, ALICE, BOB, 'low', { priority: 'low' });
    sendText(svc, CHARLIE, BOB, 'critical', { priority: 'critical' });
    sendText(svc, ALICE, BOB, 'medium', { priority: 'medium' });

    const inbox = svc.getInbox(BOB);
    assert.equal(inbox[0].payload.body, 'critical');
    assert.equal(inbox[1].payload.body, 'medium');
    assert.equal(inbox[2].payload.body, 'low');
  });

  it('within same priority, newer messages come first', () => {
    const m1 = sendText(svc, ALICE, BOB, 'first', { priority: 'high' });
    const m2 = sendText(svc, CHARLIE, BOB, 'second', { priority: 'high' });

    const inbox = svc.getInbox(BOB);
    // m2 is newer, should come first
    assert.equal(inbox[0].id, m2.id);
    assert.equal(inbox[1].id, m1.id);
  });
});

// ---------------------------------------------------------------------------
// 3. getInbox with unreadOnly filters correctly
// ---------------------------------------------------------------------------

describe('Messaging — getInbox with unreadOnly', () => {
  let svc;
  beforeEach(() => {
    svc = makeService();
  });

  it('returns only unread messages when unreadOnly is true', () => {
    const m1 = sendText(svc, ALICE, BOB, 'first');
    sendText(svc, CHARLIE, BOB, 'second');

    svc.markRead(m1.id);

    const unread = svc.getInbox(BOB, { unreadOnly: true });
    assert.equal(unread.length, 1);
    assert.equal(unread[0].payload.body, 'second');
  });

  it('returns all messages when unreadOnly is false', () => {
    const m1 = sendText(svc, ALICE, BOB, 'first');
    sendText(svc, CHARLIE, BOB, 'second');
    svc.markRead(m1.id);

    const all = svc.getInbox(BOB, { unreadOnly: false });
    assert.equal(all.length, 2);
  });

  it('returns empty array when all are read', () => {
    const m1 = sendText(svc, ALICE, BOB, 'only');
    svc.markRead(m1.id);

    const unread = svc.getInbox(BOB, { unreadOnly: true });
    assert.equal(unread.length, 0);
  });
});

// ---------------------------------------------------------------------------
// 4. markRead marks message as read
// ---------------------------------------------------------------------------

describe('Messaging — markRead', () => {
  let svc;
  beforeEach(() => {
    svc = makeService();
  });

  it('sets read to true', () => {
    const msg = sendText(svc, ALICE, BOB, 'hello');
    assert.equal(msg.read, false);

    const updated = svc.markRead(msg.id);
    assert.equal(updated.read, true);
    assert.equal(updated.id, msg.id);
  });

  it('getMessage reflects read status', () => {
    const msg = sendText(svc, ALICE, BOB, 'hello');
    svc.markRead(msg.id);
    const fetched = svc.getMessage(msg.id);
    assert.equal(fetched.read, true);
  });

  it('throws for unknown messageId', () => {
    assert.throws(
      () => svc.markRead('nonexistent-id'),
      /Message not found/,
    );
  });

  it('is idempotent', () => {
    const msg = sendText(svc, ALICE, BOB, 'hello');
    svc.markRead(msg.id);
    svc.markRead(msg.id);
    assert.equal(svc.getMessage(msg.id).read, true);
  });
});

// ---------------------------------------------------------------------------
// 5. delegateTask creates task_delegation message with correct fields
// ---------------------------------------------------------------------------

describe('Messaging — delegateTask', () => {
  let svc;
  beforeEach(() => {
    svc = makeService();
  });

  it('creates a task_delegation message', () => {
    const deadline = new Date(Date.now() + 3_600_000).toISOString();
    const task = svc.delegateTask({
      from: ALICE,
      to: BOB,
      description: 'Fulfill order #42',
      deadline,
      reward: 25.0,
      priority: 'high',
    });

    assert.equal(task.type, 'task_delegation');
    assert.equal(task.from, ALICE);
    assert.equal(task.to, BOB);
    assert.equal(task.priority, 'high');
    assert.equal(task.payload.description, 'Fulfill order #42');
    assert.equal(task.payload.deadline, deadline);
    assert.equal(task.payload.reward, 25.0);
    assert.equal(task.payload.priority, 'high');
  });

  it('appears in receiver inbox', () => {
    const task = svc.delegateTask({
      from: ALICE,
      to: BOB,
      description: 'Do thing',
      deadline: new Date().toISOString(),
      reward: 10,
      priority: 'medium',
    });

    const inbox = svc.getInbox(BOB);
    assert.equal(inbox.length, 1);
    assert.equal(inbox[0].id, task.id);
  });

  it('rejects negative reward', () => {
    assert.throws(
      () =>
        svc.delegateTask({
          from: ALICE,
          to: BOB,
          description: 'Do thing',
          deadline: new Date().toISOString(),
          reward: -5,
          priority: 'low',
        }),
      /reward must be a non-negative number/,
    );
  });

  it('rejects invalid priority', () => {
    assert.throws(
      () =>
        svc.delegateTask({
          from: ALICE,
          to: BOB,
          description: 'Do thing',
          deadline: new Date().toISOString(),
          reward: 10,
          priority: 'urgent',
        }),
      /Invalid priority/,
    );
  });
});

// ---------------------------------------------------------------------------
// 6. respondToTask updates task status
// ---------------------------------------------------------------------------

describe('Messaging — respondToTask', () => {
  let svc;
  let taskMsg;

  beforeEach(() => {
    svc = makeService();
    taskMsg = svc.delegateTask({
      from: ALICE,
      to: BOB,
      description: 'Process order',
      deadline: new Date(Date.now() + 3_600_000).toISOString(),
      reward: 50,
      priority: 'high',
    });
  });

  it('sets taskStatus to accepted', () => {
    svc.respondToTask(taskMsg.id, { status: 'accepted' });
    const updated = svc.getMessage(taskMsg.id);
    assert.equal(updated.taskStatus, 'accepted');
    assert.equal(updated.taskResponse.status, 'accepted');
    assert.ok(updated.taskResponse.respondedAt);
  });

  it('sets taskStatus to rejected', () => {
    svc.respondToTask(taskMsg.id, { status: 'rejected' });
    const updated = svc.getMessage(taskMsg.id);
    assert.equal(updated.taskStatus, 'rejected');
  });

  it('sets taskStatus to completed with result', () => {
    svc.respondToTask(taskMsg.id, {
      status: 'completed',
      result: { orderId: 'ORD-42', trackingNumber: 'TRACK-99' },
    });

    const updated = svc.getMessage(taskMsg.id);
    assert.equal(updated.taskStatus, 'completed');
    assert.deepStrictEqual(updated.taskResponse.result, {
      orderId: 'ORD-42',
      trackingNumber: 'TRACK-99',
    });
  });

  it('sends a response message back to the delegator', () => {
    const response = svc.respondToTask(taskMsg.id, { status: 'accepted' });
    assert.equal(response.type, 'status_response');
    assert.equal(response.from, BOB);
    assert.equal(response.to, ALICE);
    assert.equal(response.parentMessageId, taskMsg.id);
    assert.equal(response.payload.taskMessageId, taskMsg.id);
  });

  it('throws for non-task message', () => {
    const textMsg = sendText(svc, ALICE, BOB, 'hello');
    assert.throws(
      () => svc.respondToTask(textMsg.id, { status: 'accepted' }),
      /not a task_delegation/,
    );
  });

  it('throws for unknown messageId', () => {
    assert.throws(
      () => svc.respondToTask('nonexistent', { status: 'accepted' }),
      /Task message not found/,
    );
  });

  it('throws for invalid response status', () => {
    assert.throws(
      () => svc.respondToTask(taskMsg.id, { status: 'pending' }),
      /Invalid task response status/,
    );
  });
});

// ---------------------------------------------------------------------------
// 7. queryStatus creates status_query message
// ---------------------------------------------------------------------------

describe('Messaging — queryStatus', () => {
  let svc;
  beforeEach(() => {
    svc = makeService();
  });

  it('creates a status_query message', () => {
    const msg = svc.queryStatus({
      from: ALICE,
      to: BOB,
      queryType: 'order_status',
      context: { orderId: 'ORD-42' },
    });

    assert.equal(msg.type, 'status_query');
    assert.equal(msg.from, ALICE);
    assert.equal(msg.to, BOB);
    assert.equal(msg.payload.queryType, 'order_status');
    assert.deepStrictEqual(msg.payload.context, { orderId: 'ORD-42' });
  });

  it('appears in receiver inbox', () => {
    svc.queryStatus({
      from: ALICE,
      to: BOB,
      queryType: 'health_check',
    });

    const inbox = svc.getInbox(BOB);
    assert.equal(inbox.length, 1);
    assert.equal(inbox[0].type, 'status_query');
  });

  it('defaults context to empty object', () => {
    const msg = svc.queryStatus({
      from: ALICE,
      to: BOB,
      queryType: 'ping',
    });

    assert.deepStrictEqual(msg.payload.context, {});
  });

  it('throws for empty queryType', () => {
    assert.throws(
      () =>
        svc.queryStatus({
          from: ALICE,
          to: BOB,
          queryType: '',
        }),
      /queryType must be a non-empty string/,
    );
  });
});

// ---------------------------------------------------------------------------
// 8. getThread returns all messages in a thread
// ---------------------------------------------------------------------------

describe('Messaging — getThread', () => {
  let svc;
  beforeEach(() => {
    svc = makeService();
  });

  it('returns parent and all replies', () => {
    const parent = sendText(svc, ALICE, BOB, 'start');
    sendText(svc, BOB, ALICE, 'reply 1', { parentMessageId: parent.id });
    sendText(svc, ALICE, BOB, 'reply 2', { parentMessageId: parent.id });

    const thread = svc.getThread(parent.id);
    assert.equal(thread.length, 3);
    assert.equal(thread[0].id, parent.id);
    assert.equal(thread[1].payload.body, 'reply 1');
    assert.equal(thread[2].payload.body, 'reply 2');
  });

  it('sorts thread messages by creation time (oldest first)', () => {
    const parent = sendText(svc, ALICE, BOB, 'start');
    const r1 = sendText(svc, BOB, ALICE, 'reply 1', { parentMessageId: parent.id });
    const r2 = sendText(svc, ALICE, BOB, 'reply 2', { parentMessageId: parent.id });

    const thread = svc.getThread(parent.id);
    const times = thread.map((m) => new Date(m.createdAt).getTime());
    assert.ok(times[0] <= times[1], 'first <= second');
    assert.ok(times[1] <= times[2], 'second <= third');
  });

  it('returns empty array for unknown parent', () => {
    const thread = svc.getThread('nonexistent');
    assert.equal(thread.length, 0);
  });

  it('does not include messages from other threads', () => {
    const p1 = sendText(svc, ALICE, BOB, 'thread 1');
    const p2 = sendText(svc, ALICE, BOB, 'thread 2');
    sendText(svc, BOB, ALICE, 'reply to thread 1', { parentMessageId: p1.id });
    sendText(svc, BOB, ALICE, 'reply to thread 2', { parentMessageId: p2.id });

    const thread1 = svc.getThread(p1.id);
    assert.equal(thread1.length, 2);
    assert.ok(thread1.every((m) => m.id === p1.id || m.parentMessageId === p1.id));
  });
});

// ---------------------------------------------------------------------------
// 9. Message TTL expiry — purgeExpired removes old messages
// ---------------------------------------------------------------------------

describe('Messaging — TTL expiry and purgeExpired', () => {
  let svc;
  beforeEach(() => {
    svc = makeService();
  });

  it('expired messages are excluded from getInbox', () => {
    // Send a message with 1ms TTL
    svc.sendMessage({
      from: ALICE,
      to: BOB,
      type: 'text',
      payload: { body: 'ephemeral' },
      ttlMs: 1,
    });

    // Wait just enough for expiry
    const start = Date.now();
    while (Date.now() - start < 5) {
      // busy-wait a few ms
    }

    const inbox = svc.getInbox(BOB);
    assert.equal(inbox.length, 0);
  });

  it('purgeExpired removes expired messages from storage', () => {
    svc.sendMessage({
      from: ALICE,
      to: BOB,
      type: 'text',
      payload: { body: 'ephemeral' },
      ttlMs: 1,
    });

    const start = Date.now();
    while (Date.now() - start < 5) {
      // busy-wait
    }

    const result = svc.purgeExpired();
    assert.equal(result.purged, 1);

    // getMessage should also return null
    const inbox = svc.getInbox(BOB);
    assert.equal(inbox.length, 0);
  });

  it('purgeExpired returns zero when nothing is expired', () => {
    sendText(svc, ALICE, BOB, 'long-lived');
    const result = svc.purgeExpired();
    assert.equal(result.purged, 0);
  });

  it('non-expired messages are retained after purge', () => {
    svc.sendMessage({
      from: ALICE,
      to: BOB,
      type: 'text',
      payload: { body: 'ephemeral' },
      ttlMs: 1,
    });
    sendText(svc, CHARLIE, BOB, 'keeper');

    const start = Date.now();
    while (Date.now() - start < 5) {
      // busy-wait
    }

    svc.purgeExpired();
    const inbox = svc.getInbox(BOB);
    assert.equal(inbox.length, 1);
    assert.equal(inbox[0].payload.body, 'keeper');
  });
});

// ---------------------------------------------------------------------------
// 10. Different agents have separate inboxes
// ---------------------------------------------------------------------------

describe('Messaging — per-agent inbox isolation', () => {
  let svc;
  beforeEach(() => {
    svc = makeService();
  });

  it('messages to Bob do not appear in Charlie inbox', () => {
    sendText(svc, ALICE, BOB, 'for Bob');
    sendText(svc, ALICE, CHARLIE, 'for Charlie');

    const bobInbox = svc.getInbox(BOB);
    const charlieInbox = svc.getInbox(CHARLIE);

    assert.equal(bobInbox.length, 1);
    assert.equal(bobInbox[0].payload.body, 'for Bob');
    assert.equal(charlieInbox.length, 1);
    assert.equal(charlieInbox[0].payload.body, 'for Charlie');
  });

  it('agent with no messages has empty inbox', () => {
    sendText(svc, ALICE, BOB, 'hello');
    assert.equal(svc.getInbox(CHARLIE).length, 0);
  });

  it('outbox is also per-agent', () => {
    sendText(svc, ALICE, BOB, 'from Alice');
    sendText(svc, CHARLIE, BOB, 'from Charlie');

    assert.equal(svc.getOutbox(ALICE).length, 1);
    assert.equal(svc.getOutbox(CHARLIE).length, 1);
    assert.equal(svc.getOutbox(BOB).length, 0);
  });
});

// ---------------------------------------------------------------------------
// 11. getOutbox returns sent messages
// ---------------------------------------------------------------------------

describe('Messaging — getOutbox', () => {
  let svc;
  beforeEach(() => {
    svc = makeService();
  });

  it('returns all sent messages', () => {
    sendText(svc, ALICE, BOB, 'msg 1');
    sendText(svc, ALICE, CHARLIE, 'msg 2');
    sendText(svc, ALICE, BOB, 'msg 3');

    const outbox = svc.getOutbox(ALICE);
    assert.equal(outbox.length, 3);
  });

  it('filters by type', () => {
    sendText(svc, ALICE, BOB, 'text msg');
    svc.queryStatus({ from: ALICE, to: BOB, queryType: 'ping' });

    const textOnly = svc.getOutbox(ALICE, { type: 'text' });
    assert.equal(textOnly.length, 1);
    assert.equal(textOnly[0].type, 'text');
  });

  it('supports limit and offset', () => {
    sendText(svc, ALICE, BOB, 'a', { priority: 'critical' });
    sendText(svc, ALICE, BOB, 'b', { priority: 'critical' });
    sendText(svc, ALICE, BOB, 'c', { priority: 'critical' });

    const page = svc.getOutbox(ALICE, { limit: 2, offset: 1 });
    assert.equal(page.length, 2);
  });

  it('returns empty for agent with no sent messages', () => {
    assert.equal(svc.getOutbox(ALICE).length, 0);
  });
});

// ---------------------------------------------------------------------------
// 12. getMetrics computes correct counts
// ---------------------------------------------------------------------------

describe('Messaging — getMetrics', () => {
  let svc;
  beforeEach(() => {
    svc = makeService();
  });

  it('reports totalMessages and unreadCount', () => {
    sendText(svc, ALICE, BOB, 'one');
    sendText(svc, ALICE, BOB, 'two');
    const m3 = sendText(svc, ALICE, BOB, 'three');
    svc.markRead(m3.id);

    const metrics = svc.getMetrics();
    assert.equal(metrics.totalMessages, 3);
    assert.equal(metrics.unreadCount, 2);
  });

  it('reports zero avgResponseTimeMs when there are no replies', () => {
    sendText(svc, ALICE, BOB, 'standalone');
    const metrics = svc.getMetrics();
    assert.equal(metrics.avgResponseTimeMs, 0);
  });

  it('computes avgResponseTimeMs for threaded replies', () => {
    const parent = sendText(svc, ALICE, BOB, 'question');
    sendText(svc, BOB, ALICE, 'answer', { parentMessageId: parent.id });

    const metrics = svc.getMetrics();
    assert.equal(typeof metrics.avgResponseTimeMs, 'number');
    assert.ok(metrics.avgResponseTimeMs >= 0);
  });

  it('excludes expired messages from counts', () => {
    svc.sendMessage({
      from: ALICE,
      to: BOB,
      type: 'text',
      payload: { body: 'expired' },
      ttlMs: 1,
    });
    sendText(svc, ALICE, BOB, 'alive');

    const start = Date.now();
    while (Date.now() - start < 5) {
      // busy-wait
    }

    const metrics = svc.getMetrics();
    assert.equal(metrics.totalMessages, 1);
  });

  it('reports zeroes on empty service', () => {
    const metrics = svc.getMetrics();
    assert.equal(metrics.totalMessages, 0);
    assert.equal(metrics.unreadCount, 0);
    assert.equal(metrics.avgResponseTimeMs, 0);
  });
});

// ---------------------------------------------------------------------------
// 13. Priority ordering (critical > high > medium > low)
// ---------------------------------------------------------------------------

describe('Messaging — priority ordering', () => {
  let svc;
  beforeEach(() => {
    svc = makeService();
  });

  it('orders critical > high > medium > low in inbox', () => {
    sendText(svc, ALICE, BOB, 'low', { priority: 'low' });
    sendText(svc, ALICE, BOB, 'high', { priority: 'high' });
    sendText(svc, ALICE, BOB, 'critical', { priority: 'critical' });
    sendText(svc, ALICE, BOB, 'medium', { priority: 'medium' });

    const inbox = svc.getInbox(BOB);
    assert.equal(inbox.length, 4);
    assert.equal(inbox[0].priority, 'critical');
    assert.equal(inbox[1].priority, 'high');
    assert.equal(inbox[2].priority, 'medium');
    assert.equal(inbox[3].priority, 'low');
  });

  it('default priority is low', () => {
    const msg = sendText(svc, ALICE, BOB, 'default prio');
    assert.equal(msg.priority, 'low');
  });

  it('delegateTask inherits priority from params', () => {
    const task = svc.delegateTask({
      from: ALICE,
      to: BOB,
      description: 'critical task',
      deadline: new Date().toISOString(),
      reward: 100,
      priority: 'critical',
    });

    assert.equal(task.priority, 'critical');

    // Should appear first even if a low-priority text was sent earlier
    sendText(svc, CHARLIE, BOB, 'low prio', { priority: 'low' });
    const inbox = svc.getInbox(BOB);
    assert.equal(inbox[0].id, task.id);
  });

  it('messages of the same priority ordered by newest first', () => {
    const m1 = sendText(svc, ALICE, BOB, 'first', { priority: 'medium' });
    const m2 = sendText(svc, CHARLIE, BOB, 'second', { priority: 'medium' });
    const m3 = sendText(svc, ALICE, BOB, 'third', { priority: 'medium' });

    const inbox = svc.getInbox(BOB);
    assert.equal(inbox[0].id, m3.id);
    assert.equal(inbox[1].id, m2.id);
    assert.equal(inbox[2].id, m1.id);
  });
});

// ---------------------------------------------------------------------------
// 14. getMessage returns single message by ID
// ---------------------------------------------------------------------------

describe('Messaging — getMessage', () => {
  let svc;
  beforeEach(() => {
    svc = makeService();
  });

  it('returns the correct message', () => {
    const msg = sendText(svc, ALICE, BOB, 'findme');
    const found = svc.getMessage(msg.id);
    assert.equal(found.id, msg.id);
    assert.equal(found.payload.body, 'findme');
    assert.equal(found.from, ALICE);
    assert.equal(found.to, BOB);
  });

  it('returns null for unknown ID', () => {
    const found = svc.getMessage('nonexistent-id');
    assert.equal(found, null);
  });

  it('returns task_delegation messages with full payload', () => {
    const task = svc.delegateTask({
      from: ALICE,
      to: BOB,
      description: 'Ship widgets',
      deadline: '2026-12-31T23:59:59Z',
      reward: 75,
      priority: 'high',
    });

    const found = svc.getMessage(task.id);
    assert.equal(found.type, 'task_delegation');
    assert.equal(found.payload.description, 'Ship widgets');
    assert.equal(found.payload.reward, 75);
    assert.equal(found.priority, 'high');
  });

  it('throws for empty messageId', () => {
    assert.throws(
      () => svc.getMessage(''),
      /messageId must be a non-empty string/,
    );
  });
});

// ---------------------------------------------------------------------------
// Extra: edge cases and mixed scenarios
// ---------------------------------------------------------------------------

describe('Messaging — mixed scenarios', () => {
  let svc;
  beforeEach(() => {
    svc = makeService();
  });

  it('getInbox supports type filter', () => {
    sendText(svc, ALICE, BOB, 'hello');
    svc.queryStatus({ from: CHARLIE, to: BOB, queryType: 'ping' });

    const queries = svc.getInbox(BOB, { type: 'status_query' });
    assert.equal(queries.length, 1);
    assert.equal(queries[0].type, 'status_query');
  });

  it('supports all valid message types', () => {
    const types = [
      'text',
      'task_delegation',
      'status_query',
      'status_response',
      'data_request',
      'data_response',
    ];
    for (const type of types) {
      const msg = svc.sendMessage({
        from: ALICE,
        to: BOB,
        type,
        payload: { test: true },
      });
      assert.equal(msg.type, type);
    }
  });

  it('parentMessageId is stored on threaded messages', () => {
    const parent = sendText(svc, ALICE, BOB, 'parent');
    const reply = sendText(svc, BOB, ALICE, 'reply', {
      parentMessageId: parent.id,
    });

    assert.equal(reply.parentMessageId, parent.id);
    assert.equal(parent.parentMessageId, null);
  });

  it('custom TTL is respected', () => {
    const msg = svc.sendMessage({
      from: ALICE,
      to: BOB,
      type: 'text',
      payload: { body: 'custom ttl' },
      ttlMs: 60_000,
    });

    const created = new Date(msg.createdAt).getTime();
    const expires = new Date(msg.expiresAt).getTime();
    const diff = expires - created;
    // Allow 100ms tolerance for execution time
    assert.ok(Math.abs(diff - 60_000) < 100, `TTL diff should be ~60000ms, got ${diff}`);
  });

  it('respondToTask response appears in delegator inbox', () => {
    const task = svc.delegateTask({
      from: ALICE,
      to: BOB,
      description: 'Do work',
      deadline: new Date().toISOString(),
      reward: 10,
      priority: 'medium',
    });

    svc.respondToTask(task.id, { status: 'completed', result: { done: true } });

    const aliceInbox = svc.getInbox(ALICE);
    assert.equal(aliceInbox.length, 1);
    assert.equal(aliceInbox[0].type, 'status_response');
    assert.equal(aliceInbox[0].payload.status, 'completed');
  });

  it('getInbox with limit truncates results', () => {
    for (let i = 0; i < 10; i++) {
      sendText(svc, ALICE, BOB, `msg ${i}`);
    }

    const limited = svc.getInbox(BOB, { limit: 3 });
    assert.equal(limited.length, 3);
  });

  it('getInbox with offset skips results', () => {
    for (let i = 0; i < 5; i++) {
      sendText(svc, ALICE, BOB, `msg ${i}`, { priority: 'medium' });
    }

    const all = svc.getInbox(BOB);
    const skipped = svc.getInbox(BOB, { offset: 2 });
    assert.equal(skipped.length, 3);
    assert.equal(skipped[0].id, all[2].id);
  });
});
