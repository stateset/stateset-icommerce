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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Make an HTTP request and return { status, headers, body }.
 * @param {string} url
 * @param {object} [opts]
 * @returns {Promise<{ status: number, headers: object, body: object|string }>}
 */
function request(url, opts = {}) {
  return new Promise((resolve, reject) => {
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
    const addr = await gw.start();
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
    const addr = await gw.start();
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
    const addr = await gw.start();
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
    const addr = await gw.start();
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
