/**
 * Unit tests for http-gateway.js
 *
 * Tests the HttpGateway class by spinning up a real HTTP server and
 * making requests to it. Uses only Node.js built-ins — no external deps.
 */

import { describe, it, before, after } from 'node:test';
import assert from 'node:assert';
import http from 'node:http';
import { HttpGateway } from '../../src/channels/http-gateway.js';
import { getPluginRegistry, resetPluginRegistry } from '../../src/channels/plugin-api.js';
import { createPaymentCredential } from '../../src/mpp/index.js';
import { createMppHttpRouteHandler } from '../../src/mpp/http.js';

const TRANSIENT_REQUEST_ERROR_CODES = new Set([
  'ECONNRESET',
  'ECONNREFUSED',
  'EPIPE',
  'ETIMEDOUT',
  'EHOSTUNREACH',
  'EAI_AGAIN',
]);
const TRANSIENT_GATEWAY_START_ERROR_CODES = new Set([
  'EADDRINUSE',
  'EACCES',
  'EMFILE',
  'ENFILE',
  'EAGAIN',
]);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Make an HTTP request and return { status, headers, body }.
 * @param {string} url
 * @param {object} [opts]
 * @returns {Promise<{ status: number, headers: object, body: object|string }>}
 */
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function request(url, opts = {}) {
  const maxAttempts = opts.maxAttempts ?? 3;
  let lastError = null;

  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    try {
      return await new Promise((resolve, reject) => {
        const parsed = new URL(url);
        const reqOpts = {
          hostname: parsed.hostname,
          port: parsed.port,
          path: parsed.pathname + parsed.search,
          method: opts.method || 'GET',
          headers: opts.headers || {},
        };

        const req = http.request(reqOpts, (res) => {
          const chunks = [];
          res.on('data', (c) => chunks.push(c));
          res.on('end', () => {
            const raw = Buffer.concat(chunks).toString('utf-8');
            let body;
            try {
              body = JSON.parse(raw);
            } catch {
              body = raw;
            }
            resolve({ status: res.statusCode, headers: res.headers, body });
          });
        });

        req.on('error', reject);
        if (opts.body) req.write(opts.body);
        req.end();
      });
    } catch (error) {
      lastError = error;
      if (
        !TRANSIENT_REQUEST_ERROR_CODES.has(error?.code) ||
        attempt === maxAttempts
      ) {
        throw error;
      }
      await delay(15 * attempt);
    }
  }

  throw lastError;
}

async function startGateway(gateway, maxAttempts = 5) {
  let lastError = null;

  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    try {
      return await gateway.start();
    } catch (error) {
      lastError = error;
      if (
        !TRANSIENT_GATEWAY_START_ERROR_CODES.has(error?.code) ||
        attempt === maxAttempts
      ) {
        throw error;
      }
      await delay(25 * attempt);
    }
  }

  throw lastError;
}

// ===========================================================================
// Gateway with no API keys configured (secure-by-default)
// ===========================================================================

describe('HttpGateway (no keys configured)', () => {
  /** @type {HttpGateway} */
  let gw;
  let baseUrl;

  before(async () => {
    gw = new HttpGateway({ port: 0, host: '127.0.0.1' });
    const addr = await startGateway(gw);
    baseUrl = `http://${addr.host}:${addr.port}`;
  });

  after(async () => {
    await gw.stop();
  });

  // -----------------------------------------------------------------------
  // Health
  // -----------------------------------------------------------------------

  it('GET /health returns 200 with status ok', async () => {
    const res = await request(`${baseUrl}/health`);
    assert.strictEqual(res.status, 200);
    assert.strictEqual(res.body.status, 'ok');
    assert.ok(res.body.uptime >= 0);
    assert.ok(res.body.timestamp);
  });

  it('/health includes security headers', async () => {
    const res = await request(`${baseUrl}/health`);
    assert.strictEqual(res.headers['x-content-type-options'], 'nosniff');
    assert.strictEqual(res.headers['x-frame-options'], 'DENY');
    assert.ok(res.headers['content-security-policy']);
    assert.ok(res.headers['referrer-policy']);
  });

  it('/health includes CORS headers when Origin is allowed', async () => {
    const res = await request(`${baseUrl}/health`, {
      headers: { Origin: 'http://localhost:3000' },
    });
    assert.strictEqual(res.headers['access-control-allow-origin'], 'http://localhost:3000');
  });

  // -----------------------------------------------------------------------
  // CORS preflight
  // -----------------------------------------------------------------------

  it('OPTIONS returns 204 with CORS headers when Origin is allowed', async () => {
    const res = await request(`${baseUrl}/health`, {
      method: 'OPTIONS',
      headers: { Origin: 'http://localhost:3000' },
    });
    assert.strictEqual(res.status, 204);
    assert.strictEqual(res.headers['access-control-allow-origin'], 'http://localhost:3000');
    assert.ok(res.headers['access-control-allow-methods'].includes('GET'));
    assert.ok(res.headers['access-control-allow-methods'].includes('POST'));
  });

  // -----------------------------------------------------------------------
  // 404
  // -----------------------------------------------------------------------

  it('returns 404 for unknown routes', async () => {
    const res = await request(`${baseUrl}/nonexistent`);
    assert.strictEqual(res.status, 404);
    assert.strictEqual(res.body.error, 'Not found');
  });

  // -----------------------------------------------------------------------
  // getAddress
  // -----------------------------------------------------------------------

  it('getAddress returns host and port', () => {
    const addr = gw.getAddress();
    assert.ok(addr);
    assert.strictEqual(addr.host, '127.0.0.1');
    assert.ok(typeof addr.port === 'number');
    assert.ok(addr.port > 0);
  });
});

describe('HttpGateway readiness', () => {
  it('GET /ready returns 200 and probes the configured database path', async () => {
    let observedDbPath = null;
    const gw = new HttpGateway({
      port: 0,
      host: '127.0.0.1',
      dbPath: '/tmp/stateset-ready.db',
      databaseReadinessCheck: async (dbPath) => {
        observedDbPath = dbPath;
      },
    });

    const addr = await startGateway(gw);
    const baseUrl = `http://${addr.host}:${addr.port}`;

    try {
      const res = await request(`${baseUrl}/ready`);
      assert.strictEqual(res.status, 200);
      assert.strictEqual(res.body.status, 'ready');
      assert.strictEqual(res.body.checks.database, 'ok');
      assert.strictEqual(observedDbPath, '/tmp/stateset-ready.db');
    } finally {
      await gw.stop();
    }
  });

  it('GET /ready returns 503 when the database probe fails', async () => {
    const gw = new HttpGateway({
      port: 0,
      host: '127.0.0.1',
      databaseReadinessCheck: async () => {
        throw new Error('database offline');
      },
    });

    const addr = await startGateway(gw);
    const baseUrl = `http://${addr.host}:${addr.port}`;

    try {
      const res = await request(`${baseUrl}/ready`);
      assert.strictEqual(res.status, 503);
      assert.strictEqual(res.body.status, 'not_ready');
      assert.strictEqual(res.body.checks.database, 'unavailable');
    } finally {
      await gw.stop();
    }
  });
});

// ===========================================================================
// Gateway with API keys (authenticated mode)
// ===========================================================================

describe('HttpGateway (authenticated mode)', () => {
  /** @type {HttpGateway} */
  let gw;
  let baseUrl;

  before(async () => {
    gw = new HttpGateway({
      port: 0,
      host: '127.0.0.1',
      apiKeys: [
        { key: 'sk-admin-test', name: 'admin', level: 'admin' },
        { key: 'sk-read-test', name: 'reader', level: 'read' },
      ],
    });
    const addr = await startGateway(gw);
    baseUrl = `http://${addr.host}:${addr.port}`;
  });

  after(async () => {
    await gw.stop();
  });

  it('allows unauthenticated /health (public endpoint)', async () => {
    const res = await request(`${baseUrl}/health`);
    assert.strictEqual(res.status, 200);
  });

  it('rejects unauthenticated request to /metrics', async () => {
    const res = await request(`${baseUrl}/metrics`);
    assert.strictEqual(res.status, 401);
    assert.ok(res.body.error.includes('Authentication'));
  });

  it('allows authenticated request with bearer token', async () => {
    const res = await request(`${baseUrl}/metrics`, {
      headers: { Authorization: 'Bearer sk-admin-test' },
    });
    // May be 200 or 500 depending on whether metrics module is loaded
    // But it should NOT be 401 or 403
    assert.ok(res.status !== 401, 'should not be 401');
    assert.ok(res.status !== 403, 'should not be 403');
  });

  it('rejects authenticated request with query param by default', async () => {
    const res = await request(`${baseUrl}/metrics?api_key=sk-admin-test`);
    assert.strictEqual(res.status, 401);
  });

  it('denies read-level access to admin routes', async () => {
    const res = await request(`${baseUrl}/daemon`, {
      headers: { Authorization: 'Bearer sk-read-test' },
    });
    assert.strictEqual(res.status, 403);
    assert.ok(res.body.error.includes('Forbidden'));
  });
});

// ===========================================================================
// Rate limiting
// ===========================================================================

describe('HttpGateway (rate limiting)', () => {
  /** @type {HttpGateway} */
  let gw;
  let baseUrl;

  before(async () => {
    gw = new HttpGateway({
      port: 0,
      host: '127.0.0.1',
      rateLimitUnauth: 3, // Very low for testing
      rateLimitAuth: 5,
      rateLimitWindowMs: 5000,
    });
    const addr = await startGateway(gw);
    baseUrl = `http://${addr.host}:${addr.port}`;
  });

  after(async () => {
    await gw.stop();
  });

  it('rate-limits unauthenticated /health after threshold', async () => {
    // Make requests up to the limit
    for (let i = 0; i < 3; i++) {
      const res = await request(`${baseUrl}/health`);
      assert.strictEqual(res.status, 200, `Request ${i + 1} should be 200`);
    }

    // Next request should be rate-limited
    const res = await request(`${baseUrl}/health`);
    assert.strictEqual(res.status, 429);
    assert.ok(res.body.error.includes('Too many requests'));
    assert.ok(res.headers['retry-after']);
  });
});

// ===========================================================================
// Sandbox mode
// ===========================================================================

describe('HttpGateway (sandbox)', () => {
  /** @type {HttpGateway} */
  let gw;
  let baseUrl;

  before(async () => {
    gw = new HttpGateway({
      port: 0,
      host: '127.0.0.1',
      apiKeys: [{ key: 'sk-admin-sandbox', name: 'admin', level: 'admin' }],
      sandbox: { browser: true, shell: true },
    });
    const addr = await startGateway(gw);
    baseUrl = `http://${addr.host}:${addr.port}`;
  });

  after(async () => {
    await gw.stop();
  });

  it('blocks browser evaluate when sandbox enabled', async () => {
    const res = await request(`${baseUrl}/browser/evaluate`, {
      method: 'POST',
      headers: { Authorization: 'Bearer sk-admin-sandbox' },
    });
    assert.strictEqual(res.status, 403);
    assert.ok(res.body.error.includes('sandbox'));
  });

  it('blocks daemon when shell sandbox enabled', async () => {
    const res = await request(`${baseUrl}/daemon`, {
      headers: { Authorization: 'Bearer sk-admin-sandbox' },
    });
    assert.strictEqual(res.status, 403);
    assert.ok(res.body.error.includes('sandbox'));
  });

  it('allows health even with sandbox', async () => {
    const res = await request(`${baseUrl}/health`);
    assert.strictEqual(res.status, 200);
  });
});

describe('HttpGateway /browser/evaluate hardening', () => {
  it('disables /browser/evaluate by default', async () => {
    const gw = new HttpGateway({
      port: 0,
      host: '127.0.0.1',
      apiKeys: [{ key: 'sk-admin-eval', name: 'admin', level: 'admin' }],
    });
    gw.setSubsystems({
      browser: {
        async evaluate() {
          return 2;
        },
      },
    });

    const addr = await startGateway(gw);
    const baseUrl = `http://${addr.host}:${addr.port}`;

    try {
      const res = await request(`${baseUrl}/browser/evaluate`, {
        method: 'POST',
        headers: {
          Authorization: 'Bearer sk-admin-eval',
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ expression: '1+1' }),
      });

      assert.strictEqual(res.status, 403);
      assert.ok(res.body.reason.includes('disabled by default'));
    } finally {
      await gw.stop();
    }
  });

  describe('when allowBrowserEvaluate=true', () => {
    /** @type {HttpGateway} */
    let gw;
    let baseUrl;
    let evaluateCalls = 0;
    let lastExpression = null;

    before(async () => {
      gw = new HttpGateway({
        port: 0,
        host: '127.0.0.1',
        apiKeys: [{ key: 'sk-admin-eval-enabled', name: 'admin', level: 'admin' }],
        allowBrowserEvaluate: true,
      });
      gw.setSubsystems({
        browser: {
          async evaluate(expression) {
            evaluateCalls += 1;
            lastExpression = expression;
            return { ok: true, expression };
          },
        },
      });
      const addr = await startGateway(gw);
      baseUrl = `http://${addr.host}:${addr.port}`;
    });

    after(async () => {
      await gw.stop();
    });

    it('allows safe read-only expressions', async () => {
      const res = await request(`${baseUrl}/browser/evaluate`, {
        method: 'POST',
        headers: {
          Authorization: 'Bearer sk-admin-eval-enabled',
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ expression: 'document.title' }),
      });

      assert.strictEqual(res.status, 200);
      assert.strictEqual(res.body.result.ok, true);
      assert.strictEqual(lastExpression, 'document.title');
      assert.ok(evaluateCalls > 0);
    });

    it('rejects dynamic bypass expressions before browser execution', async () => {
      const callsBefore = evaluateCalls;
      const res = await request(`${baseUrl}/browser/evaluate`, {
        method: 'POST',
        headers: {
          Authorization: 'Bearer sk-admin-eval-enabled',
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ expression: "window[['f','etch'].join('')]('https://attacker')" }),
      });

      assert.strictEqual(res.status, 400);
      assert.ok(res.body.error.includes('read-only browser queries'));
      assert.strictEqual(evaluateCalls, callsBefore);
    });

    it('rejects template-literal selector expressions', async () => {
      const callsBefore = evaluateCalls;
      const res = await request(`${baseUrl}/browser/evaluate`, {
        method: 'POST',
        headers: {
          Authorization: 'Bearer sk-admin-eval-enabled',
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          expression: 'document.querySelector(`${document.title}`).textContent',
        }),
      });

      assert.strictEqual(res.status, 400);
      assert.ok(res.body.error.includes('read-only browser queries'));
      assert.strictEqual(evaluateCalls, callsBefore);
    });
  });
});

describe('HttpGateway /browser/navigate URL safety', () => {
  /** @type {HttpGateway} */
  let gw;
  let baseUrl;
  let lastNavigated = null;

  before(async () => {
    gw = new HttpGateway({
      port: 0,
      host: '127.0.0.1',
      apiKeys: [{ key: 'sk-admin-browser', name: 'admin', level: 'admin' }],
    });
    gw.setSubsystems({
      browser: {
        async navigate(url) {
          lastNavigated = url;
        },
      },
    });
    const addr = await startGateway(gw);
    baseUrl = `http://${addr.host}:${addr.port}`;
  });

  after(async () => {
    await gw.stop();
  });

  it('blocks localhost URLs by default', async () => {
    const res = await request(`${baseUrl}/browser/navigate`, {
      method: 'POST',
      headers: {
        Authorization: 'Bearer sk-admin-browser',
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ url: 'http://localhost:8080/private' }),
    });

    assert.strictEqual(res.status, 400);
    assert.ok(String(res.body.error || '').includes('not allowed'));
    assert.strictEqual(lastNavigated, null);
  });

  it('blocks private IPv4 targets by default', async () => {
    const res = await request(`${baseUrl}/browser/navigate`, {
      method: 'POST',
      headers: {
        Authorization: 'Bearer sk-admin-browser',
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ url: 'http://10.0.0.12/admin' }),
    });

    assert.strictEqual(res.status, 400);
    assert.ok(String(res.body.error || '').includes('not allowed'));
    assert.strictEqual(lastNavigated, null);
  });

  it('allows public HTTPS targets', async () => {
    const res = await request(`${baseUrl}/browser/navigate`, {
      method: 'POST',
      headers: {
        Authorization: 'Bearer sk-admin-browser',
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ url: 'https://example.com/products' }),
    });

    assert.strictEqual(res.status, 200);
    assert.strictEqual(lastNavigated, 'https://example.com/products');
  });
});

describe('HttpGateway plugin payment routes', () => {
  /** @type {HttpGateway} */
  let gw;
  let baseUrl;

  before(async () => {
    await resetPluginRegistry();
    await getPluginRegistry().register('mpp-http-test', async (api) => {
      api.registerHttpRoute({
        method: 'GET',
        path: '/headers',
        level: 'none',
        handler: async () => ({
          status: 201,
          headers: {
            'x-test-header': 'ok',
          },
          body: {
            ok: true,
          },
        }),
      });

      api.registerHttpRoute({
        method: 'POST',
        path: '/payable',
        level: 'none',
        handler: createMppHttpRouteHandler({
          routeId: 'POST /payable',
          description: 'Payable plugin route',
          pricing: {
            chainId: 'bitcoin',
            tokenSymbol: 'BTC',
            amount: 0.0001,
            amountSmallest: '10000',
            token: { symbol: 'BTC', decimals: 8, address: null },
          },
          handler: async ({ payment, body }) => ({
            status: 200,
            headers: {
              'x-plugin-handler': 'executed',
            },
            body: {
              ok: true,
              sku: body.sku,
              payer: payment.payer || payment.credential?.payer || null,
            },
          }),
        }),
      });
    });

    gw = new HttpGateway({ port: 0, host: '127.0.0.1' });
    const addr = await startGateway(gw);
    baseUrl = `http://${addr.host}:${addr.port}`;
  });

  after(async () => {
    await gw.stop();
    await resetPluginRegistry();
  });

  it('passes custom plugin response headers through the gateway', async () => {
    const res = await request(`${baseUrl}/headers`);

    assert.strictEqual(res.status, 201);
    assert.strictEqual(res.headers['x-test-header'], 'ok');
    assert.strictEqual(res.body.ok, true);
  });

  it('serves public HTTP service info for gateway discovery', async () => {
    const res = await request(`${baseUrl}/.well-known/service-info`);

    assert.strictEqual(res.status, 200);
    assert.strictEqual(res.body.protocol, 'mpp');
    assert.strictEqual(res.body.transport.type, 'http');
    assert.ok(res.body.discovery.canonicalOpenapiPath);
  });

  it('serves OpenAPI payment discovery for plugin routes', async () => {
    const res = await request(`${baseUrl}/openapi.json`);

    assert.strictEqual(res.status, 200);
    assert.strictEqual(res.body.openapi, '3.1.0');
    assert.strictEqual(res.body['x-service-info'].protocol, 'mpp');
    assert.strictEqual(res.body['x-service-info'].transport.type, 'http');
    assert.strictEqual(res.body.paths['/payable'].post['x-payment-info'].amount.asset, 'BTC');
    assert.strictEqual(res.body.paths['/headers'].get['x-payment-info'], undefined);
  });

  it('supports MPP 402 challenge and receipt flow for plugin routes', async () => {
    const first = await request(`${baseUrl}/payable`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ sku: 'sku_1' }),
    });

    assert.strictEqual(first.status, 402);
    assert.strictEqual(typeof first.headers['payment-required'], 'string');
    assert.strictEqual(first.body.paymentChallenge.tool, 'POST /payable');

    const credential = createPaymentCredential({
      challenge: first.body.paymentChallenge,
      payer: 'buyer-agent',
      authorization: { type: 'http-gateway-test' },
    });

    const second = await request(`${baseUrl}/payable`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        payment: Buffer.from(JSON.stringify(credential), 'utf8').toString('base64url'),
      },
      body: JSON.stringify({ sku: 'sku_1' }),
    });

    assert.strictEqual(second.status, 200);
    assert.strictEqual(second.headers['x-plugin-handler'], 'executed');
    assert.strictEqual(typeof second.headers['payment-response'], 'string');
    assert.strictEqual(second.body.ok, true);
    assert.strictEqual(second.body.sku, 'sku_1');
    assert.strictEqual(second.body.payer, 'buyer-agent');
    assert.strictEqual(second.body._meta.payment.receipt.tool, 'POST /payable');
    assert.strictEqual(second.body._meta.payment.receipt.payer, 'buyer-agent');
  });
});

// ===========================================================================
// Lifecycle
// ===========================================================================

describe('HttpGateway lifecycle', () => {
  it('getAddress returns null before start', () => {
    const gw = new HttpGateway({ port: 0 });
    assert.strictEqual(gw.getAddress(), null);
  });

  it('getAddress returns null after stop', async () => {
    const gw = new HttpGateway({ port: 0 });
    await gw.start();
    await gw.stop();
    assert.strictEqual(gw.getAddress(), null);
  });

  it('stop is safe to call twice', async () => {
    const gw = new HttpGateway({ port: 0 });
    await gw.start();
    await gw.stop();
    await gw.stop(); // should not throw
  });

  it('stop is safe to call without start', async () => {
    const gw = new HttpGateway({ port: 0 });
    await gw.stop(); // should not throw
  });
});
