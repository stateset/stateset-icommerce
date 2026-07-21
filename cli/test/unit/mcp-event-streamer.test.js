/**
 * Unit tests for cli/src/mcp-event-streamer.js
 */

import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';

import { createMcpEventStreamer } from '../../src/mcp-event-streamer.js';

function createMockRes() {
  return {
    writeHead: mock.fn(),
    write: mock.fn(),
  };
}

function createMockReq() {
  const listeners = {};
  return {
    on: mock.fn((event, handler) => {
      listeners[event] = handler;
    }),
    _listeners: listeners,
  };
}

describe('createMcpEventStreamer', () => {
  let service;
  let cleanups;

  beforeEach(() => {
    service = createMcpEventStreamer({ historyLimit: 20 });
    cleanups = [];
  });

  afterEach(() => {
    for (const cleanup of cleanups) {
      cleanup();
    }
    service.clear();
  });

  it('publishes events with normalized payload fields', () => {
    const event = service.publish({
      status: 'success',
      tool: 'create_order',
      sessionId: 'session-1',
      requestId: 'req-123',
      result: { ok: true },
    });

    assert.equal(typeof event.id, 'string');
    assert.equal(event.type, 'success');
    assert.equal(event.status, 'success');
    assert.equal(event.tool, 'create_order');
    assert.equal(event.sessionId, 'session-1');
    assert.equal(event.requestId, 'req-123');
  });

  it('routes events to matching session and global SSE clients without duplicate global fanout', async () => {
    const sessionReq = createMockReq();
    const sessionRes = createMockRes();
    const globalReq = createMockReq();
    const globalRes = createMockRes();

    const sessionSub = await service.subscribe({
      sessionId: 'session-1',
      eventTypes: ['success'],
    });
    const globalSub = await service.subscribe({
      eventTypes: ['success'],
    });

    cleanups.push(
      service.handleSSEConnection(sessionReq, sessionRes, {
        sessionId: 'session-1',
        subscriptionId: sessionSub.subscription.id,
      }),
    );
    cleanups.push(
      service.handleSSEConnection(globalReq, globalRes, {
        subscriptionId: globalSub.subscription.id,
      }),
    );

    const event = service.publish({
      status: 'success',
      sessionId: 'session-1',
    });

    assert.equal(event.id.length > 10, true);
    // Each connection writes a connected event once, then one streamed event each.
    assert.equal(sessionRes.write.mock.calls.length, 2);
    assert.equal(globalRes.write.mock.calls.length, 2);

    const hasSessionPayload = sessionRes.write.mock.calls[1].arguments[0].includes(event.id);
    const hasGlobalPayload = globalRes.write.mock.calls[1].arguments[0].includes(event.id);
    assert.equal(hasSessionPayload, true);
    assert.equal(hasGlobalPayload, true);

    // One session + global subscriptions both match this event, but clients should only see one event each.
    assert.equal(sessionSub.subscription.id.length > 0, true);
    assert.equal(globalSub.subscription.id.length > 0, true);
  });

  it('enforces event filters per SSE connection even when session ids match', async () => {
    const successSub = await service.subscribe({
      sessionId: 'session-1',
      eventTypes: ['success'],
    });
    const errorSub = await service.subscribe({
      sessionId: 'session-1',
      eventTypes: ['error'],
    });
    const successReq = createMockReq();
    const successRes = createMockRes();
    const errorReq = createMockReq();
    const errorRes = createMockRes();

    cleanups.push(
      service.handleSSEConnection(successReq, successRes, {
        sessionId: 'session-1',
        subscriptionId: successSub.subscription.id,
      }),
    );
    cleanups.push(
      service.handleSSEConnection(errorReq, errorRes, {
        sessionId: 'session-1',
        subscriptionId: errorSub.subscription.id,
      }),
    );

    const successEvent = service.publish({
      status: 'success',
      sessionId: 'session-1',
    });
    const errorEvent = service.publish({
      status: 'error',
      sessionId: 'session-1',
    });

    assert.equal(successRes.write.mock.calls.length, 2);
    assert.equal(errorRes.write.mock.calls.length, 2);
    assert.equal(successRes.write.mock.calls[1].arguments[0].includes(successEvent.id), true);
    assert.equal(successRes.write.mock.calls[1].arguments[0].includes(errorEvent.id), false);
    assert.equal(errorRes.write.mock.calls[1].arguments[0].includes(errorEvent.id), true);
    assert.equal(errorRes.write.mock.calls[1].arguments[0].includes(successEvent.id), false);
  });

  it('returns global subscriptions when no session filter is provided', async () => {
    const globalSub = await service.subscribe({ eventTypes: ['*'] });
    await service.subscribe({ sessionId: 'session-1', eventTypes: ['*'] });
    await service.subscribe({ sessionId: 'session-2', eventTypes: ['*'] });

    const subscriptions = await service.listSubscriptions();
    assert.equal(subscriptions.length, 1);
    assert.equal(subscriptions[0].id, globalSub.subscription.id);
    assert.equal(subscriptions[0].sessionId, '__global__');
  });

  it('filters subscriptions by session id', async () => {
    const sessionOne = await service.subscribe({ sessionId: 'session-1', eventTypes: ['*'] });
    await service.subscribe({ sessionId: 'session-2', eventTypes: ['*'] });

    const subscriptions = await service.listSubscriptions({ sessionId: 'session-1' });
    assert.equal(subscriptions.length, 1);
    assert.equal(subscriptions[0].id, sessionOne.subscription.id);
  });

  it('stores and retrieves event history with session and type filters', async () => {
    service.publish({ status: 'order.created', sessionId: 'session-1' });
    service.publish({ status: 'payment.failed', sessionId: 'session-2' });
    service.publish({ status: 'order.updated', sessionId: 'session-1' });

    const sessionOneHistory = await service.getEventHistory({
      sessionId: 'session-1',
      eventTypes: ['order.*'],
    });
    assert.equal(sessionOneHistory.length, 2);
    assert.equal(
      sessionOneHistory.every((event) => event.sessionId === 'session-1'),
      true,
    );
    assert.equal(
      sessionOneHistory.every((event) => event.type.startsWith('order.')),
      true,
    );
  });

  it('supports onEvent callbacks', async () => {
    const events = [];
    const unsubscribe = service.onEvent((event) => events.push(event));

    service.publish({ status: 'success', sessionId: 'session-1' });
    assert.equal(events.length, 1);
    assert.equal(events[0].type, 'success');

    unsubscribe();
    service.publish({ status: 'error', sessionId: 'session-1' });
    assert.equal(events.length, 1);
  });

  it('does not fallback to global SSE clients when unmatched global subscription exists', async () => {
    const globalReq = createMockReq();
    const globalRes = createMockRes();
    const globalSub = await service.subscribe({
      eventTypes: ['success'],
    });

    cleanups.push(
      service.handleSSEConnection(globalReq, globalRes, {
        subscriptionId: globalSub.subscription.id,
      }),
    );

    service.publish({
      status: 'error',
      sessionId: 'session-1',
    });

    // Connected event only; publish should not fan out to a mismatched global subscription.
    assert.equal(globalRes.write.mock.calls.length, 1);
    assert.equal(globalRes.write.mock.calls[0].arguments[0].includes('connected'), true);
  });
});
