/**
 * Tests for SSE reconnection features in cli/src/a2a/event-stream.js
 *
 * Focuses on: Last-Event-ID replay, event ID in SSE messages,
 * and skipping the event that the client already has.
 * Uses mock store to avoid native module dependency.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { createEventStreamService } from '../../src/a2a/event-stream.js';

function createMockStore() {
  const subscriptions = [];
  const events = [];

  return {
    createEventSubscription: async (data) => {
      subscriptions.push(data);
      return data;
    },
    getEventSubscription: async (id) => subscriptions.find((s) => s.id === id) || null,
    updateEventSubscription: async (id, updates) => {
      const s = subscriptions.find((sub) => sub.id === id);
      if (s) Object.assign(s, updates);
      return s;
    },
    listEventSubscriptions: async (filter) =>
      subscriptions.filter(
        (s) =>
          (!filter?.agent_address || s.agent_address === filter.agent_address) &&
          (filter?.active === undefined || s.active === filter.active),
      ),
    createEventLog: async (data) => {
      const evt = { ...data, created_at: data.created_at || new Date().toISOString() };
      events.push(evt);
      return evt;
    },
    getEventLog: async (id) => events.find((e) => e.id === id) || null,
    listEventLog: async (filter) => {
      let result = events.filter(
        (e) =>
          (!filter?.agent_address || e.agent_address === filter.agent_address) &&
          (!filter?.event_type || e.event_type === filter.event_type) &&
          (!filter?.since || e.created_at > filter.since),
      );
      if (filter?.limit) result = result.slice(0, filter.limit);
      return result;
    },
  };
}

function createMockResponse() {
  const written = [];
  return {
    writeHead: () => {},
    write: (data) => {
      written.push(data);
      return true;
    },
    end: () => {},
    _written: written,
  };
}

function createMockRequest(headers = {}) {
  const listeners = {};
  return {
    headers,
    on: (event, fn) => {
      listeners[event] = fn;
    },
    _listeners: listeners,
  };
}

describe('SSE event ID in pushEvent', () => {
  let store;
  let stream;

  beforeEach(() => {
    store = createMockStore();
    stream = createEventStreamService(store);
  });

  it('includes event ID in SSE messages sent to clients', async () => {
    const req = createMockRequest();
    const res = createMockResponse();
    const cleanup = stream.handleSSEConnection(req, res, '0xAgent');

    const evt = await stream.pushEvent({
      eventType: 'a2a_payment.created',
      agentAddress: '0xAgent',
      payload: { amount: 50 },
    });

    const eventMsgs = res._written.filter((m) => m.includes('a2a_payment.created'));
    assert.ok(eventMsgs.length > 0, 'Should have sent event to SSE client');

    const msg = eventMsgs[0];
    assert.ok(msg.includes('id:'), 'SSE message should include event ID');
    assert.ok(msg.includes('event: a2a_payment.created'));
    assert.ok(msg.includes('"amount":50'));

    cleanup();
  });
});

describe('SSE Last-Event-ID replay', () => {
  let store;
  let stream;

  beforeEach(() => {
    store = createMockStore();
    stream = createEventStreamService(store);
  });

  it('replays missed events on reconnection with Last-Event-ID', async () => {
    // Manually insert events with staggered timestamps so since filter works
    const baseTime = new Date('2026-03-16T08:00:00Z');

    await store.createEventLog({
      id: 'replay-evt-1',
      event_type: 'a2a_payment.created',
      agent_address: '0xAgent',
      payload: { id: 1 },
      created_at: new Date(baseTime.getTime()).toISOString(),
    });
    await store.createEventLog({
      id: 'replay-evt-2',
      event_type: 'a2a_escrow.released',
      agent_address: '0xAgent',
      payload: { id: 2 },
      created_at: new Date(baseTime.getTime() + 1000).toISOString(),
    });
    await store.createEventLog({
      id: 'replay-evt-3',
      event_type: 'a2a_payment.completed',
      agent_address: '0xAgent',
      payload: { id: 3 },
      created_at: new Date(baseTime.getTime() + 2000).toISOString(),
    });

    const req = createMockRequest({ 'last-event-id': 'replay-evt-1' });
    const res = createMockResponse();
    const cleanup = stream.handleSSEConnection(req, res, '0xAgent');

    await new Promise((r) => setTimeout(r, 200));

    const replayedEvents = res._written.filter(
      (m) => !m.includes('event: connected') && !m.includes('event: heartbeat') && m.includes('event: a2a_'),
    );
    assert.ok(replayedEvents.length >= 2, `Expected at least 2 replayed events, got ${replayedEvents.length}`);

    cleanup();
  });

  it('handles reconnection with unknown Last-Event-ID gracefully', async () => {
    const req = createMockRequest({ 'last-event-id': 'nonexistent' });
    const res = createMockResponse();
    const cleanup = stream.handleSSEConnection(req, res, '0xAgent');

    await new Promise((r) => setTimeout(r, 50));

    assert.ok(res._written.some((m) => m.includes('connected')));

    cleanup();
  });

  it('does not replay when no Last-Event-ID header', async () => {
    await stream.pushEvent({
      eventType: 'a2a_payment.created',
      agentAddress: '0xAgent',
      payload: { id: 1 },
    });

    const req = createMockRequest({});
    const res = createMockResponse();
    const cleanup = stream.handleSSEConnection(req, res, '0xAgent');

    await new Promise((r) => setTimeout(r, 50));

    const paymentMsgs = res._written.filter((m) => m.includes('a2a_payment'));
    assert.equal(paymentMsgs.length, 0, 'Should not replay without Last-Event-ID');

    cleanup();
  });
});

describe('SSE replay skips already-received event', () => {
  let store;
  let stream;

  beforeEach(() => {
    store = createMockStore();
    stream = createEventStreamService(store);
  });

  it('does not replay the Last-Event-ID event itself', async () => {
    // Push 3 events with staggered timestamps
    const baseTime = new Date('2026-03-16T10:00:00Z');

    // Manually insert events with controlled timestamps so since filter works
    await store.createEventLog({
      id: 'evt-001',
      event_type: 'a2a_payment.created',
      agent_address: '0xAgent',
      payload: { n: 1 },
      created_at: new Date(baseTime.getTime()).toISOString(),
    });
    await store.createEventLog({
      id: 'evt-002',
      event_type: 'a2a_payment.confirmed',
      agent_address: '0xAgent',
      payload: { n: 2 },
      created_at: new Date(baseTime.getTime() + 1000).toISOString(),
    });
    await store.createEventLog({
      id: 'evt-003',
      event_type: 'a2a_payment.settled',
      agent_address: '0xAgent',
      payload: { n: 3 },
      created_at: new Date(baseTime.getTime() + 2000).toISOString(),
    });

    // Reconnect with Last-Event-ID = evt-001
    const req = createMockRequest({ 'last-event-id': 'evt-001' });
    const res = createMockResponse();
    const cleanup = stream.handleSSEConnection(req, res, '0xAgent');

    await new Promise((r) => setTimeout(r, 100));

    const replayed = res._written.filter(
      (m) => !m.includes('event: connected') && !m.includes('event: heartbeat'),
    );

    // Should NOT include evt-001 (the client already has it)
    const hasEvt1 = replayed.some((m) => m.includes('evt-001'));
    assert.equal(hasEvt1, false, 'should NOT replay evt-001 (client already has it)');

    // Should include evt-002 and evt-003
    const hasEvt2 = replayed.some((m) => m.includes('evt-002'));
    const hasEvt3 = replayed.some((m) => m.includes('evt-003'));
    assert.equal(hasEvt2, true, 'should replay evt-002');
    assert.equal(hasEvt3, true, 'should replay evt-003');

    cleanup();
  });

  it('replayed messages include id, event, and data fields', async () => {
    const baseTime = new Date('2026-03-16T11:00:00Z');

    await store.createEventLog({
      id: 'skip-1',
      event_type: 'a2a_task.started',
      agent_address: '0xAgent',
      payload: JSON.stringify({ taskId: 't1' }),
      created_at: new Date(baseTime.getTime()).toISOString(),
    });
    await store.createEventLog({
      id: 'skip-2',
      event_type: 'a2a_task.completed',
      agent_address: '0xAgent',
      payload: JSON.stringify({ taskId: 't1', result: 'ok' }),
      created_at: new Date(baseTime.getTime() + 1000).toISOString(),
    });

    const req = createMockRequest({ 'last-event-id': 'skip-1' });
    const res = createMockResponse();
    const cleanup = stream.handleSSEConnection(req, res, '0xAgent');

    await new Promise((r) => setTimeout(r, 100));

    const replayed = res._written.filter(
      (m) => !m.includes('event: connected') && !m.includes('event: heartbeat'),
    );

    const evt2Write = replayed.find((m) => m.includes('skip-2'));
    assert.ok(evt2Write, 'should have replayed skip-2');
    assert.ok(evt2Write.includes('id: skip-2'), 'should include id field');
    assert.ok(evt2Write.includes('event: a2a_task.completed'), 'should include event type');
    assert.ok(evt2Write.includes('data:'), 'should include data field');

    cleanup();
  });

  it('replays nothing when only the Last-Event-ID event exists', async () => {
    await store.createEventLog({
      id: 'only-one',
      event_type: 'a2a_test.single',
      agent_address: '0xAgent',
      payload: { only: true },
      created_at: new Date('2026-03-16T12:00:00Z').toISOString(),
    });

    const req = createMockRequest({ 'last-event-id': 'only-one' });
    const res = createMockResponse();
    const cleanup = stream.handleSSEConnection(req, res, '0xAgent');

    await new Promise((r) => setTimeout(r, 100));

    const replayed = res._written.filter(
      (m) => !m.includes('event: connected') && !m.includes('event: heartbeat'),
    );

    const hasOnlyOne = replayed.some((m) => m.includes('only-one'));
    assert.equal(hasOnlyOne, false, 'should NOT replay the only event (client already has it)');

    cleanup();
  });
});
