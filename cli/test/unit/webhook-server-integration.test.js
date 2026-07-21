/**
 * Integration tests for WebhookServer, WebhookSource, and source templates.
 *
 * Uses Node built-in test runner + assert/strict.
 * HTTP tests use the built-in `http` module (no external deps).
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'crypto';
import http from 'http';

import {
  WebhookServer,
  WebhookSource,
  WebhookSourceTemplates,
  getStripeSourceTemplate,
} from '../../src/webhooks/server.js';
import { verifyStripeSignature } from '../../src/adapters/stripe/signature.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const TEST_SECRET = 'whsec_test_secret_integration';

/** Build a Stripe v1 signature header for testing. */
function makeStripeSignatureHeader(body, secret, timestamp = Math.floor(Date.now() / 1000)) {
  const sig = crypto
    .createHmac('sha256', secret)
    .update(`${timestamp}.${body}`, 'utf-8')
    .digest('hex');
  return `t=${timestamp},v1=${sig}`;
}

/** Build a WooCommerce base64 HMAC-SHA256 signature for testing. */
function makeWooCommerceSignature(body, secret) {
  return crypto.createHmac('sha256', secret).update(body, 'utf-8').digest('base64');
}

/**
 * Make an HTTP request and return { statusCode, headers, body }.
 * Uses Node built-in `http` module only.
 */
function request(port, { method = 'POST', path = '/', headers = {}, body = '' } = {}) {
  return new Promise((resolve, reject) => {
    const req = http.request({ hostname: '127.0.0.1', port, method, path, headers }, (res) => {
      const chunks = [];
      res.on('data', (c) => chunks.push(c));
      res.on('end', () => {
        const raw = Buffer.concat(chunks).toString();
        let json;
        try {
          json = JSON.parse(raw);
        } catch {
          json = null;
        }
        resolve({ statusCode: res.statusCode, headers: res.headers, body: raw, json });
      });
    });
    req.on('error', reject);
    if (body) req.write(body);
    req.end();
  });
}

/**
 * Start a WebhookServer on port 0 (random), return the actual OS-assigned port.
 * The caller must call `server.stop()` in afterEach.
 *
 * The WebhookServer.start() emits `this.port` in the `started` event, which
 * stays 0 when we pass port=0. We read the real port from the underlying
 * `server.server.address().port` once the listen callback fires.
 */
function startServer(server) {
  return new Promise((resolve, reject) => {
    server.on('started', () => {
      const actualPort = server.server.address().port;
      resolve(actualPort);
    });
    server.on('error', ({ error }) => reject(error));
    // Use port 0 for OS-assigned random port
    server.port = 0;
    server.start();
  });
}

// ===========================================================================
// 1. WebhookSource with customVerifier
// ===========================================================================

describe('WebhookSource — customVerifier', () => {
  it('calls customVerifier instead of generic HMAC when set', () => {
    let called = false;
    const source = new WebhookSource({
      name: 'test',
      path: '/test',
      secret: 'secret123',
      customVerifier: (_rawBody, _sig, _secret) => {
        called = true;
        return { valid: true };
      },
    });

    source.verifySignature('body', 'sig-header');
    assert.ok(called, 'customVerifier should have been called');
  });

  it('passes (rawBody, signatureHeader, secret) to customVerifier', () => {
    const received = {};
    const source = new WebhookSource({
      name: 'test',
      path: '/test',
      secret: 'my-secret',
      customVerifier: (rawBody, sigHeader, secret) => {
        received.rawBody = rawBody;
        received.sigHeader = sigHeader;
        received.secret = secret;
        return { valid: true };
      },
    });

    source.verifySignature('the-body', 'the-signature');
    assert.equal(received.rawBody, 'the-body');
    assert.equal(received.sigHeader, 'the-signature');
    assert.equal(received.secret, 'my-secret');
  });

  it('returns true when customVerifier returns { valid: true }', () => {
    const source = new WebhookSource({
      name: 'test',
      path: '/test',
      secret: 'secret',
      customVerifier: () => ({ valid: true }),
    });
    assert.equal(source.verifySignature('body', 'sig'), true);
  });

  it('returns false when customVerifier returns { valid: false }', () => {
    const source = new WebhookSource({
      name: 'test',
      path: '/test',
      secret: 'secret',
      customVerifier: () => ({ valid: false, error: 'bad sig' }),
    });
    assert.equal(source.verifySignature('body', 'sig'), false);
  });

  it('falls back to generic HMAC when customVerifier is null', () => {
    const secret = 'hmac-secret';
    const payload = 'test-payload';
    const expected = crypto.createHmac('sha256', secret).update(payload).digest('hex');

    const source = new WebhookSource({
      name: 'generic',
      path: '/generic',
      secret,
      signatureAlgorithm: 'sha256',
      signaturePrefix: '',
      customVerifier: null,
    });

    assert.equal(source.verifySignature(payload, expected), true);
    assert.equal(source.verifySignature(payload, 'wrong-signature'), false);
  });
});

// ===========================================================================
// 2. Stripe template with real v1 verification
// ===========================================================================

describe('Stripe template — v1 signature verification', () => {
  it('getStripeSourceTemplate() returns a template with customVerifier', async () => {
    const template = await getStripeSourceTemplate();
    assert.equal(typeof template.customVerifier, 'function');
    assert.equal(template.name, 'Stripe');
    assert.equal(template.path, '/webhooks/stripe');
    assert.equal(template.signatureHeader, 'stripe-signature');
  });

  it('verifies a valid Stripe v1 signature through WebhookSource', async () => {
    const template = await getStripeSourceTemplate();
    const source = new WebhookSource({ ...template, secret: TEST_SECRET });

    const body = JSON.stringify({ type: 'payment_intent.succeeded', data: { object: {} } });
    const header = makeStripeSignatureHeader(body, TEST_SECRET);

    assert.equal(source.verifySignature(body, header), true);
  });

  it('rejects invalid Stripe signatures', async () => {
    const template = await getStripeSourceTemplate();
    const source = new WebhookSource({ ...template, secret: TEST_SECRET });

    const body = JSON.stringify({ type: 'charge.failed', data: { object: {} } });
    const timestamp = Math.floor(Date.now() / 1000);
    // Sign with a different secret
    const badHeader = makeStripeSignatureHeader(body, 'wrong-secret', timestamp);

    assert.equal(source.verifySignature(body, badHeader), false);
  });

  it('rejects expired timestamps (>300s old)', async () => {
    const template = await getStripeSourceTemplate();
    const source = new WebhookSource({ ...template, secret: TEST_SECRET });

    const body = JSON.stringify({ type: 'invoice.paid', data: { object: {} } });
    const oldTimestamp = Math.floor(Date.now() / 1000) - 600; // 10 minutes ago
    const header = makeStripeSignatureHeader(body, TEST_SECRET, oldTimestamp);

    assert.equal(source.verifySignature(body, header), false);
  });

  it('rejects missing signature header', async () => {
    const template = await getStripeSourceTemplate();
    const source = new WebhookSource({ ...template, secret: TEST_SECRET });

    const body = JSON.stringify({ type: 'checkout.session.completed' });
    assert.equal(source.verifySignature(body, ''), false);
    assert.equal(source.verifySignature(body, undefined), false);
  });
});

// ===========================================================================
// 3. WooCommerce template verification
// ===========================================================================

describe('WooCommerce template — base64 HMAC-SHA256 verification', () => {
  const wooTemplate = WebhookSourceTemplates.woocommerce;

  it('has a customVerifier function', () => {
    assert.equal(typeof wooTemplate.customVerifier, 'function');
  });

  it('verifies valid base64 HMAC-SHA256 signatures', () => {
    const secret = 'wc_webhook_secret_42';
    const source = new WebhookSource({ ...wooTemplate, secret });

    const body = JSON.stringify({ action: 'woocommerce_order_created', arg: { id: 99 } });
    const sig = makeWooCommerceSignature(body, secret);

    assert.equal(source.verifySignature(body, sig), true);
  });

  it('rejects invalid signatures', () => {
    const secret = 'wc_secret';
    const source = new WebhookSource({ ...wooTemplate, secret });

    const body = JSON.stringify({ action: 'woocommerce_order_updated' });
    assert.equal(source.verifySignature(body, 'definitely-not-valid-base64-sig'), false);
  });

  it('rejects empty signature header', () => {
    const secret = 'wc_secret';
    const source = new WebhookSource({ ...wooTemplate, secret });

    const body = JSON.stringify({ action: 'woocommerce_product_deleted' });
    assert.equal(source.verifySignature(body, ''), false);
  });

  it('path is /webhooks/woocommerce', () => {
    assert.equal(wooTemplate.path, '/webhooks/woocommerce');
    assert.equal(wooTemplate.signatureHeader, 'x-wc-webhook-signature');
  });
});

// ===========================================================================
// 4. WebhookServer HTTP routing
// ===========================================================================

describe('WebhookServer — HTTP routing', () => {
  /** @type {WebhookServer} */
  let server;
  /** @type {number} */
  let port;

  beforeEach(async () => {
    server = new WebhookServer({ port: 0, host: '127.0.0.1' });

    // Register a simple source with a known secret
    server.registerSource({
      name: 'test-source',
      path: '/webhooks/test',
      secret: TEST_SECRET,
      signatureHeader: 'x-signature',
      signatureAlgorithm: 'sha256',
      signaturePrefix: '',
      eventTypeField: 'type',
    });

    port = await startServer(server);
  });

  afterEach(async () => {
    if (server && server.isRunning) {
      await server.stop();
    }
  });

  it('starts on a random port and is listening', () => {
    assert.ok(port > 0, `Expected port > 0, got ${port}`);
    assert.ok(server.isRunning, 'Server should be running');
  });

  it('POST to registered webhook source path returns 200', async () => {
    const body = JSON.stringify({ type: 'test.event', value: 42 });
    const sig = crypto.createHmac('sha256', TEST_SECRET).update(body).digest('hex');

    const res = await request(port, {
      path: '/webhooks/test',
      headers: {
        'content-type': 'application/json',
        'x-signature': sig,
      },
      body,
    });

    assert.equal(res.statusCode, 200);
    assert.equal(res.json.received, true);
    assert.ok(res.json.eventId, 'Should return an eventId');
  });

  it('POST to unregistered path returns 404', async () => {
    const body = JSON.stringify({ type: 'test.event' });
    const res = await request(port, {
      path: '/webhooks/nonexistent',
      headers: { 'content-type': 'application/json' },
      body,
    });
    assert.equal(res.statusCode, 404);
    assert.match(res.json.error, /unknown/i);
  });

  it('invalid signature returns 401', async () => {
    const body = JSON.stringify({ type: 'test.event' });
    const res = await request(port, {
      path: '/webhooks/test',
      headers: {
        'content-type': 'application/json',
        'x-signature': 'bad-signature-value',
      },
      body,
    });
    assert.equal(res.statusCode, 401);
    assert.match(res.json.error, /signature/i);
  });

  it('invalid JSON body returns 400', async () => {
    const body = 'this is not json {{{';
    const sig = crypto.createHmac('sha256', TEST_SECRET).update(body).digest('hex');

    const res = await request(port, {
      path: '/webhooks/test',
      headers: {
        'content-type': 'application/json',
        'x-signature': sig,
      },
      body,
    });
    assert.equal(res.statusCode, 400);
    assert.match(res.json.error, /json/i);
  });

  it('GET /health returns 200 with status ok', async () => {
    const res = await request(port, {
      method: 'GET',
      path: '/health',
    });
    assert.equal(res.statusCode, 200);
    assert.equal(res.json.status, 'ok');
    assert.ok(res.json.timestamp, 'Should include a timestamp');
  });

  it('server stop/cleanup works', async () => {
    assert.ok(server.isRunning);
    await server.stop();
    assert.equal(server.isRunning, false);

    // Attempting a connection should fail
    await assert.rejects(
      () => request(port, { method: 'GET', path: '/health' }),
      (err) => err.code === 'ECONNREFUSED',
    );
  });

  it('rejects payloads larger than maxPayloadBytes via Content-Length', async () => {
    // Create a server with a tiny payload limit
    const smallServer = new WebhookServer({
      port: 0,
      host: '127.0.0.1',
      maxPayloadBytes: 256,
    });
    smallServer.registerSource({
      name: 'tiny',
      path: '/webhooks/tiny',
      secret: null, // no sig check for this test
      eventTypeField: 'type',
    });
    const smallPort = await startServer(smallServer);

    try {
      const largeBody = JSON.stringify({ type: 'big', data: 'x'.repeat(512) });
      const res = await request(smallPort, {
        path: '/webhooks/tiny',
        headers: {
          'content-type': 'application/json',
          'content-length': String(Buffer.byteLength(largeBody)),
        },
        body: largeBody,
      });
      assert.equal(res.statusCode, 413);
      assert.match(res.json.error, /too large/i);
    } finally {
      await smallServer.stop();
    }
  });
});

// ===========================================================================
// Bonus: end-to-end Stripe webhook through the HTTP server
// ===========================================================================

describe('WebhookServer — Stripe end-to-end via HTTP', () => {
  /** @type {WebhookServer} */
  let server;
  /** @type {number} */
  let port;

  beforeEach(async () => {
    server = new WebhookServer({ port: 0, host: '127.0.0.1' });
  });

  afterEach(async () => {
    if (server && server.isRunning) {
      await server.stop();
    }
  });

  it('accepts a valid Stripe webhook POST end-to-end', async () => {
    const template = await getStripeSourceTemplate();
    server.registerSource({ ...template, secret: TEST_SECRET });
    port = await startServer(server);

    const body = JSON.stringify({
      type: 'payment_intent.succeeded',
      data: { object: { id: 'pi_test', amount: 5000 } },
    });
    const header = makeStripeSignatureHeader(body, TEST_SECRET);

    const res = await request(port, {
      path: '/webhooks/stripe',
      headers: {
        'content-type': 'application/json',
        'stripe-signature': header,
      },
      body,
    });

    assert.equal(res.statusCode, 200);
    assert.equal(res.json.received, true);
  });

  it('rejects a Stripe webhook POST with tampered body', async () => {
    const template = await getStripeSourceTemplate();
    server.registerSource({ ...template, secret: TEST_SECRET });
    port = await startServer(server);

    const originalBody = JSON.stringify({ type: 'charge.succeeded', data: { object: {} } });
    const header = makeStripeSignatureHeader(originalBody, TEST_SECRET);

    // Tamper with the body after signing
    const tamperedBody = JSON.stringify({
      type: 'charge.succeeded',
      data: { object: { tampered: true } },
    });

    const res = await request(port, {
      path: '/webhooks/stripe',
      headers: {
        'content-type': 'application/json',
        'stripe-signature': header,
      },
      body: tamperedBody,
    });

    assert.equal(res.statusCode, 401);
    assert.match(res.json.error, /signature/i);
  });
});
