/**
 * Unit tests for a2a/notifications.js — A2A Agent Notification Webhook Service
 *
 * Tests: sendNotification, retryPendingNotifications, configureWebhooks,
 *        getNotificationLog, formatNotificationLog
 */

import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createHmac } from 'node:crypto';
import { createNotificationService } from '../../src/a2a/notifications.js';

// ===========================================================================
// Helpers
// ===========================================================================

const originalFetch = globalThis.fetch;

/** Build a mock store with all required methods as mock.fn() stubs. */
function makeStore(overrides = {}) {
  return {
    getWebhookConfig: mock.fn(async () => null),
    createNotificationLog: mock.fn(async () => {}),
    getNotificationLog: mock.fn(async () => null),
    updateNotificationLog: mock.fn(async () => {}),
    listNotificationLog: mock.fn(async () => []),
    getPendingNotifications: mock.fn(async () => []),
    upsertWebhookConfig: mock.fn(async () => {}),
    listWebhookConfigs: mock.fn(async () => []),
    ...overrides,
  };
}

/** Standard webhook config for a recipient. */
function webhookConfig(overrides = {}) {
  return {
    agent_address: '0xRecipient',
    endpoint_url: 'https://hooks.example.com/webhook',
    secret: 'whsec_testsecret123',
    enabled_events: ['*'],
    active: true,
    ...overrides,
  };
}

/** Compute HMAC-SHA256 hex digest (mirrors the service's signing logic). */
function hmacHex(secret, body) {
  return createHmac('sha256', secret).update(body).digest('hex');
}

// ===========================================================================
// Lifecycle
// ===========================================================================

afterEach(() => {
  globalThis.fetch = originalFetch;
});

// ===========================================================================
// sendNotification
// ===========================================================================

describe('sendNotification', () => {
  it('successfully delivers a notification and logs it', async () => {
    const config = webhookConfig();
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => config),
      getNotificationLog: mock.fn(async (id) => ({
        id,
        recipient_address: '0xRecipient',
        endpoint_url: config.endpoint_url,
        event_type: 'payment.completed',
        payload: JSON.stringify({ event_type: 'payment.completed', payload: { amount: 50 }, timestamp: '2026-01-01T00:00:00.000Z' }),
        signature: 'abc',
        status: 'delivered',
        attempts: 1,
        last_attempt_at: '2026-01-01T00:00:00.000Z',
        last_error: null,
        delivered_at: '2026-01-01T00:00:00.000Z',
        created_at: '2026-01-01T00:00:00.000Z',
        updated_at: '2026-01-01T00:00:00.000Z',
      })),
    });

    globalThis.fetch = mock.fn(async () => ({ ok: true, status: 200, statusText: 'OK' }));

    const svc = createNotificationService(store);
    const result = await svc.sendNotification({
      recipientAddress: '0xRecipient',
      eventType: 'payment.completed',
      payload: { amount: 50 },
    });

    assert.equal(result.status, 'delivered');
    assert.equal(result.recipientAddress, '0xRecipient');
    assert.equal(result.eventType, 'payment.completed');
    assert.equal(store.createNotificationLog.mock.calls.length, 1);
    assert.equal(globalThis.fetch.mock.calls.length, 1);

    // Verify fetch was called with POST and correct headers
    const [url, opts] = globalThis.fetch.mock.calls[0].arguments;
    assert.equal(url, config.endpoint_url);
    assert.equal(opts.method, 'POST');
    assert.equal(opts.headers['Content-Type'], 'application/json');
    assert.ok(opts.headers['X-StateSet-Signature'].startsWith('sha256='));
    assert.ok(opts.headers['X-StateSet-Timestamp']);
    assert.equal(opts.headers['X-StateSet-Event'], 'payment.completed');
  });

  it('throws when recipientAddress is missing', async () => {
    const svc = createNotificationService(makeStore());
    await assert.rejects(
      () => svc.sendNotification({ eventType: 'test', payload: {} }),
      { message: 'recipientAddress is required' },
    );
  });

  it('throws when eventType is missing', async () => {
    const svc = createNotificationService(makeStore());
    await assert.rejects(
      () => svc.sendNotification({ recipientAddress: '0xA', payload: {} }),
      { message: 'eventType is required' },
    );
  });

  it('throws when no webhook config exists and no override URL', async () => {
    const store = makeStore({ getWebhookConfig: mock.fn(async () => null) });
    const svc = createNotificationService(store);
    await assert.rejects(
      () => svc.sendNotification({ recipientAddress: '0xA', eventType: 'test', payload: {} }),
      /No webhook endpoint configured for 0xA/,
    );
  });

  it('throws when webhook is not active', async () => {
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => webhookConfig({ active: false })),
    });
    const svc = createNotificationService(store);
    await assert.rejects(
      () => svc.sendNotification({ recipientAddress: '0xRecipient', eventType: 'test', payload: {} }),
      /not active/,
    );
  });

  it('throws when event type is not in enabled_events', async () => {
    const store = makeStore({
      getWebhookConfig: mock.fn(async () =>
        webhookConfig({ enabled_events: ['payment.completed', 'order.created'] }),
      ),
    });
    const svc = createNotificationService(store);
    await assert.rejects(
      () => svc.sendNotification({ recipientAddress: '0xRecipient', eventType: 'escrow.released', payload: {} }),
      /not enabled/,
    );
  });

  it('allows any event when enabled_events includes wildcard *', async () => {
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => webhookConfig({ enabled_events: ['*'] })),
      getNotificationLog: mock.fn(async (id) => ({
        id,
        recipient_address: '0xRecipient',
        endpoint_url: 'https://hooks.example.com/webhook',
        event_type: 'some.random.event',
        payload: '{}',
        signature: 'sig',
        status: 'delivered',
        attempts: 1,
        last_attempt_at: null,
        last_error: null,
        delivered_at: null,
        created_at: null,
        updated_at: null,
      })),
    });
    globalThis.fetch = mock.fn(async () => ({ ok: true, status: 200, statusText: 'OK' }));

    const svc = createNotificationService(store);
    const result = await svc.sendNotification({
      recipientAddress: '0xRecipient',
      eventType: 'some.random.event',
      payload: {},
    });
    assert.equal(result.eventType, 'some.random.event');
  });

  it('validates the endpoint URL (rejects non-http protocols)', async () => {
    const store = makeStore({
      getWebhookConfig: mock.fn(async () =>
        webhookConfig({ endpoint_url: 'ftp://evil.example.com/hook' }),
      ),
    });
    const svc = createNotificationService(store);
    await assert.rejects(
      () => svc.sendNotification({ recipientAddress: '0xRecipient', eventType: 'test', payload: {} }),
      /protocol|Invalid/i,
    );
  });

  it('rejects SSRF attempts to localhost', async () => {
    const store = makeStore({
      getWebhookConfig: mock.fn(async () =>
        webhookConfig({ endpoint_url: 'http://localhost:8080/admin' }),
      ),
    });
    const svc = createNotificationService(store);
    await assert.rejects(
      () => svc.sendNotification({ recipientAddress: '0xRecipient', eventType: 'test', payload: {} }),
      /SSRF|blocked|internal/i,
    );
  });

  it('rejects SSRF attempts to private IP', async () => {
    const store = makeStore({
      getWebhookConfig: mock.fn(async () =>
        webhookConfig({ endpoint_url: 'http://192.168.1.1/hook' }),
      ),
    });
    const svc = createNotificationService(store);
    await assert.rejects(
      () => svc.sendNotification({ recipientAddress: '0xRecipient', eventType: 'test', payload: {} }),
      /SSRF|blocked|internal/i,
    );
  });

  it('does not follow webhook redirects to internal addresses', async () => {
    const config = webhookConfig({ endpoint_url: 'https://hooks.example.com/webhook' });
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => config),
      getNotificationLog: mock.fn(async () => null),
    });
    globalThis.fetch = mock.fn(async () =>
      new Response('', {
        status: 302,
        headers: { location: 'http://169.254.169.254/latest/meta-data' },
      }),
    );

    const svc = createNotificationService(store);
    const result = await svc.sendNotification({
      recipientAddress: '0xRecipient',
      eventType: 'test',
      payload: {},
    });

    assert.equal(result.status, 'pending');
    assert.match(result.lastError, /SSRF|blocked|internal/i);
    assert.equal(globalThis.fetch.mock.calls.length, 1);
    assert.equal(globalThis.fetch.mock.calls[0].arguments[1].redirect, 'manual');
  });

  it('computes correct HMAC-SHA256 signature', async () => {
    const secret = 'whsec_mysecret';
    const config = webhookConfig({ secret });
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => config),
      getNotificationLog: mock.fn(async () => null),
    });
    globalThis.fetch = mock.fn(async () => ({ ok: true, status: 200, statusText: 'OK' }));

    const svc = createNotificationService(store);
    await svc.sendNotification({
      recipientAddress: '0xRecipient',
      eventType: 'payment.completed',
      payload: { id: 'pay-1' },
    });

    // Verify the log record created has a valid HMAC
    const logRecord = store.createNotificationLog.mock.calls[0].arguments[0];
    const bodyStr = JSON.stringify(logRecord.payload);
    const expected = hmacHex(secret, bodyStr);
    assert.equal(logRecord.signature, expected);

    // Verify the fetch header matches
    const [, opts] = globalThis.fetch.mock.calls[0].arguments;
    assert.equal(opts.headers['X-StateSet-Signature'], `sha256=${expected}`);
  });

  it('uses empty string as HMAC secret when config has no secret', async () => {
    const config = webhookConfig({ secret: null });
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => config),
      getNotificationLog: mock.fn(async () => null),
    });
    globalThis.fetch = mock.fn(async () => ({ ok: true, status: 200, statusText: 'OK' }));

    const svc = createNotificationService(store);
    await svc.sendNotification({
      recipientAddress: '0xRecipient',
      eventType: 'test',
      payload: { x: 1 },
    });

    const logRecord = store.createNotificationLog.mock.calls[0].arguments[0];
    const bodyStr = JSON.stringify(logRecord.payload);
    const expected = hmacHex('', bodyStr);
    assert.equal(logRecord.signature, expected);
  });

  it('sets status to pending on HTTP error response', async () => {
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => webhookConfig()),
      getNotificationLog: mock.fn(async () => null),
    });
    globalThis.fetch = mock.fn(async () => ({ ok: false, status: 503, statusText: 'Service Unavailable' }));

    const svc = createNotificationService(store);
    const result = await svc.sendNotification({
      recipientAddress: '0xRecipient',
      eventType: 'test',
      payload: {},
    });

    const logRecord = store.createNotificationLog.mock.calls[0].arguments[0];
    assert.equal(logRecord.status, 'pending');
    assert.equal(logRecord.last_error, 'HTTP 503: Service Unavailable');
    assert.equal(logRecord.delivered_at, null);
  });

  it('sets status to pending on network error and logs warning', async () => {
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => webhookConfig()),
      getNotificationLog: mock.fn(async () => null),
    });
    globalThis.fetch = mock.fn(async () => { throw new Error('ECONNREFUSED'); });

    const svc = createNotificationService(store);
    const result = await svc.sendNotification({
      recipientAddress: '0xRecipient',
      eventType: 'test',
      payload: {},
    });

    const logRecord = store.createNotificationLog.mock.calls[0].arguments[0];
    assert.equal(logRecord.status, 'pending');
    assert.equal(logRecord.last_error, 'ECONNREFUSED');
  });

  it('uses override endpointUrl when provided, bypassing config lookup', async () => {
    // No config for this address — but override URL is provided
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => null),
      getNotificationLog: mock.fn(async () => null),
    });
    globalThis.fetch = mock.fn(async () => ({ ok: true, status: 200, statusText: 'OK' }));

    const svc = createNotificationService(store);
    await svc.sendNotification({
      recipientAddress: '0xRecipient',
      eventType: 'test',
      payload: { a: 1 },
      endpointUrl: 'https://override.example.com/hook',
    });

    const [url] = globalThis.fetch.mock.calls[0].arguments;
    assert.equal(url, 'https://override.example.com/hook');
  });

  it('prefers override endpointUrl over config endpoint_url', async () => {
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => webhookConfig()),
      getNotificationLog: mock.fn(async () => null),
    });
    globalThis.fetch = mock.fn(async () => ({ ok: true, status: 200, statusText: 'OK' }));

    const svc = createNotificationService(store);
    await svc.sendNotification({
      recipientAddress: '0xRecipient',
      eventType: 'test',
      payload: {},
      endpointUrl: 'https://override.example.com/v2',
    });

    const [url] = globalThis.fetch.mock.calls[0].arguments;
    assert.equal(url, 'https://override.example.com/v2');
  });

  it('returns formatted log from store when available', async () => {
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => webhookConfig()),
      getNotificationLog: mock.fn(async (id) => ({
        id,
        recipient_address: '0xRecipient',
        endpoint_url: 'https://hooks.example.com/webhook',
        event_type: 'test',
        payload: '{"event_type":"test","payload":{},"timestamp":"2026-01-01"}',
        signature: 'deadbeef',
        status: 'delivered',
        attempts: 1,
        last_attempt_at: '2026-01-01',
        last_error: null,
        delivered_at: '2026-01-01',
        created_at: '2026-01-01',
        updated_at: '2026-01-01',
      })),
    });
    globalThis.fetch = mock.fn(async () => ({ ok: true, status: 200, statusText: 'OK' }));

    const svc = createNotificationService(store);
    const result = await svc.sendNotification({
      recipientAddress: '0xRecipient',
      eventType: 'test',
      payload: {},
    });

    // formatNotificationLog converts snake_case -> camelCase
    assert.ok(result.id);
    assert.equal(result.recipientAddress, '0xRecipient');
    assert.equal(result.endpointUrl, 'https://hooks.example.com/webhook');
    assert.equal(result.deliveredAt, '2026-01-01');
    assert.equal(result.lastError, null);
  });

  it('falls back to local logRecord when store.getNotificationLog returns null', async () => {
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => webhookConfig()),
      getNotificationLog: mock.fn(async () => null),
    });
    globalThis.fetch = mock.fn(async () => ({ ok: true, status: 200, statusText: 'OK' }));

    const svc = createNotificationService(store);
    const result = await svc.sendNotification({
      recipientAddress: '0xRecipient',
      eventType: 'test',
      payload: { v: 1 },
    });

    // Should still return formatted output even without stored version
    assert.ok(result.id);
    assert.equal(result.recipientAddress, '0xRecipient');
    assert.equal(result.status, 'delivered');
  });

  it('includes event_type, payload, and timestamp in signed body', async () => {
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => webhookConfig()),
      getNotificationLog: mock.fn(async () => null),
    });
    globalThis.fetch = mock.fn(async () => ({ ok: true, status: 200, statusText: 'OK' }));

    const svc = createNotificationService(store);
    await svc.sendNotification({
      recipientAddress: '0xRecipient',
      eventType: 'order.shipped',
      payload: { orderId: 'ORD-1' },
    });

    const [, opts] = globalThis.fetch.mock.calls[0].arguments;
    const body = JSON.parse(opts.body);
    assert.equal(body.event_type, 'order.shipped');
    assert.deepEqual(body.payload, { orderId: 'ORD-1' });
    assert.ok(typeof body.timestamp === 'string');
  });
});

// ===========================================================================
// retryPendingNotifications
// ===========================================================================

describe('retryPendingNotifications', () => {
  it('returns zero counts when there are no pending notifications', async () => {
    const store = makeStore({ getPendingNotifications: mock.fn(async () => []) });
    globalThis.fetch = mock.fn(async () => ({ ok: true, status: 200, statusText: 'OK' }));

    const svc = createNotificationService(store);
    const result = await svc.retryPendingNotifications();

    assert.deepEqual(result, { retried: 0, succeeded: 0, failed: 0 });
    assert.equal(globalThis.fetch.mock.calls.length, 0);
  });

  it('successfully retries a pending notification', async () => {
    const pending = [{
      id: 'notif-1',
      endpoint_url: 'https://hooks.example.com/webhook',
      event_type: 'payment.completed',
      payload: JSON.stringify({ event_type: 'payment.completed', payload: {}, timestamp: '2026-01-01' }),
      signature: 'sig1',
      attempts: 1,
      last_attempt_at: '2020-01-01T00:00:00.000Z', // far in the past — backoff elapsed
    }];
    const store = makeStore({ getPendingNotifications: mock.fn(async () => pending) });
    globalThis.fetch = mock.fn(async () => ({ ok: true, status: 200, statusText: 'OK' }));

    const svc = createNotificationService(store);
    const result = await svc.retryPendingNotifications();

    assert.equal(result.retried, 1);
    assert.equal(result.succeeded, 1);
    assert.equal(result.failed, 0);

    // Verify store was updated with delivered status
    const updateCall = store.updateNotificationLog.mock.calls[0].arguments;
    assert.equal(updateCall[0], 'notif-1');
    assert.equal(updateCall[1].status, 'delivered');
    assert.ok(updateCall[1].delivered_at);
    assert.equal(updateCall[1].last_error, null);
    assert.equal(updateCall[1].attempts, 2);
  });

  it('skips notifications still within backoff window', async () => {
    // last_attempt_at is right now, attempts=1 => backoff = 2s
    const pending = [{
      id: 'notif-2',
      endpoint_url: 'https://hooks.example.com/webhook',
      event_type: 'test',
      payload: '{}',
      signature: 'sig',
      attempts: 1,
      last_attempt_at: new Date().toISOString(), // just now — within backoff
    }];
    const store = makeStore({ getPendingNotifications: mock.fn(async () => pending) });
    globalThis.fetch = mock.fn(async () => ({ ok: true, status: 200, statusText: 'OK' }));

    const svc = createNotificationService(store);
    const result = await svc.retryPendingNotifications();

    assert.equal(result.retried, 0);
    assert.equal(result.succeeded, 0);
    assert.equal(result.failed, 0);
    assert.equal(globalThis.fetch.mock.calls.length, 0);
  });

  it('marks notification as failed when max attempts reached on HTTP error', async () => {
    const pending = [{
      id: 'notif-3',
      endpoint_url: 'https://hooks.example.com/webhook',
      event_type: 'test',
      payload: '{"event_type":"test"}',
      signature: 'sig',
      attempts: 2, // next will be 3 (= maxAttempts), so status becomes 'failed'
      last_attempt_at: '2020-01-01T00:00:00.000Z',
    }];
    const store = makeStore({ getPendingNotifications: mock.fn(async () => pending) });
    globalThis.fetch = mock.fn(async () => ({ ok: false, status: 500, statusText: 'Internal Server Error' }));

    const svc = createNotificationService(store);
    const result = await svc.retryPendingNotifications();

    assert.equal(result.retried, 1);
    assert.equal(result.succeeded, 0);
    assert.equal(result.failed, 1);

    const updateCall = store.updateNotificationLog.mock.calls[0].arguments;
    assert.equal(updateCall[1].status, 'failed');
    assert.equal(updateCall[1].attempts, 3);
    assert.equal(updateCall[1].last_error, 'HTTP 500: Internal Server Error');
  });

  it('keeps notification pending when attempts < maxAttempts on HTTP error', async () => {
    const pending = [{
      id: 'notif-4',
      endpoint_url: 'https://hooks.example.com/webhook',
      event_type: 'test',
      payload: '{}',
      signature: 'sig',
      attempts: 1, // next will be 2, still < 3
      last_attempt_at: '2020-01-01T00:00:00.000Z',
    }];
    const store = makeStore({ getPendingNotifications: mock.fn(async () => pending) });
    globalThis.fetch = mock.fn(async () => ({ ok: false, status: 502, statusText: 'Bad Gateway' }));

    const svc = createNotificationService(store);
    const result = await svc.retryPendingNotifications();

    assert.equal(result.retried, 1);
    assert.equal(result.succeeded, 0);
    assert.equal(result.failed, 0); // not failed yet — still pending

    const updateCall = store.updateNotificationLog.mock.calls[0].arguments;
    assert.equal(updateCall[1].status, 'pending');
    assert.equal(updateCall[1].attempts, 2);
  });

  it('marks notification as failed when max attempts reached on network error', async () => {
    const pending = [{
      id: 'notif-5',
      endpoint_url: 'https://hooks.example.com/webhook',
      event_type: 'test',
      payload: '{}',
      signature: 'sig',
      attempts: 2,
      last_attempt_at: '2020-01-01T00:00:00.000Z',
    }];
    const store = makeStore({ getPendingNotifications: mock.fn(async () => pending) });
    globalThis.fetch = mock.fn(async () => { throw new Error('ETIMEDOUT'); });

    const svc = createNotificationService(store);
    const result = await svc.retryPendingNotifications();

    assert.equal(result.retried, 1);
    assert.equal(result.succeeded, 0);
    assert.equal(result.failed, 1);

    const updateCall = store.updateNotificationLog.mock.calls[0].arguments;
    assert.equal(updateCall[1].status, 'failed');
    assert.equal(updateCall[1].last_error, 'ETIMEDOUT');
  });

  it('handles multiple pending notifications independently', async () => {
    const pending = [
      {
        id: 'notif-a',
        endpoint_url: 'https://a.example.com/hook',
        event_type: 'ev1',
        payload: '{"event_type":"ev1"}',
        signature: 'sig-a',
        attempts: 1,
        last_attempt_at: '2020-01-01T00:00:00.000Z',
      },
      {
        id: 'notif-b',
        endpoint_url: 'https://b.example.com/hook',
        event_type: 'ev2',
        payload: '{"event_type":"ev2"}',
        signature: 'sig-b',
        attempts: 1,
        last_attempt_at: '2020-01-01T00:00:00.000Z',
      },
    ];
    const store = makeStore({ getPendingNotifications: mock.fn(async () => pending) });

    let callCount = 0;
    globalThis.fetch = mock.fn(async () => {
      callCount++;
      if (callCount === 1) return { ok: true, status: 200, statusText: 'OK' };
      return { ok: false, status: 503, statusText: 'Service Unavailable' };
    });

    const svc = createNotificationService(store);
    const result = await svc.retryPendingNotifications();

    assert.equal(result.retried, 2);
    assert.equal(result.succeeded, 1);
    assert.equal(result.failed, 0); // second one is still pending (attempts 2 < 3)
  });

  it('calls getPendingNotifications with maxAttempts=3 and limit=50', async () => {
    const store = makeStore();
    const svc = createNotificationService(store);
    await svc.retryPendingNotifications();

    const call = store.getPendingNotifications.mock.calls[0].arguments;
    assert.equal(call[0], 3);
    assert.equal(call[1], 50);
  });

  it('handles payload as an object (not string)', async () => {
    const pending = [{
      id: 'notif-obj',
      endpoint_url: 'https://hooks.example.com/webhook',
      event_type: 'test',
      payload: { event_type: 'test', payload: {}, timestamp: '2026-01-01' }, // object, not string
      signature: 'sig',
      attempts: 1,
      last_attempt_at: '2020-01-01T00:00:00.000Z',
    }];
    const store = makeStore({ getPendingNotifications: mock.fn(async () => pending) });
    globalThis.fetch = mock.fn(async () => ({ ok: true, status: 200, statusText: 'OK' }));

    const svc = createNotificationService(store);
    const result = await svc.retryPendingNotifications();

    assert.equal(result.succeeded, 1);
    // Body should be JSON stringified object
    const [, opts] = globalThis.fetch.mock.calls[0].arguments;
    assert.equal(typeof opts.body, 'string');
    const parsed = JSON.parse(opts.body);
    assert.equal(parsed.event_type, 'test');
  });

  it('skips notifications with no last_attempt_at (no backoff delay)', async () => {
    const pending = [{
      id: 'notif-no-ts',
      endpoint_url: 'https://hooks.example.com/webhook',
      event_type: 'test',
      payload: '{}',
      signature: 'sig',
      attempts: 0,
      last_attempt_at: null, // no previous attempt
    }];
    const store = makeStore({ getPendingNotifications: mock.fn(async () => pending) });
    globalThis.fetch = mock.fn(async () => ({ ok: true, status: 200, statusText: 'OK' }));

    const svc = createNotificationService(store);
    const result = await svc.retryPendingNotifications();

    // Should proceed since there's no last_attempt_at to cause backoff
    assert.equal(result.retried, 1);
    assert.equal(result.succeeded, 1);
  });

  it('sends correct headers during retry delivery', async () => {
    const pending = [{
      id: 'notif-hdr',
      endpoint_url: 'https://hooks.example.com/webhook',
      event_type: 'order.shipped',
      payload: '{"event_type":"order.shipped"}',
      signature: 'abc123',
      attempts: 1,
      last_attempt_at: '2020-01-01T00:00:00.000Z',
    }];
    const store = makeStore({ getPendingNotifications: mock.fn(async () => pending) });
    globalThis.fetch = mock.fn(async () => ({ ok: true, status: 200, statusText: 'OK' }));

    const svc = createNotificationService(store);
    await svc.retryPendingNotifications();

    const [url, opts] = globalThis.fetch.mock.calls[0].arguments;
    assert.equal(url, 'https://hooks.example.com/webhook');
    assert.equal(opts.method, 'POST');
    assert.equal(opts.headers['Content-Type'], 'application/json');
    assert.equal(opts.headers['X-StateSet-Signature'], 'sha256=abc123');
    assert.equal(opts.headers['X-StateSet-Event'], 'order.shipped');
  });
});

// ===========================================================================
// configureWebhooks
// ===========================================================================

describe('configureWebhooks', () => {
  it('successfully creates a webhook configuration', async () => {
    const storedConfig = webhookConfig({ agent_address: '0xSeller' });
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => storedConfig),
    });

    const svc = createNotificationService(store);
    const result = await svc.configureWebhooks({
      agentAddress: '0xSeller',
      endpointUrl: 'https://seller.example.com/hooks',
      secret: 'whsec_secret',
      enabledEvents: ['payment.completed'],
    });

    assert.equal(store.upsertWebhookConfig.mock.calls.length, 1);
    const upserted = store.upsertWebhookConfig.mock.calls[0].arguments[0];
    assert.equal(upserted.agent_address, '0xSeller');
    assert.equal(upserted.endpoint_url, 'https://seller.example.com/hooks');
    assert.equal(upserted.secret, 'whsec_secret');
    assert.deepEqual(upserted.enabled_events, ['payment.completed']);
    assert.equal(upserted.active, true);
  });

  it('throws when agentAddress is missing', async () => {
    const svc = createNotificationService(makeStore());
    await assert.rejects(
      () => svc.configureWebhooks({ endpointUrl: 'https://x.com/h' }),
      { message: 'agentAddress is required' },
    );
  });

  it('throws when endpointUrl is missing', async () => {
    const svc = createNotificationService(makeStore());
    await assert.rejects(
      () => svc.configureWebhooks({ agentAddress: '0xA' }),
      { message: 'endpointUrl is required' },
    );
  });

  it('throws when endpointUrl uses non-HTTP protocol', async () => {
    const svc = createNotificationService(makeStore());
    await assert.rejects(
      () => svc.configureWebhooks({ agentAddress: '0xA', endpointUrl: 'ftp://example.com/hook' }),
      /Unsupported protocol|must start with http/,
    );
  });

  it('defaults enabledEvents to [*] when not provided', async () => {
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => webhookConfig()),
    });
    const svc = createNotificationService(store);
    await svc.configureWebhooks({
      agentAddress: '0xA',
      endpointUrl: 'https://example.com/hook',
    });

    const upserted = store.upsertWebhookConfig.mock.calls[0].arguments[0];
    assert.deepEqual(upserted.enabled_events, ['*']);
  });

  it('sets secret to null when not provided', async () => {
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => webhookConfig()),
    });
    const svc = createNotificationService(store);
    await svc.configureWebhooks({
      agentAddress: '0xA',
      endpointUrl: 'https://example.com/hook',
    });

    const upserted = store.upsertWebhookConfig.mock.calls[0].arguments[0];
    assert.equal(upserted.secret, null);
  });

  it('returns stored config from store after upsert', async () => {
    const stored = webhookConfig({ agent_address: '0xAgent' });
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => stored),
    });

    const svc = createNotificationService(store);
    const result = await svc.configureWebhooks({
      agentAddress: '0xAgent',
      endpointUrl: 'https://example.com/hook',
    });

    assert.equal(result.agent_address, '0xAgent');
    assert.equal(result.active, true);
  });

  it('returns local config when store.getWebhookConfig returns null', async () => {
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => null),
    });

    const svc = createNotificationService(store);
    const result = await svc.configureWebhooks({
      agentAddress: '0xFresh',
      endpointUrl: 'https://fresh.example.com/hook',
      secret: 'sec',
      enabledEvents: ['order.created'],
    });

    assert.equal(result.agent_address, '0xFresh');
    assert.equal(result.endpoint_url, 'https://fresh.example.com/hook');
    assert.equal(result.secret, 'sec');
    assert.deepEqual(result.enabled_events, ['order.created']);
    assert.equal(result.active, true);
  });

  it('accepts http:// endpoint URL', async () => {
    const store = makeStore({
      getWebhookConfig: mock.fn(async () => webhookConfig()),
    });
    const svc = createNotificationService(store);
    const result = await svc.configureWebhooks({
      agentAddress: '0xA',
      endpointUrl: 'http://example.com/hook',
    });
    assert.ok(result);
    assert.equal(store.upsertWebhookConfig.mock.calls.length, 1);
  });
});

// ===========================================================================
// getNotificationLog
// ===========================================================================

describe('getNotificationLog', () => {
  it('returns formatted logs from the store', async () => {
    const rawLogs = [
      {
        id: 'log-1',
        recipient_address: '0xA',
        endpoint_url: 'https://a.example.com',
        event_type: 'payment.completed',
        payload: '{"event_type":"payment.completed","payload":{}}',
        signature: 'sig1',
        status: 'delivered',
        attempts: 1,
        last_attempt_at: '2026-01-01',
        last_error: null,
        delivered_at: '2026-01-01',
        created_at: '2026-01-01',
        updated_at: '2026-01-01',
      },
      {
        id: 'log-2',
        recipient_address: '0xB',
        endpoint_url: 'https://b.example.com',
        event_type: 'order.shipped',
        payload: '{"event_type":"order.shipped","payload":{}}',
        signature: 'sig2',
        status: 'pending',
        attempts: 2,
        last_attempt_at: '2026-01-02',
        last_error: 'HTTP 500: Internal Server Error',
        delivered_at: null,
        created_at: '2026-01-01',
        updated_at: '2026-01-02',
      },
    ];
    const store = makeStore({ listNotificationLog: mock.fn(async () => rawLogs) });

    const svc = createNotificationService(store);
    const result = await svc.getNotificationLog({ status: 'delivered' });

    assert.equal(result.length, 2);
    assert.equal(result[0].recipientAddress, '0xA');
    assert.equal(result[0].status, 'delivered');
    assert.equal(result[1].recipientAddress, '0xB');
    assert.equal(result[1].lastError, 'HTTP 500: Internal Server Error');
  });

  it('returns empty array when no logs exist', async () => {
    const store = makeStore({ listNotificationLog: mock.fn(async () => []) });
    const svc = createNotificationService(store);
    const result = await svc.getNotificationLog();

    assert.deepEqual(result, []);
  });

  it('passes filter through to store.listNotificationLog', async () => {
    const store = makeStore({ listNotificationLog: mock.fn(async () => []) });
    const svc = createNotificationService(store);
    const filter = { recipient_address: '0xA', event_type: 'payment.completed', limit: 10, offset: 0 };
    await svc.getNotificationLog(filter);

    const passedFilter = store.listNotificationLog.mock.calls[0].arguments[0];
    assert.deepEqual(passedFilter, filter);
  });

  it('defaults filter to empty object when not provided', async () => {
    const store = makeStore({ listNotificationLog: mock.fn(async () => []) });
    const svc = createNotificationService(store);
    await svc.getNotificationLog();

    const passedFilter = store.listNotificationLog.mock.calls[0].arguments[0];
    assert.deepEqual(passedFilter, {});
  });
});

// ===========================================================================
// formatNotificationLog
// ===========================================================================

describe('formatNotificationLog', () => {
  it('converts snake_case fields to camelCase', () => {
    const svc = createNotificationService(makeStore());
    const row = {
      id: 'log-x',
      recipient_address: '0xAddr',
      endpoint_url: 'https://example.com/hook',
      event_type: 'payment.completed',
      payload: { event_type: 'payment.completed', payload: { amount: 100 } },
      signature: 'sig',
      status: 'delivered',
      attempts: 1,
      last_attempt_at: '2026-01-01',
      last_error: null,
      delivered_at: '2026-01-01',
      created_at: '2026-01-01',
      updated_at: '2026-01-01',
    };

    const result = svc.formatNotificationLog(row);

    assert.equal(result.id, 'log-x');
    assert.equal(result.recipientAddress, '0xAddr');
    assert.equal(result.endpointUrl, 'https://example.com/hook');
    assert.equal(result.eventType, 'payment.completed');
    assert.equal(result.signature, 'sig');
    assert.equal(result.status, 'delivered');
    assert.equal(result.attempts, 1);
    assert.equal(result.lastAttemptAt, '2026-01-01');
    assert.equal(result.lastError, null);
    assert.equal(result.deliveredAt, '2026-01-01');
    assert.equal(result.createdAt, '2026-01-01');
    assert.equal(result.updatedAt, '2026-01-01');
  });

  it('parses JSON string payload', () => {
    const svc = createNotificationService(makeStore());
    const row = {
      id: 'log-json',
      recipient_address: '0xA',
      endpoint_url: 'https://example.com',
      event_type: 'test',
      payload: '{"event_type":"test","payload":{"key":"value"},"timestamp":"2026-01-01"}',
      signature: 'sig',
      status: 'delivered',
      attempts: 1,
      last_attempt_at: null,
      last_error: null,
      delivered_at: null,
      created_at: null,
      updated_at: null,
    };

    const result = svc.formatNotificationLog(row);
    assert.equal(typeof result.payload, 'object');
    assert.equal(result.payload.event_type, 'test');
    assert.deepEqual(result.payload.payload, { key: 'value' });
  });

  it('keeps payload as-is when it is already an object', () => {
    const svc = createNotificationService(makeStore());
    const payloadObj = { event_type: 'test', payload: { x: 1 } };
    const row = {
      id: 'log-obj',
      recipient_address: '0xA',
      endpoint_url: 'https://example.com',
      event_type: 'test',
      payload: payloadObj,
      signature: 'sig',
      status: 'pending',
      attempts: 1,
      last_attempt_at: null,
      last_error: null,
      delivered_at: null,
      created_at: null,
      updated_at: null,
    };

    const result = svc.formatNotificationLog(row);
    assert.deepEqual(result.payload, payloadObj);
  });

  it('keeps invalid JSON payload as the raw string', () => {
    const svc = createNotificationService(makeStore());
    const row = {
      id: 'log-bad',
      recipient_address: '0xA',
      endpoint_url: 'https://example.com',
      event_type: 'test',
      payload: 'this is not valid JSON {{{',
      signature: 'sig',
      status: 'pending',
      attempts: 1,
      last_attempt_at: null,
      last_error: null,
      delivered_at: null,
      created_at: null,
      updated_at: null,
    };

    const result = svc.formatNotificationLog(row);
    assert.equal(result.payload, 'this is not valid JSON {{{');
  });

  it('returns null when row is null', () => {
    const svc = createNotificationService(makeStore());
    assert.equal(svc.formatNotificationLog(null), null);
  });

  it('returns null when row is undefined', () => {
    const svc = createNotificationService(makeStore());
    assert.equal(svc.formatNotificationLog(undefined), null);
  });
});
