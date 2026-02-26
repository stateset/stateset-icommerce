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

// ============================================================================
// WebhookSource
// ============================================================================

describe('WebhookSource', () => {
  describe('constructor', () => {
    it('sets default values', () => {
      const src = new WebhookSource({ name: 'test', path: '/webhooks/test' });
      assert.equal(src.name, 'test');
      assert.equal(src.path, '/webhooks/test');
      assert.equal(src.enabled, true);
      assert.equal(src.signatureAlgorithm, 'sha256');
      assert.equal(src.signatureHeader, 'x-signature');
      assert.equal(src.eventTypeField, 'type');
      assert.equal(src.retryOnFailure, true);
      assert.equal(src.maxRetries, 3);
      assert.ok(src.id); // auto-generated UUID
    });

    it('accepts custom config', () => {
      const src = new WebhookSource({
        id: 'custom-id',
        name: 'stripe',
        path: '/webhooks/stripe',
        secret: 'whsec_123',
        signatureHeader: 'stripe-signature',
        enabled: false,
      });
      assert.equal(src.id, 'custom-id');
      assert.equal(src.secret, 'whsec_123');
      assert.equal(src.signatureHeader, 'stripe-signature');
      assert.equal(src.enabled, false);
    });
  });

  describe('toJSON', () => {
    it('excludes secret', () => {
      const src = new WebhookSource({ name: 'test', path: '/x', secret: 'top-secret' });
      const json = src.toJSON();
      assert.equal(json.secret, undefined);
      assert.equal(json.name, 'test');
    });

    it('includes all public fields', () => {
      const src = new WebhookSource({ name: 'test', path: '/x' });
      const json = src.toJSON();
      assert.ok('id' in json);
      assert.ok('name' in json);
      assert.ok('path' in json);
      assert.ok('enabled' in json);
      assert.ok('signatureHeader' in json);
      assert.ok('eventTypeField' in json);
      assert.ok('retryOnFailure' in json);
      assert.ok('maxRetries' in json);
    });
  });

  describe('verifySignature', () => {
    it('returns true when no secret is set', () => {
      const src = new WebhookSource({ name: 'test', path: '/x' });
      assert.ok(src.verifySignature('body', 'any-sig'));
    });

    it('returns true for valid HMAC signature', () => {
      const secret = 'my-secret';
      const body = '{"event":"test"}';
      const src = new WebhookSource({ name: 'test', path: '/x', secret });
      const sig = createHmac('sha256', secret).update(body).digest('hex');
      assert.ok(src.verifySignature(body, sig));
    });

    it('returns false for invalid signature', () => {
      const src = new WebhookSource({ name: 'test', path: '/x', secret: 'secret' });
      assert.equal(src.verifySignature('body', 'bad-signature'), false);
    });

    it('supports signature prefix', () => {
      const secret = 'secret';
      const body = 'data';
      const src = new WebhookSource({
        name: 'test', path: '/x', secret,
        signaturePrefix: 'sha256=',
      });
      const sig = 'sha256=' + createHmac('sha256', secret).update(body).digest('hex');
      assert.ok(src.verifySignature(body, sig));
    });
  });
});

// ============================================================================
// WebhookHandler
// ============================================================================

describe('WebhookHandler', () => {
  describe('constructor', () => {
    it('sets default values', () => {
      const h = new WebhookHandler({ name: 'test', sourceId: 's1' });
      assert.equal(h.name, 'test');
      assert.equal(h.sourceId, 's1');
      assert.deepEqual(h.eventTypes, ['*']);
      assert.equal(h.enabled, true);
      assert.equal(h.priority, 0);
      assert.ok(h.id);
    });
  });

  describe('matches', () => {
    it('matches any event type with wildcard', () => {
      const h = new WebhookHandler({ name: 'all', sourceId: 's1' });
      assert.ok(h.matches('any.event', {}));
    });

    it('matches specific event type', () => {
      const h = new WebhookHandler({ name: 'test', sourceId: 's1', eventTypes: ['order.created'] });
      assert.ok(h.matches('order.created', {}));
      assert.equal(h.matches('order.deleted', {}), false);
    });

    it('returns false when disabled', () => {
      const h = new WebhookHandler({ name: 'test', sourceId: 's1', enabled: false });
      assert.equal(h.matches('any', {}), false);
    });

    it('evaluates conditions with nested field matching', () => {
      const h = new WebhookHandler({
        name: 'test', sourceId: 's1',
        conditions: { 'data.status': 'active' },
      });
      assert.ok(h.matches('event', { data: { status: 'active' } }));
      assert.equal(h.matches('event', { data: { status: 'inactive' } }), false);
    });

    it('fails when condition field is missing from payload', () => {
      const h = new WebhookHandler({
        name: 'test', sourceId: 's1',
        conditions: { 'data.status': 'active' },
      });
      assert.equal(h.matches('event', {}), false);
    });
  });

  describe('toJSON', () => {
    it('includes all fields', () => {
      const h = new WebhookHandler({
        name: 'test', sourceId: 's1',
        action: { agent: 'orders', request: 'do something' },
      });
      const json = h.toJSON();
      assert.equal(json.name, 'test');
      assert.equal(json.sourceId, 's1');
      assert.ok(json.action);
      assert.ok(json.id);
    });
  });
});

// ============================================================================
// WebhookEvent
// ============================================================================

describe('WebhookEvent', () => {
  describe('constructor', () => {
    it('creates event with defaults', () => {
      const e = new WebhookEvent({
        sourceId: 's1',
        sourceName: 'Test',
        eventType: 'test.event',
        payload: { data: 1 },
      });
      assert.equal(e.sourceId, 's1');
      assert.equal(e.status, 'pending');
      assert.ok(e.id);
      assert.ok(e.receivedAt);
      assert.deepEqual(e.results, []);
      assert.equal(e.retryCount, 0);
    });
  });

  describe('toJSON', () => {
    it('serializes without headers', () => {
      const e = new WebhookEvent({
        sourceId: 's1',
        sourceName: 'Test',
        eventType: 'test',
        payload: {},
        headers: { authorization: 'Bearer secret' },
      });
      const json = e.toJSON();
      // headers are NOT included in toJSON
      assert.equal(json.headers, undefined);
      assert.equal(json.sourceId, 's1');
    });
  });
});

// ============================================================================
// WebhookServer
// ============================================================================

describe('WebhookServer', () => {
  let server;

  beforeEach(() => {
    server = new WebhookServer({ port: 0, storePath: null });
  });

  describe('constructor', () => {
    it('initializes with defaults', () => {
      assert.equal(server.isRunning, false);
      assert.equal(server.sources.size, 0);
      assert.equal(server.handlers.size, 0);
      assert.deepEqual(server.eventHistory, []);
    });

    it('uses EventEmitter', () => {
      assert.equal(typeof server.on, 'function');
      assert.equal(typeof server.emit, 'function');
    });
  });

  describe('registerSource', () => {
    it('registers a source from config object', () => {
      const src = server.registerSource({ name: 'test', path: '/webhooks/test' });
      assert.ok(server.sources.has(src.id));
      assert.equal(server.sources.size, 1);
    });

    it('registers an existing WebhookSource instance', () => {
      const src = new WebhookSource({ name: 'test', path: '/x' });
      server.registerSource(src);
      assert.ok(server.sources.has(src.id));
    });

    it('emits source:registered event', () => {
      let emitted = null;
      server.on('source:registered', (data) => { emitted = data; });
      server.registerSource({ name: 'test', path: '/x' });
      assert.ok(emitted);
      assert.equal(emitted.source.name, 'test');
    });
  });

  describe('registerHandler', () => {
    it('registers a handler from config object', () => {
      const h = server.registerHandler({
        name: 'test', sourceId: 's1',
        action: { agent: 'orders', request: 'test' },
      });
      assert.ok(server.handlers.has(h.id));
    });

    it('emits handler:registered event', () => {
      let emitted = null;
      server.on('handler:registered', (data) => { emitted = data; });
      server.registerHandler({ name: 'test', sourceId: 's1' });
      assert.ok(emitted);
    });
  });

  describe('findSourceByPath', () => {
    it('finds enabled source by path', () => {
      server.registerSource({ name: 'test', path: '/webhooks/test' });
      const found = server.findSourceByPath('/webhooks/test');
      assert.ok(found);
      assert.equal(found.name, 'test');
    });

    it('returns null for unknown path', () => {
      assert.equal(server.findSourceByPath('/unknown'), null);
    });

    it('skips disabled sources', () => {
      server.registerSource({ name: 'test', path: '/x', enabled: false });
      assert.equal(server.findSourceByPath('/x'), null);
    });
  });

  describe('processEvent', () => {
    it('completes with no matching handlers', async () => {
      const event = new WebhookEvent({
        sourceId: 'unknown', sourceName: 'Test',
        eventType: 'test', payload: {},
      });
      const result = await server.processEvent(event);
      assert.equal(result.status, 'completed');
      assert.ok(result.results.some((r) => r.message === 'No matching handlers'));
    });

    it('executes matching handlers', async () => {
      const src = server.registerSource({ name: 'test', path: '/x' });
      let executedAction = null;
      server.executor = async (action) => { executedAction = action; return 'ok'; };
      server.registerHandler({
        name: 'h1', sourceId: src.id,
        eventTypes: ['order.created'],
        action: { agent: 'orders', request: 'process order' },
      });
      const event = new WebhookEvent({
        sourceId: src.id, sourceName: 'Test',
        eventType: 'order.created', payload: {},
      });
      const result = await server.processEvent(event);
      assert.equal(result.status, 'completed');
      assert.ok(executedAction);
      assert.ok(result.results.some((r) => r.success));
    });

    it('records failed handler execution', async () => {
      const src = server.registerSource({ name: 'test', path: '/x' });
      server.executor = async () => { throw new Error('exec failed'); };
      server.registerHandler({
        name: 'h1', sourceId: src.id,
        action: { agent: 'orders', request: 'x' },
      });
      const event = new WebhookEvent({
        sourceId: src.id, sourceName: 'Test',
        eventType: 'test', payload: {},
      });
      const result = await server.processEvent(event);
      assert.ok(result.results.some((r) => r.success === false));
    });

    it('adds to event history', async () => {
      const src = server.registerSource({ name: 'test', path: '/x' });
      server.registerHandler({ name: 'h1', sourceId: src.id });
      const event = new WebhookEvent({
        sourceId: src.id, sourceName: 'Test',
        eventType: 'test', payload: {},
      });
      await server.processEvent(event);
      assert.equal(server.eventHistory.length, 1);
    });
  });

  describe('interpolateAction', () => {
    it('interpolates payload values into action strings', () => {
      const action = { agent: 'orders', request: 'Process order {order_id}' };
      const event = new WebhookEvent({
        sourceId: 's1', sourceName: 'Test',
        eventType: 'test', payload: { order_id: 'ORD-42' },
      });
      const result = server.interpolateAction(action, event);
      assert.equal(result.request, 'Process order ORD-42');
    });

    it('preserves unmatched placeholders', () => {
      const action = { request: 'Unknown: {missing_field}' };
      const event = new WebhookEvent({
        sourceId: 's1', sourceName: 'Test',
        eventType: 'test', payload: {},
      });
      const result = server.interpolateAction(action, event);
      assert.equal(result.request, 'Unknown: {missing_field}');
    });

    it('passes non-string values through unchanged', () => {
      const action = { agent: 'orders', count: 42 };
      const event = new WebhookEvent({
        sourceId: 's1', sourceName: 'Test',
        eventType: 'test', payload: {},
      });
      const result = server.interpolateAction(action, event);
      assert.equal(result.count, 42);
    });
  });

  describe('getStatus', () => {
    it('returns status object', () => {
      const status = server.getStatus();
      assert.equal(status.isRunning, false);
      assert.equal(status.sourceCount, 0);
      assert.equal(status.handlerCount, 0);
      assert.ok(Array.isArray(status.recentEvents));
    });
  });

  describe('getHistory', () => {
    it('returns empty array initially', () => {
      assert.deepEqual(server.getHistory(), []);
    });

    it('filters by sourceId', async () => {
      const src = server.registerSource({ name: 'test', path: '/x' });
      server.registerHandler({ name: 'h1', sourceId: src.id });
      const e1 = new WebhookEvent({ sourceId: src.id, sourceName: 'T', eventType: 'a', payload: {} });
      await server.processEvent(e1);
      const history = server.getHistory({ sourceId: src.id });
      assert.equal(history.length, 1);
    });

    it('respects limit', async () => {
      const src = server.registerSource({ name: 'test', path: '/x' });
      server.registerHandler({ name: 'h1', sourceId: src.id });
      for (let i = 0; i < 5; i++) {
        const ev = new WebhookEvent({ sourceId: src.id, sourceName: 'T', eventType: 'a', payload: {} });
        await server.processEvent(ev);
      }
      const history = server.getHistory({ limit: 2 });
      assert.equal(history.length, 2);
    });
  });

  describe('listSources / listHandlers', () => {
    it('lists sources as JSON', () => {
      server.registerSource({ name: 'A', path: '/a' });
      const list = server.listSources();
      assert.equal(list.length, 1);
      assert.equal(list[0].name, 'A');
    });

    it('lists handlers as JSON', () => {
      server.registerHandler({ name: 'H', sourceId: 's1' });
      const list = server.listHandlers();
      assert.equal(list.length, 1);
      assert.equal(list[0].name, 'H');
    });
  });
});

// ============================================================================
// Templates
// ============================================================================

describe('WebhookSourceTemplates', () => {
  it('has stripe template', () => {
    assert.ok(WebhookSourceTemplates.stripe);
    assert.equal(WebhookSourceTemplates.stripe.path, '/webhooks/stripe');
  });

  it('has shopify template', () => {
    assert.ok(WebhookSourceTemplates.shopify);
    assert.equal(WebhookSourceTemplates.shopify.path, '/webhooks/shopify');
  });

  it('has square template', () => {
    assert.ok(WebhookSourceTemplates.square);
  });

  it('has shippo template', () => {
    assert.ok(WebhookSourceTemplates.shippo);
  });

  it('has carrier hub template', () => {
    assert.ok(WebhookSourceTemplates.carrierHub);
    assert.equal(WebhookSourceTemplates.carrierHub.path, '/webhooks/carrier-hub');
  });

  it('has avalara template', () => {
    assert.ok(WebhookSourceTemplates.avalara);
    assert.equal(WebhookSourceTemplates.avalara.path, '/webhooks/avalara');
  });

  it('has taxjar template', () => {
    assert.ok(WebhookSourceTemplates.taxjar);
    assert.equal(WebhookSourceTemplates.taxjar.path, '/webhooks/taxjar');
  });

  it('has custom template', () => {
    assert.ok(WebhookSourceTemplates.custom);
  });
});

describe('WebhookHandlerTemplates', () => {
  it('has stripePaymentSucceeded', () => {
    assert.ok(WebhookHandlerTemplates.stripePaymentSucceeded);
    assert.ok(WebhookHandlerTemplates.stripePaymentSucceeded.eventTypes.includes('payment_intent.succeeded'));
  });

  it('has stripePaymentFailed', () => {
    assert.ok(WebhookHandlerTemplates.stripePaymentFailed);
  });

  it('has stripePaymentCanceled', () => {
    assert.ok(WebhookHandlerTemplates.stripePaymentCanceled);
    assert.ok(WebhookHandlerTemplates.stripePaymentCanceled.eventTypes.includes('payment_intent.canceled'));
  });

  it('has shopifyOrderCreated', () => {
    assert.ok(WebhookHandlerTemplates.shopifyOrderCreated);
  });

  it('has shippoTrackingUpdate', () => {
    assert.ok(WebhookHandlerTemplates.shippoTrackingUpdate);
  });

  it('has carrierHubTrackingEvent', () => {
    assert.ok(WebhookHandlerTemplates.carrierHubTrackingEvent);
    assert.ok(WebhookHandlerTemplates.carrierHubTrackingEvent.eventTypes.includes('shipment.delivered'));
  });

  it('has avalaraTransactionCommitted', () => {
    assert.ok(WebhookHandlerTemplates.avalaraTransactionCommitted);
    assert.ok(WebhookHandlerTemplates.avalaraTransactionCommitted.eventTypes.includes('transaction.committed'));
  });

  it('has taxProviderTransactionVoided', () => {
    assert.ok(WebhookHandlerTemplates.taxProviderTransactionVoided);
    assert.ok(WebhookHandlerTemplates.taxProviderTransactionVoided.eventTypes.includes('transaction.voided'));
  });
});
