/**
 * v0.5.0 Permission Sandboxing Tests
 *
 * Tests for:
 * - API key authentication (createApiKeyAuth)
 * - Route permission checking (checkRoutePermission)
 * - Sandbox enforcement (checkSandbox)
 * - HTTP Gateway auth integration
 * - Level enforcement end-to-end
 * - Sandbox mode end-to-end
 * - Config defaults
 */

import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import http from 'http';

// ============================================================================
// Helpers
// ============================================================================

function request(port, method, path, body = null, headers = {}) {
  return new Promise((resolve, reject) => {
    const opts = {
      hostname: '127.0.0.1',
      port,
      path,
      method,
      headers: { ...headers },
    };

    if (body && typeof body === 'object' && !Buffer.isBuffer(body)) {
      body = JSON.stringify(body);
      opts.headers['Content-Type'] = 'application/json';
    }

    const req = http.request(opts, (res) => {
      const chunks = [];
      res.on('data', (c) => chunks.push(c));
      res.on('end', () => {
        const raw = Buffer.concat(chunks).toString('utf-8');
        let parsed;
        try { parsed = JSON.parse(raw); } catch { parsed = raw; }
        resolve({ status: res.statusCode, headers: res.headers, body: parsed });
      });
    });

    req.on('error', reject);
    if (body) req.write(typeof body === 'string' ? body : body);
    req.end();
  });
}

// ============================================================================
// http-auth module unit tests
// ============================================================================

describe('v0.5.0 — http-auth module (unit)', () => {
  let createApiKeyAuth, checkRoutePermission, checkSandbox, LEVELS, ROUTE_PERMISSIONS;

  before(async () => {
    const mod = await import('../src/channels/http-auth.js');
    createApiKeyAuth = mod.createApiKeyAuth;
    checkRoutePermission = mod.checkRoutePermission;
    checkSandbox = mod.checkSandbox;
    LEVELS = mod.LEVELS;
    ROUTE_PERMISSIONS = mod.ROUTE_PERMISSIONS;
  });

  it('LEVELS has correct numeric values', () => {
    assert.equal(LEVELS.none, 0);
    assert.equal(LEVELS.read, 1);
    assert.equal(LEVELS.write, 3);
    assert.equal(LEVELS.delete, 4);
    assert.equal(LEVELS.admin, 5);
  });

  it('ROUTE_PERMISSIONS covers all subsystem prefixes', () => {
    assert.ok(ROUTE_PERMISSIONS['/health']);
    assert.ok(ROUTE_PERMISSIONS['/voice']);
    assert.ok(ROUTE_PERMISSIONS['/browser']);
    assert.ok(ROUTE_PERMISSIONS['/memory']);
    assert.ok(ROUTE_PERMISSIONS['/heartbeat']);
    assert.ok(ROUTE_PERMISSIONS['/daemon']);
  });

  // createApiKeyAuth
  it('empty apiKeys = auth disabled, returns admin', () => {
    const auth = createApiKeyAuth([]);
    const mockReq = { headers: {} };
    const mockUrl = new URL('http://localhost/metrics');
    const result = auth.authenticate(mockReq, mockUrl);
    assert.equal(result.authenticated, true);
    assert.equal(result.identity.level, 'admin');
  });

  it('no token provided returns unauthenticated', () => {
    const auth = createApiKeyAuth([{ key: 'secret123', name: 'test', level: 'admin' }]);
    const mockReq = { headers: {} };
    const mockUrl = new URL('http://localhost/metrics');
    const result = auth.authenticate(mockReq, mockUrl);
    assert.equal(result.authenticated, false);
  });

  it('valid Bearer token authenticates', () => {
    const auth = createApiKeyAuth([{ key: 'secret123', name: 'admin-key', level: 'admin' }]);
    const mockReq = { headers: { authorization: 'Bearer secret123' } };
    const mockUrl = new URL('http://localhost/metrics');
    const result = auth.authenticate(mockReq, mockUrl);
    assert.equal(result.authenticated, true);
    assert.equal(result.identity.name, 'admin-key');
    assert.equal(result.identity.level, 'admin');
  });

  it('valid api_key query param authenticates', () => {
    const auth = createApiKeyAuth([{ key: 'qp-key-456', name: 'query-key', level: 'read' }]);
    const mockReq = { headers: {} };
    const mockUrl = new URL('http://localhost/metrics?api_key=qp-key-456');
    const result = auth.authenticate(mockReq, mockUrl);
    assert.equal(result.authenticated, true);
    assert.equal(result.identity.name, 'query-key');
  });

  it('invalid token rejects', () => {
    const auth = createApiKeyAuth([{ key: 'correct', name: 'test', level: 'admin' }]);
    const mockReq = { headers: { authorization: 'Bearer wrong' } };
    const mockUrl = new URL('http://localhost/metrics');
    const result = auth.authenticate(mockReq, mockUrl);
    assert.equal(result.authenticated, false);
  });

  it('empty key entries are filtered out', () => {
    const auth = createApiKeyAuth([{ key: '', name: 'empty' }, { key: 'real', name: 'real', level: 'read' }]);
    const mockReq = { headers: { authorization: 'Bearer real' } };
    const mockUrl = new URL('http://localhost/metrics');
    const result = auth.authenticate(mockReq, mockUrl);
    assert.equal(result.authenticated, true);
    assert.equal(result.identity.name, 'real');
  });

  // checkRoutePermission
  it('admin can access everything', () => {
    const identity = { name: 'admin', level: 'admin' };
    assert.equal(checkRoutePermission(identity, '/daemon', 'GET').allowed, true);
    assert.equal(checkRoutePermission(identity, '/browser/evaluate', 'POST').allowed, true);
    assert.equal(checkRoutePermission(identity, '/memory/42', 'DELETE').allowed, true);
  });

  it('read level can access GET /metrics', () => {
    const identity = { name: 'reader', level: 'read' };
    assert.equal(checkRoutePermission(identity, '/metrics', 'GET').allowed, true);
  });

  it('read level denied POST /memory/save (requires write)', () => {
    const identity = { name: 'reader', level: 'read' };
    const result = checkRoutePermission(identity, '/memory/save', 'POST');
    assert.equal(result.allowed, false);
    assert.ok(result.reason.includes('write'));
  });

  it('read level denied DELETE /memory/42 (requires delete)', () => {
    const identity = { name: 'reader', level: 'read' };
    const result = checkRoutePermission(identity, '/memory/42', 'DELETE');
    assert.equal(result.allowed, false);
  });

  it('read level denied /daemon (requires admin)', () => {
    const identity = { name: 'reader', level: 'read' };
    const result = checkRoutePermission(identity, '/daemon', 'GET');
    assert.equal(result.allowed, false);
    assert.ok(result.reason.includes('admin'));
  });

  it('write level can POST /voice/transcribe', () => {
    const identity = { name: 'writer', level: 'write' };
    assert.equal(checkRoutePermission(identity, '/voice/transcribe', 'POST').allowed, true);
  });

  it('method-specific override: GET /browser/status uses read', () => {
    const identity = { name: 'reader', level: 'read' };
    assert.equal(checkRoutePermission(identity, '/browser/status', 'GET').allowed, true);
  });

  it('unmatched route defaults to read', () => {
    const identity = { name: 'reader', level: 'read' };
    assert.equal(checkRoutePermission(identity, '/unknown-route', 'GET').allowed, true);
  });

  // checkSandbox
  it('null sandbox blocks nothing', () => {
    assert.equal(checkSandbox(null, '/browser/evaluate').blocked, false);
  });

  it('browser sandbox blocks /browser/evaluate', () => {
    const result = checkSandbox({ browser: true }, '/browser/evaluate');
    assert.equal(result.blocked, true);
    assert.ok(result.reason.includes('browser sandbox'));
  });

  it('browser sandbox blocks /browser/navigate', () => {
    assert.equal(checkSandbox({ browser: true }, '/browser/navigate').blocked, true);
  });

  it('browser sandbox does NOT block /browser/status', () => {
    assert.equal(checkSandbox({ browser: true }, '/browser/status').blocked, false);
  });

  it('browser sandbox does NOT block /browser/content', () => {
    assert.equal(checkSandbox({ browser: true }, '/browser/content').blocked, false);
  });

  it('shell sandbox blocks /daemon', () => {
    assert.equal(checkSandbox({ shell: true }, '/daemon').blocked, true);
  });

  it('shell sandbox does NOT block /metrics', () => {
    assert.equal(checkSandbox({ shell: true }, '/metrics').blocked, false);
  });
});

// ============================================================================
// HTTP Gateway auth integration
// ============================================================================

describe('v0.5.0 — HTTP Gateway auth integration', () => {
  let gw, port;
  const API_KEY = 'test-admin-key-abc123';

  before(async () => {
    const { createHttpGateway } = await import('../src/channels/http-gateway.js');
    gw = createHttpGateway({
      port: 0,
      apiKeys: [{ key: API_KEY, name: 'test-admin', level: 'admin' }],
    });
    const addr = await gw.start();
    port = addr.port;
  });

  after(async () => {
    await gw.stop();
  });

  it('/health bypasses auth (returns 200 without token)', async () => {
    const res = await request(port, 'GET', '/health');
    assert.equal(res.status, 200);
    assert.equal(res.body.status, 'ok');
  });

  it('unauthenticated request to /metrics returns 401', async () => {
    const res = await request(port, 'GET', '/metrics');
    assert.equal(res.status, 401);
    assert.ok(res.body.error.includes('Authentication'));
  });

  it('valid Bearer token to /metrics returns 200', async () => {
    const res = await request(port, 'GET', '/metrics', null, {
      Authorization: `Bearer ${API_KEY}`,
    });
    assert.equal(res.status, 200);
  });

  it('valid api_key query param to /metrics returns 200', async () => {
    const res = await request(port, 'GET', `/metrics?api_key=${API_KEY}`);
    assert.equal(res.status, 200);
  });

  it('invalid Bearer token returns 401', async () => {
    const res = await request(port, 'GET', '/metrics', null, {
      Authorization: 'Bearer wrong-key',
    });
    assert.equal(res.status, 401);
  });

  it('OPTIONS preflight returns 204 without auth', async () => {
    const res = await request(port, 'OPTIONS', '/metrics');
    assert.equal(res.status, 204);
    assert.ok(res.headers['access-control-allow-headers'].includes('Authorization'));
  });
});

// ============================================================================
// Level enforcement end-to-end
// ============================================================================

describe('v0.5.0 — Level enforcement', () => {
  let gw, port;
  const READ_KEY = 'read-only-key-xyz';

  before(async () => {
    const { createHttpGateway } = await import('../src/channels/http-gateway.js');
    gw = createHttpGateway({
      port: 0,
      apiKeys: [{ key: READ_KEY, name: 'reader', level: 'read' }],
    });
    const addr = await gw.start();
    port = addr.port;
  });

  after(async () => {
    await gw.stop();
  });

  it('read key can GET /metrics', async () => {
    const res = await request(port, 'GET', '/metrics', null, {
      Authorization: `Bearer ${READ_KEY}`,
    });
    assert.equal(res.status, 200);
  });

  it('read key denied POST /memory/save (requires write)', async () => {
    const res = await request(port, 'POST', '/memory/save', { summary: 'test' }, {
      Authorization: `Bearer ${READ_KEY}`,
    });
    assert.equal(res.status, 403);
    assert.ok(res.body.reason.includes('write'));
  });

  it('read key denied GET /daemon (requires admin)', async () => {
    const res = await request(port, 'GET', '/daemon', null, {
      Authorization: `Bearer ${READ_KEY}`,
    });
    assert.equal(res.status, 403);
    assert.ok(res.body.reason.includes('admin'));
  });

  it('read key can GET /browser/status (method override to read)', async () => {
    const res = await request(port, 'GET', '/browser/status', null, {
      Authorization: `Bearer ${READ_KEY}`,
    });
    // Will be 501 (no subsystem) but NOT 403 (permission ok)
    assert.equal(res.status, 501);
  });
});

// ============================================================================
// Sandbox mode end-to-end
// ============================================================================

describe('v0.5.0 — Sandbox mode', () => {
  let gw, port;
  const ADMIN_KEY = 'sandbox-admin-key';

  before(async () => {
    const { createHttpGateway } = await import('../src/channels/http-gateway.js');
    gw = createHttpGateway({
      port: 0,
      apiKeys: [{ key: ADMIN_KEY, name: 'admin', level: 'admin' }],
      sandbox: { browser: true, shell: true },
    });
    const addr = await gw.start();
    port = addr.port;
  });

  after(async () => {
    await gw.stop();
  });

  it('POST /browser/evaluate blocked by browser sandbox', async () => {
    const res = await request(port, 'POST', '/browser/evaluate', { expression: '1+1' }, {
      Authorization: `Bearer ${ADMIN_KEY}`,
    });
    assert.equal(res.status, 403);
    assert.ok(res.body.reason.includes('browser sandbox'));
  });

  it('GET /browser/status NOT blocked by sandbox (read-only status)', async () => {
    const res = await request(port, 'GET', '/browser/status', null, {
      Authorization: `Bearer ${ADMIN_KEY}`,
    });
    // 501 (no subsystem) not 403 (not blocked)
    assert.equal(res.status, 501);
  });

  it('GET /browser/links NOT blocked by sandbox (read-only)', async () => {
    const res = await request(port, 'GET', '/browser/links', null, {
      Authorization: `Bearer ${ADMIN_KEY}`,
    });
    assert.equal(res.status, 501);
  });

  it('GET /daemon blocked by shell sandbox', async () => {
    const res = await request(port, 'GET', '/daemon', null, {
      Authorization: `Bearer ${ADMIN_KEY}`,
    });
    assert.equal(res.status, 403);
    assert.ok(res.body.reason.includes('shell sandbox'));
  });

  it('GET /metrics NOT blocked by any sandbox', async () => {
    const res = await request(port, 'GET', '/metrics', null, {
      Authorization: `Bearer ${ADMIN_KEY}`,
    });
    assert.equal(res.status, 200);
  });
});

// ============================================================================
// Backwards compatibility (no auth configured)
// ============================================================================

describe('v0.5.0 — Backwards compat (no apiKeys)', () => {
  let gw, port;

  before(async () => {
    const { createHttpGateway } = await import('../src/channels/http-gateway.js');
    gw = createHttpGateway({ port: 0 });
    const addr = await gw.start();
    port = addr.port;
  });

  after(async () => {
    await gw.stop();
  });

  it('all routes accessible without token when no apiKeys configured', async () => {
    const res = await request(port, 'GET', '/metrics');
    assert.equal(res.status, 200);
  });
});
