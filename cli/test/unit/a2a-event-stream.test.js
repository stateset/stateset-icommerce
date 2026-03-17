/**
 * Tests for cli/src/a2a/event-stream.js
 *
 * Covers: subscribe, unsubscribe, listSubscriptions, pushEvent,
 * getEventHistory, handleSSEConnection, and matchesEventFilter
 * (tested indirectly through pushEvent).
 */

import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';

import { createEventStreamService } from '../../src/a2a/event-stream.js';

// ---------------------------------------------------------------------------
// Mock store factory
// ---------------------------------------------------------------------------

function createMockStore() {
  const subscriptions = new Map();
  const events = new Map();

  return {
    createEventSubscription: mock.fn(async (sub) => {
      const record = {
        ...sub,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };
      subscriptions.set(sub.id, record);
      return record;
    }),

    getEventSubscription: mock.fn(async (id) => {
      return subscriptions.get(id) || null;
    }),

    updateEventSubscription: mock.fn(async (id, updates) => {
      const existing = subscriptions.get(id);
      if (!existing) return null;
      const updated = {
        ...existing,
        ...updates,
        updated_at: new Date().toISOString(),
      };
      subscriptions.set(id, updated);
      return updated;
    }),

    listEventSubscriptions: mock.fn(async (filter) => {
      let results = [...subscriptions.values()];
      if (filter?.agent_address) {
        results = results.filter((s) => s.agent_address === filter.agent_address);
      }
      if (filter?.active !== undefined) {
        results = results.filter((s) => s.active === filter.active);
      }
      return results;
    }),

    createEventLog: mock.fn(async (entry) => {
      const record = {
        ...entry,
        created_at: new Date().toISOString(),
      };
      events.set(entry.id, record);
      return record;
    }),

    getEventLog: mock.fn(async (id) => {
      return events.get(id) || null;
    }),

    listEventLog: mock.fn(async (filter) => {
      let results = [...events.values()];
      if (filter?.agent_address) {
        results = results.filter((e) => e.agent_address === filter.agent_address);
      }
      if (filter?.event_type) {
        results = results.filter((e) => e.event_type === filter.event_type);
      }
      if (filter?.since) {
        results = results.filter((e) => e.created_at > filter.since);
      }
      if (filter?.limit) {
        results = results.slice(0, filter.limit);
      }
      return results;
    }),

    // Expose internals for assertions
    _subscriptions: subscriptions,
    _events: events,
  };
}

// ---------------------------------------------------------------------------
// SSE mock helpers
// ---------------------------------------------------------------------------

function createMockRes() {
  return {
    writeHead: mock.fn(),
    write: mock.fn(),
  };
}

function createMockReq(headers = {}) {
  const listeners = {};
  return {
    headers,
    on: mock.fn((event, cb) => {
      listeners[event] = cb;
    }),
    _listeners: listeners,
  };
}

// ---------------------------------------------------------------------------
// 1. subscribe
// ---------------------------------------------------------------------------

describe('EventStreamService.subscribe', () => {
  let store;
  let service;

  beforeEach(() => {
    store = createMockStore();
    service = createEventStreamService(store);
  });

  it('creates a subscription with specified eventTypes', async () => {
    const result = await service.subscribe({
      agentAddress: '0xAgent1',
      eventTypes: ['a2a_payment.created', 'a2a_escrow.released'],
    });

    assert.equal(result.success, true);
    assert.ok(result.subscription);
    assert.ok(result.subscription.id, 'should have a generated UUID');
    assert.equal(result.subscription.agentAddress, '0xAgent1');
    assert.deepStrictEqual(result.subscription.eventTypes, [
      'a2a_payment.created',
      'a2a_escrow.released',
    ]);
    assert.equal(result.subscription.active, true);
    assert.ok(result.subscription.createdAt);
  });

  it('defaults eventTypes to ["*"] when not provided', async () => {
    const result = await service.subscribe({
      agentAddress: '0xAgent2',
    });

    assert.equal(result.success, true);
    assert.deepStrictEqual(result.subscription.eventTypes, ['*']);
  });

  it('defaults eventTypes to ["*"] when explicitly undefined', async () => {
    const result = await service.subscribe({
      agentAddress: '0xAgent3',
      eventTypes: undefined,
    });

    assert.deepStrictEqual(result.subscription.eventTypes, ['*']);
  });

  it('calls store.createEventSubscription with correct shape', async () => {
    await service.subscribe({
      agentAddress: '0xAgent4',
      eventTypes: ['a2a_task.*'],
    });

    assert.equal(store.createEventSubscription.mock.calls.length, 1);
    const arg = store.createEventSubscription.mock.calls[0].arguments[0];
    assert.equal(arg.agent_address, '0xAgent4');
    assert.deepStrictEqual(arg.event_types, ['a2a_task.*']);
    assert.equal(arg.active, true);
    assert.ok(arg.id, 'should have an id');
  });

  it('throws when agentAddress is missing', async () => {
    await assert.rejects(() => service.subscribe({ eventTypes: ['*'] }), {
      message: 'agentAddress is required',
    });
  });

  it('throws when agentAddress is empty string', async () => {
    await assert.rejects(() => service.subscribe({ agentAddress: '', eventTypes: ['*'] }), {
      message: 'agentAddress is required',
    });
  });

  it('throws when eventTypes is not an array', async () => {
    await assert.rejects(
      () => service.subscribe({ agentAddress: '0xAgent', eventTypes: 'a2a_payment.*' }),
      { message: 'eventTypes must be an array' },
    );
  });

  it('generates unique IDs for different subscriptions', async () => {
    const r1 = await service.subscribe({ agentAddress: '0xA' });
    const r2 = await service.subscribe({ agentAddress: '0xB' });

    assert.notEqual(r1.subscription.id, r2.subscription.id);
  });
});

// ---------------------------------------------------------------------------
// 2. unsubscribe
// ---------------------------------------------------------------------------

describe('EventStreamService.unsubscribe', () => {
  let store;
  let service;

  beforeEach(() => {
    store = createMockStore();
    service = createEventStreamService(store);
  });

  it('deactivates an existing subscription', async () => {
    const { subscription } = await service.subscribe({
      agentAddress: '0xAgent',
      eventTypes: ['*'],
    });

    const result = await service.unsubscribe(subscription.id);

    assert.equal(result.success, true);
    assert.equal(result.subscription.active, false);
    assert.equal(result.subscription.id, subscription.id);
  });

  it('calls store.updateEventSubscription with active: false', async () => {
    const { subscription } = await service.subscribe({ agentAddress: '0xAgent' });

    await service.unsubscribe(subscription.id);

    // Find the updateEventSubscription call that sets active: false
    const updateCalls = store.updateEventSubscription.mock.calls;
    const deactivateCall = updateCalls.find(
      (c) => c.arguments[0] === subscription.id && c.arguments[1].active === false,
    );
    assert.ok(deactivateCall, 'should have called updateEventSubscription with active: false');
  });

  it('throws when subscription is not found', async () => {
    await assert.rejects(() => service.unsubscribe('nonexistent-id'), {
      message: 'Subscription not found',
    });
  });
});

// ---------------------------------------------------------------------------
// 3. pushEvent
// ---------------------------------------------------------------------------

describe('EventStreamService.pushEvent', () => {
  let store;
  let service;
  let cleanups;

  beforeEach(() => {
    store = createMockStore();
    service = createEventStreamService(store);
    cleanups = [];
  });

  afterEach(() => {
    for (const fn of cleanups) fn();
  });

  it('logs an event and returns formatted result', async () => {
    const result = await service.pushEvent({
      eventType: 'a2a_payment.created',
      agentAddress: '0xAgent',
      payload: { paymentId: 'pay-1', amount: 50 },
    });

    assert.ok(result.id);
    assert.equal(result.eventType, 'a2a_payment.created');
    assert.equal(result.agentAddress, '0xAgent');
    assert.deepStrictEqual(result.payload, { paymentId: 'pay-1', amount: 50 });
    assert.ok(result.createdAt);
  });

  it('calls store.createEventLog with snake_case fields', async () => {
    await service.pushEvent({
      eventType: 'a2a_task.completed',
      agentAddress: '0xAgent',
      payload: { task: 'abc' },
    });

    assert.equal(store.createEventLog.mock.calls.length, 1);
    const arg = store.createEventLog.mock.calls[0].arguments[0];
    assert.equal(arg.event_type, 'a2a_task.completed');
    assert.equal(arg.agent_address, '0xAgent');
    assert.deepStrictEqual(arg.payload, { task: 'abc' });
    assert.ok(arg.id);
  });

  it('defaults payload to {} when undefined', async () => {
    await service.pushEvent({
      eventType: 'a2a_ping',
      agentAddress: '0xAgent',
    });

    const arg = store.createEventLog.mock.calls[0].arguments[0];
    assert.deepStrictEqual(arg.payload, {});
  });

  it('updates last_event_id on matching subscriptions', async () => {
    // Create a subscription that matches the event
    await service.subscribe({
      agentAddress: '0xAgent',
      eventTypes: ['a2a_payment.*'],
    });

    const result = await service.pushEvent({
      eventType: 'a2a_payment.created',
      agentAddress: '0xAgent',
      payload: {},
    });

    // Find the update call that sets last_event_id
    const updateCalls = store.updateEventSubscription.mock.calls;
    const lastEventCall = updateCalls.find((c) => c.arguments[1]?.last_event_id === result.id);
    assert.ok(lastEventCall, 'should update last_event_id on matching subscription');
  });

  it('does NOT update last_event_id on non-matching subscriptions', async () => {
    // Subscribe to escrow events only
    await service.subscribe({
      agentAddress: '0xAgent',
      eventTypes: ['a2a_escrow.*'],
    });

    await service.pushEvent({
      eventType: 'a2a_payment.created',
      agentAddress: '0xAgent',
      payload: {},
    });

    // No updateEventSubscription calls should contain last_event_id
    const updateCalls = store.updateEventSubscription.mock.calls;
    const lastEventCall = updateCalls.find((c) => c.arguments[1]?.last_event_id);
    assert.equal(lastEventCall, undefined, 'should not update non-matching subscription');
  });

  it('notifies SSE clients when a matching connection exists', async () => {
    const mockReq = createMockReq();
    const mockRes = createMockRes();

    cleanups.push(service.handleSSEConnection(mockReq, mockRes, '0xAgent'));

    // Reset write calls from the initial connected event
    mockRes.write.mock.resetCalls();

    await service.pushEvent({
      eventType: 'a2a_payment.created',
      agentAddress: '0xAgent',
      payload: { amount: 100 },
    });

    assert.equal(mockRes.write.mock.calls.length, 1);
    const written = mockRes.write.mock.calls[0].arguments[0];
    assert.ok(written.includes('event: a2a_payment.created'));
    assert.ok(written.includes('"amount":100'));
  });

  it('does not fail when there are no SSE clients', async () => {
    // No SSE clients connected, should not throw
    const result = await service.pushEvent({
      eventType: 'a2a_payment.created',
      agentAddress: '0xAgent',
      payload: {},
    });

    assert.ok(result.id);
  });

  it('throws when eventType is missing', async () => {
    await assert.rejects(() => service.pushEvent({ agentAddress: '0xAgent', payload: {} }), {
      message: 'eventType is required',
    });
  });

  it('throws when agentAddress is missing', async () => {
    await assert.rejects(() => service.pushEvent({ eventType: 'a2a_test', payload: {} }), {
      message: 'agentAddress is required',
    });
  });

  it('handles store.updateEventSubscription failure gracefully', async () => {
    // Create a matching subscription
    await service.subscribe({
      agentAddress: '0xAgent',
      eventTypes: ['*'],
    });

    // Override updateEventSubscription to throw
    store.updateEventSubscription = mock.fn(async () => {
      throw new Error('DB write error');
    });

    // Should not throw despite update failure (logs console.warn)
    const result = await service.pushEvent({
      eventType: 'a2a_something',
      agentAddress: '0xAgent',
      payload: {},
    });

    assert.ok(result.id, 'event should still be created');
  });

  it('handles SSE write failure gracefully', async () => {
    const mockReq = createMockReq();
    const mockRes = createMockRes();

    cleanups.push(service.handleSSEConnection(mockReq, mockRes, '0xAgent'));

    // Override write to throw
    mockRes.write = mock.fn(() => {
      throw new Error('Connection reset');
    });

    // Should not throw despite SSE write failure (logs console.warn)
    const result = await service.pushEvent({
      eventType: 'a2a_task.done',
      agentAddress: '0xAgent',
      payload: {},
    });

    assert.ok(result.id, 'event should still be created');
  });
});

// ---------------------------------------------------------------------------
// 4. getEventHistory
// ---------------------------------------------------------------------------

describe('EventStreamService.getEventHistory', () => {
  let store;
  let service;

  beforeEach(() => {
    store = createMockStore();
    service = createEventStreamService(store);
  });

  it('returns formatted events for an agent', async () => {
    await service.pushEvent({
      eventType: 'a2a_payment.created',
      agentAddress: '0xAgent',
      payload: { amount: 10 },
    });
    await service.pushEvent({
      eventType: 'a2a_payment.settled',
      agentAddress: '0xAgent',
      payload: { amount: 10 },
    });

    const history = await service.getEventHistory({ agentAddress: '0xAgent' });

    assert.equal(history.length, 2);
    assert.equal(history[0].eventType, 'a2a_payment.created');
    assert.equal(history[1].eventType, 'a2a_payment.settled');
    assert.ok(history[0].id);
    assert.ok(history[0].createdAt);
  });

  it('filters by eventType (first entry)', async () => {
    await service.pushEvent({
      eventType: 'a2a_payment.created',
      agentAddress: '0xAgent',
      payload: {},
    });
    await service.pushEvent({
      eventType: 'a2a_escrow.created',
      agentAddress: '0xAgent',
      payload: {},
    });

    const history = await service.getEventHistory({
      agentAddress: '0xAgent',
      eventTypes: ['a2a_payment.created'],
    });

    assert.equal(history.length, 1);
    assert.equal(history[0].eventType, 'a2a_payment.created');
  });

  it('passes since parameter to store', async () => {
    const since = '2026-01-01T00:00:00Z';
    await service.getEventHistory({
      agentAddress: '0xAgent',
      since,
    });

    assert.equal(store.listEventLog.mock.calls.length, 1);
    const filter = store.listEventLog.mock.calls[0].arguments[0];
    assert.equal(filter.since, since);
  });

  it('passes limit parameter to store', async () => {
    await service.getEventHistory({
      agentAddress: '0xAgent',
      limit: 5,
    });

    const filter = store.listEventLog.mock.calls[0].arguments[0];
    assert.equal(filter.limit, 5);
  });

  it('returns empty array when no events exist', async () => {
    const history = await service.getEventHistory({ agentAddress: '0xNobody' });
    assert.deepStrictEqual(history, []);
  });

  it('throws when agentAddress is missing', async () => {
    await assert.rejects(() => service.getEventHistory({ limit: 10 }), {
      message: 'agentAddress is required',
    });
  });

  it('parses JSON string payload from store', async () => {
    // Directly insert a record with a string payload into the store
    store._events.set('evt-json', {
      id: 'evt-json',
      event_type: 'a2a_test',
      agent_address: '0xAgent',
      payload: '{"key":"value"}',
      created_at: new Date().toISOString(),
    });

    const history = await service.getEventHistory({ agentAddress: '0xAgent' });
    assert.equal(history.length, 1);
    assert.deepStrictEqual(history[0].payload, { key: 'value' });
  });

  it('keeps non-JSON string payload as-is', async () => {
    store._events.set('evt-raw', {
      id: 'evt-raw',
      event_type: 'a2a_test',
      agent_address: '0xAgent',
      payload: 'not-json-content',
      created_at: new Date().toISOString(),
    });

    const history = await service.getEventHistory({ agentAddress: '0xAgent' });
    assert.equal(history.length, 1);
    assert.equal(history[0].payload, 'not-json-content');
  });
});

// ---------------------------------------------------------------------------
// 5. listSubscriptions
// ---------------------------------------------------------------------------

describe('EventStreamService.listSubscriptions', () => {
  let store;
  let service;

  beforeEach(() => {
    store = createMockStore();
    service = createEventStreamService(store);
  });

  it('returns active subscriptions for an agent', async () => {
    await service.subscribe({ agentAddress: '0xAgent', eventTypes: ['*'] });
    await service.subscribe({ agentAddress: '0xAgent', eventTypes: ['a2a_payment.*'] });

    const subs = await service.listSubscriptions({ agentAddress: '0xAgent' });

    assert.equal(subs.length, 2);
    subs.forEach((s) => {
      assert.equal(s.agentAddress, '0xAgent');
      assert.equal(s.active, true);
    });
  });

  it('does not return deactivated subscriptions', async () => {
    const { subscription } = await service.subscribe({
      agentAddress: '0xAgent',
      eventTypes: ['*'],
    });
    await service.unsubscribe(subscription.id);

    const subs = await service.listSubscriptions({ agentAddress: '0xAgent' });
    assert.equal(subs.length, 0);
  });

  it('returns empty array for unknown agent', async () => {
    const subs = await service.listSubscriptions({ agentAddress: '0xUnknown' });
    assert.deepStrictEqual(subs, []);
  });

  it('only returns subscriptions for the requested agent', async () => {
    await service.subscribe({ agentAddress: '0xA' });
    await service.subscribe({ agentAddress: '0xB' });

    const subsA = await service.listSubscriptions({ agentAddress: '0xA' });
    assert.equal(subsA.length, 1);
    assert.equal(subsA[0].agentAddress, '0xA');
  });

  it('formats subscriptions with camelCase fields', async () => {
    await service.subscribe({
      agentAddress: '0xAgent',
      eventTypes: ['a2a_task.*'],
    });

    const subs = await service.listSubscriptions({ agentAddress: '0xAgent' });
    const sub = subs[0];

    assert.ok(sub.id);
    assert.equal(sub.agentAddress, '0xAgent');
    assert.deepStrictEqual(sub.eventTypes, ['a2a_task.*']);
    assert.equal(sub.active, true);
    assert.equal(sub.lastEventId, null);
    assert.ok(sub.createdAt);
    assert.ok(sub.updatedAt);
  });
});

// ---------------------------------------------------------------------------
// 6. handleSSEConnection
// ---------------------------------------------------------------------------

describe('EventStreamService.handleSSEConnection', () => {
  let store;
  let service;
  let timers;

  beforeEach(() => {
    store = createMockStore();
    service = createEventStreamService(store);
    timers = [];
    // Capture setInterval calls so we can clean up
    const originalSetInterval = globalThis.setInterval;
    mock.method(globalThis, 'setInterval', (...args) => {
      const id = originalSetInterval(...args);
      timers.push(id);
      return id;
    });
  });

  afterEach(() => {
    // Clean up any dangling intervals
    for (const id of timers) {
      clearInterval(id);
    }
    globalThis.setInterval.mock.restore();
  });

  it('sets SSE response headers', () => {
    const mockReq = createMockReq();
    const mockRes = createMockRes();

    service.handleSSEConnection(mockReq, mockRes, '0xAgent');

    assert.equal(mockRes.writeHead.mock.calls.length, 1);
    const [statusCode, headers] = mockRes.writeHead.mock.calls[0].arguments;
    assert.equal(statusCode, 200);
    assert.equal(headers['Content-Type'], 'text/event-stream');
    assert.equal(headers['Cache-Control'], 'no-cache');
    assert.equal(headers['Connection'], 'keep-alive');
    assert.equal(headers['X-Accel-Buffering'], 'no');
  });

  it('sends connected event on initial connection', () => {
    const mockReq = createMockReq();
    const mockRes = createMockRes();

    service.handleSSEConnection(mockReq, mockRes, '0xAgent');

    assert.equal(mockRes.write.mock.calls.length, 1);
    const written = mockRes.write.mock.calls[0].arguments[0];
    assert.ok(written.startsWith('event: connected\n'));
    assert.ok(written.includes('"agentAddress":"0xAgent"'));
    assert.ok(written.endsWith('\n\n'));
  });

  it('registers a close listener on the request', () => {
    const mockReq = createMockReq();
    const mockRes = createMockRes();

    service.handleSSEConnection(mockReq, mockRes, '0xAgent');

    assert.equal(mockReq.on.mock.calls.length, 1);
    assert.equal(mockReq.on.mock.calls[0].arguments[0], 'close');
    assert.equal(typeof mockReq.on.mock.calls[0].arguments[1], 'function');
  });

  it('returns a cleanup function', () => {
    const mockReq = createMockReq();
    const mockRes = createMockRes();

    const cleanup = service.handleSSEConnection(mockReq, mockRes, '0xAgent');
    assert.equal(typeof cleanup, 'function');
  });

  it('cleanup removes SSE client so pushEvent does not notify', async () => {
    const mockReq = createMockReq();
    const mockRes = createMockRes();

    const cleanup = service.handleSSEConnection(mockReq, mockRes, '0xAgent');
    mockRes.write.mock.resetCalls();

    // Cleanup (simulates disconnect)
    cleanup();

    await service.pushEvent({
      eventType: 'a2a_test',
      agentAddress: '0xAgent',
      payload: {},
    });

    // No writes after cleanup
    assert.equal(mockRes.write.mock.calls.length, 0);
  });

  it('supports multiple SSE clients for the same agent', async () => {
    const req1 = createMockReq();
    const res1 = createMockRes();
    const req2 = createMockReq();
    const res2 = createMockRes();

    service.handleSSEConnection(req1, res1, '0xAgent');
    service.handleSSEConnection(req2, res2, '0xAgent');

    res1.write.mock.resetCalls();
    res2.write.mock.resetCalls();

    await service.pushEvent({
      eventType: 'a2a_broadcast',
      agentAddress: '0xAgent',
      payload: { msg: 'hello' },
    });

    assert.equal(res1.write.mock.calls.length, 1);
    assert.equal(res2.write.mock.calls.length, 1);
  });

  it('cleanup of one client does not remove others', async () => {
    const req1 = createMockReq();
    const res1 = createMockRes();
    const req2 = createMockReq();
    const res2 = createMockRes();

    const cleanup1 = service.handleSSEConnection(req1, res1, '0xAgent');
    service.handleSSEConnection(req2, res2, '0xAgent');

    cleanup1();
    res2.write.mock.resetCalls();

    await service.pushEvent({
      eventType: 'a2a_test',
      agentAddress: '0xAgent',
      payload: {},
    });

    // res1 should NOT receive (cleaned up), res2 should receive
    assert.equal(res2.write.mock.calls.length, 1);
  });

  it('close event on req triggers cleanup', async () => {
    const mockReq = createMockReq();
    const mockRes = createMockRes();

    service.handleSSEConnection(mockReq, mockRes, '0xAgent');
    mockRes.write.mock.resetCalls();

    // Simulate client disconnect
    const closeHandler = mockReq._listeners['close'];
    assert.ok(closeHandler, 'should have registered a close handler');
    closeHandler();

    await service.pushEvent({
      eventType: 'a2a_test',
      agentAddress: '0xAgent',
      payload: {},
    });

    assert.equal(mockRes.write.mock.calls.length, 0);
  });
});

// ---------------------------------------------------------------------------
// 7. matchesEventFilter (tested indirectly through pushEvent)
// ---------------------------------------------------------------------------

describe('matchesEventFilter (via pushEvent)', () => {
  let store;
  let service;

  beforeEach(() => {
    store = createMockStore();
    service = createEventStreamService(store);
  });

  it('wildcard "*" matches any event type', async () => {
    await service.subscribe({
      agentAddress: '0xAgent',
      eventTypes: ['*'],
    });

    const result = await service.pushEvent({
      eventType: 'some.random.event',
      agentAddress: '0xAgent',
      payload: {},
    });

    const updateCalls = store.updateEventSubscription.mock.calls;
    const lastEventCall = updateCalls.find((c) => c.arguments[1]?.last_event_id === result.id);
    assert.ok(lastEventCall, 'wildcard should match any event type');
  });

  it('exact match works for specific event type', async () => {
    await service.subscribe({
      agentAddress: '0xAgent',
      eventTypes: ['a2a_payment.created'],
    });

    const result = await service.pushEvent({
      eventType: 'a2a_payment.created',
      agentAddress: '0xAgent',
      payload: {},
    });

    const updateCalls = store.updateEventSubscription.mock.calls;
    const lastEventCall = updateCalls.find((c) => c.arguments[1]?.last_event_id === result.id);
    assert.ok(lastEventCall, 'exact match should match');
  });

  it('prefix wildcard "a2a_payment.*" matches "a2a_payment.created"', async () => {
    await service.subscribe({
      agentAddress: '0xAgent',
      eventTypes: ['a2a_payment.*'],
    });

    const result = await service.pushEvent({
      eventType: 'a2a_payment.created',
      agentAddress: '0xAgent',
      payload: {},
    });

    const updateCalls = store.updateEventSubscription.mock.calls;
    const lastEventCall = updateCalls.find((c) => c.arguments[1]?.last_event_id === result.id);
    assert.ok(lastEventCall, 'prefix wildcard should match');
  });

  it('prefix wildcard "a2a_payment.*" matches "a2a_payment.settled"', async () => {
    await service.subscribe({
      agentAddress: '0xAgent',
      eventTypes: ['a2a_payment.*'],
    });

    const result = await service.pushEvent({
      eventType: 'a2a_payment.settled',
      agentAddress: '0xAgent',
      payload: {},
    });

    const updateCalls = store.updateEventSubscription.mock.calls;
    const lastEventCall = updateCalls.find((c) => c.arguments[1]?.last_event_id === result.id);
    assert.ok(lastEventCall, 'prefix wildcard should match different suffix');
  });

  it('prefix wildcard "a2a_payment.*" does NOT match "a2a_escrow.created"', async () => {
    await service.subscribe({
      agentAddress: '0xAgent',
      eventTypes: ['a2a_payment.*'],
    });

    await service.pushEvent({
      eventType: 'a2a_escrow.created',
      agentAddress: '0xAgent',
      payload: {},
    });

    const updateCalls = store.updateEventSubscription.mock.calls;
    const lastEventCall = updateCalls.find((c) => c.arguments[1]?.last_event_id);
    assert.equal(lastEventCall, undefined, 'prefix wildcard should not match different prefix');
  });

  it('exact match does not match partial event type', async () => {
    await service.subscribe({
      agentAddress: '0xAgent',
      eventTypes: ['a2a_payment'],
    });

    await service.pushEvent({
      eventType: 'a2a_payment.created',
      agentAddress: '0xAgent',
      payload: {},
    });

    const updateCalls = store.updateEventSubscription.mock.calls;
    const lastEventCall = updateCalls.find((c) => c.arguments[1]?.last_event_id);
    assert.equal(lastEventCall, undefined, 'exact match should not match longer event type');
  });

  it('multiple filters: matches if any filter matches', async () => {
    await service.subscribe({
      agentAddress: '0xAgent',
      eventTypes: ['a2a_escrow.*', 'a2a_payment.created'],
    });

    const result = await service.pushEvent({
      eventType: 'a2a_payment.created',
      agentAddress: '0xAgent',
      payload: {},
    });

    const updateCalls = store.updateEventSubscription.mock.calls;
    const lastEventCall = updateCalls.find((c) => c.arguments[1]?.last_event_id === result.id);
    assert.ok(lastEventCall, 'should match on second filter');
  });

  it('no filters match when subscription eventTypes is empty array', async () => {
    // Directly insert a subscription with empty event_types
    store._subscriptions.set('sub-empty', {
      id: 'sub-empty',
      agent_address: '0xAgent',
      event_types: [],
      active: true,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });

    await service.pushEvent({
      eventType: 'a2a_anything',
      agentAddress: '0xAgent',
      payload: {},
    });

    const updateCalls = store.updateEventSubscription.mock.calls;
    const lastEventCall = updateCalls.find((c) => c.arguments[1]?.last_event_id);
    assert.equal(lastEventCall, undefined, 'empty filter array should not match anything');
  });
});

// ---------------------------------------------------------------------------
// 8. SSE message format
// ---------------------------------------------------------------------------

describe('SSE message format', () => {
  let store;
  let service;
  let timers;

  beforeEach(() => {
    store = createMockStore();
    service = createEventStreamService(store);
    timers = [];
    const originalSetInterval = globalThis.setInterval;
    mock.method(globalThis, 'setInterval', (...args) => {
      const id = originalSetInterval(...args);
      timers.push(id);
      return id;
    });
  });

  afterEach(() => {
    for (const id of timers) {
      clearInterval(id);
    }
    globalThis.setInterval.mock.restore();
  });

  it('pushEvent sends SSE in correct format: id + event + data + double newline', async () => {
    const mockReq = createMockReq();
    const mockRes = createMockRes();

    service.handleSSEConnection(mockReq, mockRes, '0xAgent');
    mockRes.write.mock.resetCalls();

    await service.pushEvent({
      eventType: 'a2a_payment.created',
      agentAddress: '0xAgent',
      payload: { id: 'p1' },
    });

    const msg = mockRes.write.mock.calls[0].arguments[0];
    const lines = msg.split('\n');
    assert.match(lines[0], /^id: /);
    assert.equal(lines[1], 'event: a2a_payment.created');
    assert.equal(lines[2], 'data: {"id":"p1"}');
    assert.equal(lines[3], '');
    assert.equal(lines[4], '');
  });

  it('pushEvent with undefined payload sends {} as SSE data', async () => {
    const mockReq = createMockReq();
    const mockRes = createMockRes();

    service.handleSSEConnection(mockReq, mockRes, '0xAgent');
    mockRes.write.mock.resetCalls();

    await service.pushEvent({
      eventType: 'a2a_ping',
      agentAddress: '0xAgent',
    });

    const msg = mockRes.write.mock.calls[0].arguments[0];
    assert.ok(msg.includes('data: {}'), 'undefined payload should serialize as {}');
  });
});

// ---------------------------------------------------------------------------
// 9. formatSubscription / formatEvent edge cases
// ---------------------------------------------------------------------------

describe('formatSubscription / formatEvent edge cases', () => {
  let store;
  let service;

  beforeEach(() => {
    store = createMockStore();
    service = createEventStreamService(store);
  });

  it('lastEventId defaults to null when last_event_id is missing', async () => {
    const { subscription } = await service.subscribe({
      agentAddress: '0xAgent',
      eventTypes: ['*'],
    });

    assert.equal(subscription.lastEventId, null);
  });

  it('formatEvent handles null row from store gracefully (via getEventHistory)', async () => {
    // Override listEventLog to return a null entry (edge case)
    store.listEventLog = mock.fn(async () => [null]);

    const history = await service.getEventHistory({ agentAddress: '0xAgent' });
    assert.equal(history.length, 1);
    assert.equal(history[0], null);
  });
});
