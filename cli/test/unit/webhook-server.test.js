/**
 * Tests for cli/src/webhooks/server.js — WebhookSource, WebhookHandler, WebhookEvent, WebhookServer
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { createHmac } from 'crypto';

import {
  WebhookSource,
  WebhookHandler,
  WebhookEvent,
  WebhookServer,
  WebhookSourceTemplates,
  WebhookHandlerTemplates,
} from '../../src/webhooks/server.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Build a valid HMAC signature for the given body + secret */
function signBody(body, secret, algorithm = 'sha256', prefix = '') {
  return prefix + createHmac(algorithm, secret).update(body).digest('hex');
}

function createMockResponse() {
  return {
    statusCode: null,
    headers: null,
    body: '',
    ended: false,
    writeHead(statusCode, headers) {
      this.statusCode = statusCode;
      this.headers = headers;
    },
    end(payload = '') {
      this.ended = true;
      this.body = typeof payload === 'string' ? payload : String(payload || '');
    },
  };
}

function createMockRequest({
  method = 'POST',
  path = '/',
  headers = {},
  chunks = [],
  onDestroy,
} = {}) {
  return {
    method,
    url: path,
    headers: { host: 'localhost:3000', ...headers },
    destroy: onDestroy || (() => {}),
    async *[Symbol.asyncIterator]() {
      for (const chunk of chunks) {
        yield chunk;
      }
    },
  };
}

// ---------------------------------------------------------------------------
// WebhookSource
// ---------------------------------------------------------------------------

describe('WebhookSource', () => {
  describe('constructor defaults', () => {
    it('generates an id when none is provided', () => {
      const src = new WebhookSource({ name: 'test', path: '/wh' });
      assert.ok(typeof src.id === 'string' && src.id.length > 0);
    });

    it('uses the id when explicitly provided', () => {
      const src = new WebhookSource({ id: 'custom-id', name: 'test', path: '/wh' });
      assert.equal(src.id, 'custom-id');
    });

    it('defaults enabled to true', () => {
      const src = new WebhookSource({ name: 'test', path: '/wh' });
      assert.equal(src.enabled, true);
    });

    it('defaults secret to null', () => {
      const src = new WebhookSource({ name: 'test', path: '/wh' });
      assert.equal(src.secret, null);
    });

    it('defaults signatureHeader to x-signature', () => {
      const src = new WebhookSource({ name: 'test', path: '/wh' });
      assert.equal(src.signatureHeader, 'x-signature');
    });

    it('defaults signatureAlgorithm to sha256', () => {
      const src = new WebhookSource({ name: 'test', path: '/wh' });
      assert.equal(src.signatureAlgorithm, 'sha256');
    });

    it('defaults signaturePrefix to empty string', () => {
      const src = new WebhookSource({ name: 'test', path: '/wh' });
      assert.equal(src.signaturePrefix, '');
    });

    it('defaults eventTypeField to type', () => {
      const src = new WebhookSource({ name: 'test', path: '/wh' });
      assert.equal(src.eventTypeField, 'type');
    });

    it('defaults payloadField to null', () => {
      const src = new WebhookSource({ name: 'test', path: '/wh' });
      assert.equal(src.payloadField, null);
    });

    it('defaults retryOnFailure to true', () => {
      const src = new WebhookSource({ name: 'test', path: '/wh' });
      assert.equal(src.retryOnFailure, true);
    });

    it('defaults maxRetries to 3', () => {
      const src = new WebhookSource({ name: 'test', path: '/wh' });
      assert.equal(src.maxRetries, 3);
    });

    it('defaults metadata to empty object', () => {
      const src = new WebhookSource({ name: 'test', path: '/wh' });
      assert.deepEqual(src.metadata, {});
    });

    it('defaults description to empty string', () => {
      const src = new WebhookSource({ name: 'test', path: '/wh' });
      assert.equal(src.description, '');
    });
  });

  // ---- Signature verification ----

  describe('verifySignature', () => {
    it('returns true when secret is null (no verification)', () => {
      const src = new WebhookSource({ name: 'test', path: '/wh', secret: null });
      assert.equal(src.verifySignature('anything', 'anything'), true);
    });

    it('verifies a correct sha256 HMAC signature', () => {
      const secret = 'my-secret';
      const body = '{"type":"order.created"}';
      const src = new WebhookSource({ name: 'test', path: '/wh', secret });
      const sig = signBody(body, secret);
      assert.equal(src.verifySignature(body, sig), true);
    });

    it('rejects an incorrect signature', () => {
      const secret = 'my-secret';
      const body = '{"type":"order.created"}';
      const src = new WebhookSource({ name: 'test', path: '/wh', secret });
      assert.equal(src.verifySignature(body, 'bad-signature-value'), false);
    });

    it('rejects a signature with tampered body', () => {
      const secret = 'my-secret';
      const original = '{"type":"order.created"}';
      const tampered = '{"type":"order.deleted"}';
      const src = new WebhookSource({ name: 'test', path: '/wh', secret });
      const sig = signBody(original, secret);
      assert.equal(src.verifySignature(tampered, sig), false);
    });

    it('handles signaturePrefix correctly', () => {
      const secret = 'my-secret';
      const body = '{"type":"order.created"}';
      const prefix = 'sha256=';
      const src = new WebhookSource({
        name: 'test',
        path: '/wh',
        secret,
        signaturePrefix: prefix,
      });
      const sig = signBody(body, secret, 'sha256', prefix);
      assert.equal(src.verifySignature(body, sig), true);
    });

    it('rejects when prefix is expected but not present in supplied signature', () => {
      const secret = 'my-secret';
      const body = '{"type":"order.created"}';
      const src = new WebhookSource({
        name: 'test',
        path: '/wh',
        secret,
        signaturePrefix: 'sha256=',
      });
      // Provide the raw HMAC without the prefix
      const rawSig = signBody(body, secret, 'sha256', '');
      assert.equal(src.verifySignature(body, rawSig), false);
    });

    it('returns false on length mismatch (timingSafeEqual throws)', () => {
      const secret = 'my-secret';
      const body = '{"type":"order.created"}';
      const src = new WebhookSource({ name: 'test', path: '/wh', secret });
      // Short signature triggers buffer length mismatch
      assert.equal(src.verifySignature(body, 'short'), false);
    });

    it('returns false for empty signature when secret is set', () => {
      const secret = 'my-secret';
      const src = new WebhookSource({ name: 'test', path: '/wh', secret });
      assert.equal(src.verifySignature('body', ''), false);
    });

    it('verifies correctly with a different algorithm (sha512)', () => {
      const secret = 'my-secret';
      const body = '{"amount":100}';
      const src = new WebhookSource({
        name: 'test',
        path: '/wh',
        secret,
        signatureAlgorithm: 'sha512',
      });
      const sig = signBody(body, secret, 'sha512');
      assert.equal(src.verifySignature(body, sig), true);
    });
  });

  // ---- toJSON ----

  describe('toJSON', () => {
    it('does not expose the secret', () => {
      const src = new WebhookSource({ name: 'test', path: '/wh', secret: 'top-secret' });
      const json = src.toJSON();
      assert.equal(json.secret, undefined);
    });

    it('includes core fields', () => {
      const src = new WebhookSource({
        name: 'Stripe',
        path: '/webhooks/stripe',
        description: 'Stripe hooks',
      });
      const json = src.toJSON();
      assert.equal(json.name, 'Stripe');
      assert.equal(json.path, '/webhooks/stripe');
      assert.equal(json.description, 'Stripe hooks');
      assert.equal(json.enabled, true);
    });
  });
});

// ---------------------------------------------------------------------------
// WebhookHandler
// ---------------------------------------------------------------------------

describe('WebhookHandler', () => {
  describe('constructor defaults', () => {
    it('generates an id', () => {
      const h = new WebhookHandler({ name: 'h', sourceId: 's1', action: {} });
      assert.ok(typeof h.id === 'string');
    });

    it('defaults eventTypes to wildcard', () => {
      const h = new WebhookHandler({ name: 'h', sourceId: 's1', action: {} });
      assert.deepEqual(h.eventTypes, ['*']);
    });

    it('defaults enabled to true', () => {
      const h = new WebhookHandler({ name: 'h', sourceId: 's1', action: {} });
      assert.equal(h.enabled, true);
    });

    it('defaults priority to 0', () => {
      const h = new WebhookHandler({ name: 'h', sourceId: 's1', action: {} });
      assert.equal(h.priority, 0);
    });

    it('defaults conditions to null', () => {
      const h = new WebhookHandler({ name: 'h', sourceId: 's1', action: {} });
      assert.equal(h.conditions, null);
    });
  });

  describe('matches', () => {
    it('returns true for wildcard handler', () => {
      const h = new WebhookHandler({ name: 'h', sourceId: 's', action: {} });
      assert.equal(h.matches('order.created', {}), true);
    });

    it('returns true when eventType is in the list', () => {
      const h = new WebhookHandler({
        name: 'h',
        sourceId: 's',
        action: {},
        eventTypes: ['order.created', 'order.updated'],
      });
      assert.equal(h.matches('order.created', {}), true);
    });

    it('returns false when eventType is not in the list', () => {
      const h = new WebhookHandler({
        name: 'h',
        sourceId: 's',
        action: {},
        eventTypes: ['order.created'],
      });
      assert.equal(h.matches('order.deleted', {}), false);
    });

    it('returns false when handler is disabled', () => {
      const h = new WebhookHandler({
        name: 'h',
        sourceId: 's',
        action: {},
        enabled: false,
      });
      assert.equal(h.matches('anything', {}), false);
    });

    it('matches when conditions are met', () => {
      const h = new WebhookHandler({
        name: 'h',
        sourceId: 's',
        action: {},
        conditions: { status: 'active' },
      });
      assert.equal(h.matches('event', { status: 'active' }), true);
    });

    it('rejects when conditions are not met', () => {
      const h = new WebhookHandler({
        name: 'h',
        sourceId: 's',
        action: {},
        conditions: { status: 'active' },
      });
      assert.equal(h.matches('event', { status: 'inactive' }), false);
    });

    it('checks nested conditions via dot paths', () => {
      const h = new WebhookHandler({
        name: 'h',
        sourceId: 's',
        action: {},
        conditions: { 'data.object.status': 'paid' },
      });
      assert.equal(h.matches('event', { data: { object: { status: 'paid' } } }), true);
    });

    it('rejects when nested condition value is wrong', () => {
      const h = new WebhookHandler({
        name: 'h',
        sourceId: 's',
        action: {},
        conditions: { 'data.object.status': 'paid' },
      });
      assert.equal(h.matches('event', { data: { object: { status: 'unpaid' } } }), false);
    });

    it('rejects when nested condition path does not exist', () => {
      const h = new WebhookHandler({
        name: 'h',
        sourceId: 's',
        action: {},
        conditions: { 'data.missing.field': 'value' },
      });
      assert.equal(h.matches('event', { data: {} }), false);
    });

    it('handles multiple conditions (all must match)', () => {
      const h = new WebhookHandler({
        name: 'h',
        sourceId: 's',
        action: {},
        conditions: { status: 'active', region: 'us' },
      });
      assert.equal(h.matches('event', { status: 'active', region: 'us' }), true);
      assert.equal(h.matches('event', { status: 'active', region: 'eu' }), false);
    });
  });

  describe('toJSON', () => {
    it('returns all fields', () => {
      const h = new WebhookHandler({
        id: 'h1',
        name: 'handler',
        sourceId: 's1',
        action: { agent: 'orders' },
        priority: 5,
      });
      const json = h.toJSON();
      assert.equal(json.id, 'h1');
      assert.equal(json.name, 'handler');
      assert.equal(json.sourceId, 's1');
      assert.equal(json.priority, 5);
      assert.deepEqual(json.action, { agent: 'orders' });
    });
  });
});

// ---------------------------------------------------------------------------
// WebhookEvent
// ---------------------------------------------------------------------------

describe('WebhookEvent', () => {
  describe('constructor defaults', () => {
    it('generates an id', () => {
      const ev = new WebhookEvent({ sourceId: 's', eventType: 'e', payload: {} });
      assert.ok(typeof ev.id === 'string');
    });

    it('defaults status to pending', () => {
      const ev = new WebhookEvent({ sourceId: 's', eventType: 'e', payload: {} });
      assert.equal(ev.status, 'pending');
    });

    it('defaults retryCount to 0', () => {
      const ev = new WebhookEvent({ sourceId: 's', eventType: 'e', payload: {} });
      assert.equal(ev.retryCount, 0);
    });

    it('records receivedAt as ISO string', () => {
      const ev = new WebhookEvent({ sourceId: 's', eventType: 'e', payload: {} });
      assert.ok(typeof ev.receivedAt === 'string');
      assert.doesNotThrow(() => new Date(ev.receivedAt));
    });

    it('defaults processedAt to null', () => {
      const ev = new WebhookEvent({ sourceId: 's', eventType: 'e', payload: {} });
      assert.equal(ev.processedAt, null);
    });
  });

  describe('toJSON', () => {
    it('returns all public fields', () => {
      const ev = new WebhookEvent({
        id: 'ev1',
        sourceId: 's',
        sourceName: 'Stripe',
        eventType: 'payment_intent.succeeded',
        payload: { amount: 100 },
      });
      const json = ev.toJSON();
      assert.equal(json.id, 'ev1');
      assert.equal(json.sourceId, 's');
      assert.equal(json.sourceName, 'Stripe');
      assert.equal(json.eventType, 'payment_intent.succeeded');
      assert.deepEqual(json.payload, { amount: 100 });
    });
  });
});

// ---------------------------------------------------------------------------
// WebhookServer (without binding a port)
// ---------------------------------------------------------------------------

describe('WebhookServer', () => {
  /** @type {WebhookServer} */
  let server;

  beforeEach(() => {
    server = new WebhookServer({ port: 0, autoStart: false });
  });

  // ---- Constructor ----

  describe('constructor', () => {
    it('defaults port to 3000', () => {
      const s = new WebhookServer({});
      assert.equal(s.port, 3000);
    });

    it('defaults host to 0.0.0.0', () => {
      const s = new WebhookServer({});
      assert.equal(s.host, '0.0.0.0');
    });

    it('starts with empty sources map', () => {
      assert.equal(server.sources.size, 0);
    });

    it('starts with empty handlers map', () => {
      assert.equal(server.handlers.size, 0);
    });

    it('starts not running', () => {
      assert.equal(server.isRunning, false);
    });

    it('accepts custom port', () => {
      const s = new WebhookServer({ port: 9999 });
      assert.equal(s.port, 9999);
    });
  });

  describe('handleRequest', () => {
    it('rejects oversized body by content-length', async () => {
      const maxPayloadBytes = 8;
      const maxPayloadServer = new WebhookServer({
        port: 0,
        autoStart: false,
        maxPayloadBytes,
      });
      maxPayloadServer.registerSource({ id: 'src', name: 'source', path: '/webhooks/ship' });

      const req = createMockRequest({
        path: '/webhooks/ship',
        headers: { 'content-length': String(maxPayloadBytes + 1) },
        chunks: [Buffer.from('123456789')],
      });
      const res = createMockResponse();

      await maxPayloadServer.handleRequest(req, res);

      assert.equal(res.statusCode, 413);
      assert.ok(res.body.includes('Payload too large'));
    });

    it('rejects oversized body while streaming', async () => {
      const maxPayloadBytes = 8;
      let destroyed = false;
      const maxPayloadServer = new WebhookServer({
        port: 0,
        autoStart: false,
        maxPayloadBytes,
      });
      maxPayloadServer.registerSource({ id: 'src', name: 'source', path: '/webhooks/ship' });

      const req = createMockRequest({
        path: '/webhooks/ship',
        chunks: [Buffer.from('12345678'), Buffer.from('12')],
        onDestroy: () => {
          destroyed = true;
        },
      });
      const res = createMockResponse();

      await maxPayloadServer.handleRequest(req, res);

      assert.equal(res.statusCode, 413);
      assert.ok(res.body.includes('Payload too large'));
      assert.equal(destroyed, true);
    });
  });

  // ---- registerSource ----

  describe('registerSource', () => {
    it('registers a source from config object', () => {
      const src = server.registerSource({ name: 'Stripe', path: '/webhooks/stripe' });
      assert.ok(src instanceof WebhookSource);
      assert.equal(server.sources.size, 1);
    });

    it('registers a WebhookSource instance directly', () => {
      const ws = new WebhookSource({ name: 'Direct', path: '/wh/direct' });
      const result = server.registerSource(ws);
      assert.equal(result, ws);
      assert.equal(server.sources.get(ws.id), ws);
    });

    it('emits source:registered event', () => {
      let emitted = false;
      server.on('source:registered', () => {
        emitted = true;
      });
      server.registerSource({ name: 'x', path: '/x' });
      assert.equal(emitted, true);
    });
  });

  // ---- registerHandler ----

  describe('registerHandler', () => {
    it('registers a handler from config object', () => {
      const h = server.registerHandler({ name: 'h1', sourceId: 's1', action: {} });
      assert.ok(h instanceof WebhookHandler);
      assert.equal(server.handlers.size, 1);
    });

    it('registers a WebhookHandler instance directly', () => {
      const wh = new WebhookHandler({ name: 'h', sourceId: 's', action: {} });
      server.registerHandler(wh);
      assert.equal(server.handlers.get(wh.id), wh);
    });

    it('emits handler:registered event', () => {
      let emitted = false;
      server.on('handler:registered', () => {
        emitted = true;
      });
      server.registerHandler({ name: 'h', sourceId: 's', action: {} });
      assert.equal(emitted, true);
    });
  });

  // ---- findSourceByPath ----

  describe('findSourceByPath', () => {
    it('returns the source that matches the given path', () => {
      server.registerSource({ id: 's1', name: 'A', path: '/a' });
      server.registerSource({ id: 's2', name: 'B', path: '/b' });
      const found = server.findSourceByPath('/b');
      assert.equal(found.id, 's2');
    });

    it('returns null when no source matches', () => {
      server.registerSource({ id: 's1', name: 'A', path: '/a' });
      assert.equal(server.findSourceByPath('/unknown'), null);
    });

    it('ignores disabled sources', () => {
      server.registerSource({ id: 's1', name: 'A', path: '/a', enabled: false });
      assert.equal(server.findSourceByPath('/a'), null);
    });
  });

  // ---- findHandlers ----

  describe('findHandlers', () => {
    it('returns handlers matching sourceId', () => {
      server.registerHandler({ id: 'h1', name: 'h', sourceId: 's1', action: {} });
      server.registerHandler({ id: 'h2', name: 'h', sourceId: 's2', action: {} });
      const result = server.findHandlers('s1', 'evt');
      assert.equal(result.length, 1);
      assert.equal(result[0].id, 'h1');
    });

    it('returns wildcard handlers (sourceId = *)', () => {
      server.registerHandler({ id: 'h1', name: 'h', sourceId: '*', action: {} });
      const result = server.findHandlers('any-source', 'evt');
      assert.equal(result.length, 1);
    });

    it('sorts by priority descending', () => {
      server.registerHandler({ id: 'h-low', name: 'lo', sourceId: 's', action: {}, priority: 1 });
      server.registerHandler({ id: 'h-hi', name: 'hi', sourceId: 's', action: {}, priority: 10 });
      const result = server.findHandlers('s', 'evt');
      assert.equal(result[0].id, 'h-hi');
      assert.equal(result[1].id, 'h-low');
    });

    it('excludes disabled handlers', () => {
      server.registerHandler({ id: 'h1', name: 'h', sourceId: 's', action: {}, enabled: false });
      const result = server.findHandlers('s', 'evt');
      assert.equal(result.length, 0);
    });
  });

  // ---- processEvent ----

  describe('processEvent', () => {
    it('completes with "No matching handlers" when none match', async () => {
      const ev = new WebhookEvent({ sourceId: 's', eventType: 'order.created', payload: {} });
      const result = await server.processEvent(ev);
      assert.equal(result.status, 'completed');
      assert.equal(result.results[0].message, 'No matching handlers');
    });

    it('invokes executor for matching handlers', async () => {
      let executorCalled = false;
      const s = new WebhookServer({
        port: 0,
        autoStart: false,
        executor: async () => {
          executorCalled = true;
          return 'ok';
        },
      });
      s.registerHandler({
        id: 'h1',
        name: 'h',
        sourceId: 's1',
        action: { agent: 'orders', request: 'process' },
      });
      const ev = new WebhookEvent({ sourceId: 's1', eventType: 'order.created', payload: {} });
      await s.processEvent(ev);
      assert.equal(executorCalled, true);
    });

    it('records success result from executor', async () => {
      const s = new WebhookServer({
        port: 0,
        autoStart: false,
        executor: async () => 'done',
      });
      s.registerHandler({
        id: 'h1',
        name: 'myhandler',
        sourceId: 's1',
        action: { agent: 'orders', request: 'x' },
      });
      const ev = new WebhookEvent({ sourceId: 's1', eventType: 'e', payload: {} });
      const result = await s.processEvent(ev);
      assert.equal(result.results[0].success, true);
      assert.equal(result.results[0].result, 'done');
      assert.equal(result.results[0].handlerName, 'myhandler');
    });

    it('records failure when executor throws', async () => {
      const s = new WebhookServer({
        port: 0,
        autoStart: false,
        executor: async () => {
          throw new Error('boom');
        },
      });
      s.registerHandler({
        id: 'h1',
        name: 'h',
        sourceId: 's1',
        action: { agent: 'orders', request: 'x' },
      });
      const ev = new WebhookEvent({ sourceId: 's1', eventType: 'e', payload: {} });
      const result = await s.processEvent(ev);
      assert.equal(result.results[0].success, false);
      assert.equal(result.results[0].error, 'boom');
    });

    it('emits event:processing and event:completed', async () => {
      const events = [];
      server.on('event:processing', () => events.push('processing'));
      server.on('event:completed', () => events.push('completed'));
      const ev = new WebhookEvent({ sourceId: 's', eventType: 'e', payload: {} });
      await server.processEvent(ev);
      assert.deepEqual(events, ['processing', 'completed']);
    });

    it('emits handler:executed on success', async () => {
      let emitted = false;
      const s = new WebhookServer({
        port: 0,
        autoStart: false,
        executor: async () => 'ok',
      });
      s.on('handler:executed', () => {
        emitted = true;
      });
      s.registerHandler({ id: 'h1', name: 'h', sourceId: 's1', action: { request: 'x' } });
      const ev = new WebhookEvent({ sourceId: 's1', eventType: 'e', payload: {} });
      await s.processEvent(ev);
      assert.equal(emitted, true);
    });

    it('emits handler:failed on error', async () => {
      let emitted = false;
      const s = new WebhookServer({
        port: 0,
        autoStart: false,
        executor: async () => {
          throw new Error('fail');
        },
      });
      s.on('handler:failed', () => {
        emitted = true;
      });
      s.registerHandler({ id: 'h1', name: 'h', sourceId: 's1', action: { request: 'x' } });
      const ev = new WebhookEvent({ sourceId: 's1', eventType: 'e', payload: {} });
      await s.processEvent(ev);
      assert.equal(emitted, true);
    });

    it('adds completed events to eventHistory', async () => {
      server.registerHandler({ id: 'h1', name: 'h', sourceId: 's', action: {} });
      const ev = new WebhookEvent({ sourceId: 's', eventType: 'e', payload: {} });
      await server.processEvent(ev);
      assert.equal(server.eventHistory.length, 1);
    });

    it('trims eventHistory to 1000 entries', async () => {
      server.registerHandler({ id: 'h1', name: 'h', sourceId: 's', action: {} });
      // Pre-populate with 1001 entries
      for (let i = 0; i < 1001; i++) {
        server.eventHistory.push(
          new WebhookEvent({ sourceId: 's', eventType: 'e', payload: { i } }),
        );
      }
      const ev = new WebhookEvent({ sourceId: 's', eventType: 'e', payload: {} });
      await server.processEvent(ev);
      assert.ok(server.eventHistory.length <= 1001);
    });

    it('runs handlers in priority order', async () => {
      const order = [];
      const s = new WebhookServer({
        port: 0,
        autoStart: false,
        executor: async (action) => {
          order.push(action.request);
        },
      });
      s.registerHandler({
        id: 'low',
        name: 'low',
        sourceId: 's1',
        action: { request: 'low' },
        priority: 1,
      });
      s.registerHandler({
        id: 'high',
        name: 'high',
        sourceId: 's1',
        action: { request: 'high' },
        priority: 10,
      });
      const ev = new WebhookEvent({ sourceId: 's1', eventType: 'e', payload: {} });
      await s.processEvent(ev);
      assert.deepEqual(order, ['high', 'low']);
    });

    it('sets processedAt on completion', async () => {
      const ev = new WebhookEvent({ sourceId: 's', eventType: 'e', payload: {} });
      await server.processEvent(ev);
      assert.ok(ev.processedAt !== null);
    });

    it('records handler ids on the event', async () => {
      server.registerHandler({ id: 'h1', name: 'h', sourceId: 's', action: {} });
      server.registerHandler({ id: 'h2', name: 'h2', sourceId: 's', action: {} });
      const ev = new WebhookEvent({ sourceId: 's', eventType: 'e', payload: {} });
      await server.processEvent(ev);
      assert.ok(ev.handlers.includes('h1'));
      assert.ok(ev.handlers.includes('h2'));
    });

    it('skips executor when handler has no action', async () => {
      let called = false;
      const s = new WebhookServer({
        port: 0,
        autoStart: false,
        executor: async () => {
          called = true;
        },
      });
      s.registerHandler({ id: 'h1', name: 'h', sourceId: 's', action: null });
      const ev = new WebhookEvent({ sourceId: 's', eventType: 'e', payload: {} });
      await s.processEvent(ev);
      assert.equal(called, false);
    });
  });

  // ---- interpolateAction ----

  describe('interpolateAction', () => {
    it('interpolates payload fields into request string', () => {
      const action = { request: 'Process payment of {amount} {currency}' };
      const event = new WebhookEvent({
        sourceId: 's',
        eventType: 'e',
        payload: { amount: 100, currency: 'USD' },
      });
      const result = server.interpolateAction(action, event);
      assert.equal(result.request, 'Process payment of 100 USD');
    });

    it('interpolates event-level fields when payload field is missing', () => {
      const action = { request: 'Event {eventType} from {sourceId}' };
      const event = new WebhookEvent({
        sourceId: 'stripe-src',
        eventType: 'payment.success',
        payload: {},
      });
      const result = server.interpolateAction(action, event);
      assert.equal(result.request, 'Event payment.success from stripe-src');
    });

    it('leaves unresolved placeholders as-is', () => {
      const action = { request: 'Value is {nonexistent}' };
      const event = new WebhookEvent({ sourceId: 's', eventType: 'e', payload: {} });
      const result = server.interpolateAction(action, event);
      assert.equal(result.request, 'Value is {nonexistent}');
    });

    it('interpolates workflow field', () => {
      const action = { workflow: 'run-{status}' };
      const event = new WebhookEvent({
        sourceId: 's',
        eventType: 'e',
        payload: { status: 'active' },
      });
      const result = server.interpolateAction(action, event);
      assert.equal(result.workflow, 'run-active');
    });

    it('passes non-string values through unchanged', () => {
      const action = { request: 42, extra: true };
      const event = new WebhookEvent({ sourceId: 's', eventType: 'e', payload: {} });
      const result = server.interpolateAction(action, event);
      assert.equal(result.request, 42);
    });
  });

  // ---- getStatus ----

  describe('getStatus', () => {
    it('returns expected shape', () => {
      const status = server.getStatus();
      assert.equal(status.isRunning, false);
      assert.equal(status.sourceCount, 0);
      assert.equal(status.handlerCount, 0);
      assert.ok(Array.isArray(status.recentEvents));
    });

    it('reflects registered sources and handlers', () => {
      server.registerSource({ name: 'a', path: '/a' });
      server.registerHandler({ name: 'h', sourceId: 's', action: {} });
      const status = server.getStatus();
      assert.equal(status.sourceCount, 1);
      assert.equal(status.handlerCount, 1);
    });
  });

  // ---- getHistory ----

  describe('getHistory', () => {
    beforeEach(async () => {
      // Add some events to history
      server.registerHandler({ id: 'h1', name: 'h', sourceId: 'src-a', action: {} });
      server.registerHandler({ id: 'h2', name: 'h', sourceId: 'src-b', action: {} });
      await server.processEvent(
        new WebhookEvent({ sourceId: 'src-a', eventType: 'order.created', payload: {} }),
      );
      await server.processEvent(
        new WebhookEvent({ sourceId: 'src-b', eventType: 'payment.failed', payload: {} }),
      );
    });

    it('returns all history when no filters', () => {
      const h = server.getHistory();
      assert.equal(h.length, 2);
    });

    it('filters by sourceId', () => {
      const h = server.getHistory({ sourceId: 'src-a' });
      assert.equal(h.length, 1);
      assert.equal(h[0].sourceId, 'src-a');
    });

    it('filters by eventType', () => {
      const h = server.getHistory({ eventType: 'payment.failed' });
      assert.equal(h.length, 1);
      assert.equal(h[0].eventType, 'payment.failed');
    });

    it('filters by status', () => {
      const h = server.getHistory({ status: 'completed' });
      assert.equal(h.length, 2);
    });

    it('respects limit', () => {
      const h = server.getHistory({ limit: 1 });
      assert.equal(h.length, 1);
    });
  });

  // ---- listSources / listHandlers ----

  describe('listSources', () => {
    it('returns JSON array of registered sources', () => {
      server.registerSource({ name: 'Stripe', path: '/s' });
      server.registerSource({ name: 'Shopify', path: '/sh' });
      const list = server.listSources();
      assert.equal(list.length, 2);
      assert.ok(list.every((s) => typeof s.name === 'string'));
    });
  });

  describe('listHandlers', () => {
    it('returns JSON array of registered handlers', () => {
      server.registerHandler({ name: 'h1', sourceId: 's', action: {} });
      const list = server.listHandlers();
      assert.equal(list.length, 1);
      assert.equal(list[0].name, 'h1');
    });
  });
});

// ---------------------------------------------------------------------------
// WebhookSourceTemplates
// ---------------------------------------------------------------------------

describe('WebhookSourceTemplates', () => {
  it('includes stripe template', () => {
    assert.equal(WebhookSourceTemplates.stripe.name, 'Stripe');
    assert.equal(WebhookSourceTemplates.stripe.path, '/webhooks/stripe');
    assert.equal(WebhookSourceTemplates.stripe.signatureHeader, 'stripe-signature');
    assert.equal(WebhookSourceTemplates.stripe.payloadField, 'data.object');
  });

  it('includes shopify template', () => {
    assert.equal(WebhookSourceTemplates.shopify.name, 'Shopify');
    assert.equal(WebhookSourceTemplates.shopify.signatureHeader, 'x-shopify-hmac-sha256');
  });

  it('includes square template', () => {
    assert.equal(WebhookSourceTemplates.square.name, 'Square');
    assert.equal(WebhookSourceTemplates.square.payloadField, 'data');
  });

  it('includes shippo template', () => {
    assert.equal(WebhookSourceTemplates.shippo.name, 'Shippo');
    assert.equal(WebhookSourceTemplates.shippo.eventTypeField, 'event');
  });

  it('includes custom template', () => {
    assert.equal(WebhookSourceTemplates.custom.name, 'Custom');
    assert.equal(WebhookSourceTemplates.custom.eventTypeField, 'event_type');
  });

  it('can create WebhookSource from each template', () => {
    for (const [key, template] of Object.entries(WebhookSourceTemplates)) {
      const src = new WebhookSource(template);
      assert.ok(src instanceof WebhookSource, `Failed for template: ${key}`);
      assert.equal(src.name, template.name);
    }
  });
});

// ---------------------------------------------------------------------------
// WebhookHandlerTemplates
// ---------------------------------------------------------------------------

describe('WebhookHandlerTemplates', () => {
  it('includes stripePaymentSucceeded template', () => {
    const t = WebhookHandlerTemplates.stripePaymentSucceeded;
    assert.ok(t.eventTypes.includes('payment_intent.succeeded'));
    assert.equal(t.action.agent, 'payments');
  });

  it('includes stripePaymentFailed template', () => {
    const t = WebhookHandlerTemplates.stripePaymentFailed;
    assert.ok(t.eventTypes.includes('payment_intent.payment_failed'));
  });

  it('includes shopifyOrderCreated template', () => {
    const t = WebhookHandlerTemplates.shopifyOrderCreated;
    assert.ok(t.eventTypes.includes('orders/create'));
    assert.equal(t.action.agent, 'orders');
  });

  it('includes shippoTrackingUpdate template', () => {
    const t = WebhookHandlerTemplates.shippoTrackingUpdate;
    assert.ok(t.eventTypes.includes('track_updated'));
  });

  it('can create WebhookHandler from each template (with required sourceId)', () => {
    for (const [key, template] of Object.entries(WebhookHandlerTemplates)) {
      const h = new WebhookHandler({ ...template, sourceId: 'test-src' });
      assert.ok(h instanceof WebhookHandler, `Failed for template: ${key}`);
      assert.equal(h.name, template.name);
    }
  });
});

// ---------------------------------------------------------------------------
// Integration-style: processEvent with handler conditions
// ---------------------------------------------------------------------------

describe('WebhookServer integration: processEvent with conditions', () => {
  it('only invokes handlers whose conditions match the payload', async () => {
    const calls = [];
    const s = new WebhookServer({
      port: 0,
      autoStart: false,
      executor: async (action) => {
        calls.push(action.request);
      },
    });

    s.registerHandler({
      id: 'paid-only',
      name: 'paid',
      sourceId: 's1',
      action: { request: 'handle-paid' },
      conditions: { status: 'paid' },
    });
    s.registerHandler({
      id: 'any-status',
      name: 'any',
      sourceId: 's1',
      action: { request: 'handle-any' },
    });

    const ev = new WebhookEvent({
      sourceId: 's1',
      eventType: 'order.updated',
      payload: { status: 'pending' },
    });

    await s.processEvent(ev);

    // 'paid-only' should NOT fire (status != paid), 'any-status' should fire
    assert.deepEqual(calls, ['handle-any']);
  });

  it('continues to next handler after one handler throws', async () => {
    const calls = [];
    let callCount = 0;
    const s = new WebhookServer({
      port: 0,
      autoStart: false,
      executor: async (action) => {
        callCount++;
        if (callCount === 1) throw new Error('first fails');
        calls.push(action.request);
      },
    });

    s.registerHandler({
      id: 'h1',
      name: 'first',
      sourceId: 's',
      action: { request: 'first' },
      priority: 10,
    });
    s.registerHandler({
      id: 'h2',
      name: 'second',
      sourceId: 's',
      action: { request: 'second' },
      priority: 1,
    });

    const ev = new WebhookEvent({ sourceId: 's', eventType: 'e', payload: {} });
    const result = await s.processEvent(ev);

    // First handler failed, second succeeded
    assert.equal(result.results[0].success, false);
    assert.equal(result.results[1].success, true);
    assert.deepEqual(calls, ['second']);
  });
});
