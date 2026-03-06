/**
 * v0.3.0 Integration Tests
 *
 * Tests for:
 * - HttpGateway creation, start, stop
 * - setSubsystems method
 * - Voice/Browser/Memory routes return 501 when subsystem disabled
 * - Memory save + search + vector-search + hybrid-search end-to-end
 * - WebChat HTML response via plugin routes
 * - Orchestrator getStatus() includes subsystem info
 * - Package version is 0.3.0
 * - Config version matches
 */

import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import http from 'http';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { readFileSync, mkdirSync, rmSync } from 'node:fs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const TMP_DIR = join(__dirname, '.tmp-v030-test');
const MEMORY_SKIP_REASON = 'Skipping: better-sqlite3 native module not available.';
let memoryAvailable = true;
const API_KEY = 'test-admin-key-v030';
const AUTH_HEADERS = { Authorization: `Bearer ${API_KEY}` };

// ============================================================================
// Helpers
// ============================================================================

/**
 * Make an HTTP request and return { status, headers, body }.
 */
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
// Version Tests
// ============================================================================

describe('v0.3.0 — Package version', () => {
  it('package.json version should be 0.7.21', () => {
    const pkg = JSON.parse(readFileSync(join(__dirname, '..', 'package.json'), 'utf-8'));
    assert.equal(pkg.version, '0.7.21');
  });

  it('config CLI_VERSION should be 0.7.21', async () => {
    const config = await import('../src/config.js');
    assert.equal(config.CLI_VERSION, '0.7.21');
  });

  it('package.json should have botbuilder in optionalDependencies', () => {
    const pkg = JSON.parse(readFileSync(join(__dirname, '..', 'package.json'), 'utf-8'));
    assert.ok(pkg.optionalDependencies.botbuilder);
  });

  it('package.json should have matrix-js-sdk in optionalDependencies', () => {
    const pkg = JSON.parse(readFileSync(join(__dirname, '..', 'package.json'), 'utf-8'));
    assert.ok(pkg.optionalDependencies['matrix-js-sdk']);
  });

  it('package.json should have ws in optionalDependencies', () => {
    const pkg = JSON.parse(readFileSync(join(__dirname, '..', 'package.json'), 'utf-8'));
    assert.ok(pkg.optionalDependencies.ws);
  });
});

// ============================================================================
// HttpGateway Tests
// ============================================================================

describe('v0.3.0 — HttpGateway lifecycle', () => {
  let HttpGateway, createHttpGateway;

  before(async () => {
    const mod = await import('../src/channels/http-gateway.js');
    HttpGateway = mod.HttpGateway;
    createHttpGateway = mod.createHttpGateway;
  });

  it('createHttpGateway returns an HttpGateway instance', () => {
    const gw = createHttpGateway({ port: 0 });
    assert.ok(gw instanceof HttpGateway);
  });

  it('HttpGateway has setSubsystems method', () => {
    const gw = createHttpGateway({ port: 0 });
    assert.equal(typeof gw.setSubsystems, 'function');
  });

  it('start and stop on random port', async () => {
    const gw = createHttpGateway({ port: 0 });
    const addr = await gw.start();
    assert.ok(addr.port > 0);
    assert.ok(addr.host);
    const addrCheck = gw.getAddress();
    assert.equal(addrCheck.port, addr.port);
    await gw.stop();
    assert.equal(gw.getAddress(), null);
  });

  it('health endpoint returns ok', async () => {
    const gw = createHttpGateway({ port: 0 });
    const addr = await gw.start();
    try {
      const res = await request(addr.port, 'GET', '/health');
      assert.equal(res.status, 200);
      assert.equal(res.body.status, 'ok');
      assert.ok(res.body.uptime >= 0);
    } finally {
      await gw.stop();
    }
  });

  it('CORS preflight returns 204', async () => {
    const gw = createHttpGateway({ port: 0 });
    const addr = await gw.start();
    try {
      const res = await request(addr.port, 'OPTIONS', '/health', null, {
        Origin: 'http://localhost:3000',
      });
      assert.equal(res.status, 204);
      assert.ok(res.headers['access-control-allow-methods'].includes('DELETE'));
    } finally {
      await gw.stop();
    }
  });

  it('unknown route returns 404', async () => {
    const gw = createHttpGateway({ port: 0 });
    const addr = await gw.start();
    try {
      const res = await request(addr.port, 'GET', '/nonexistent');
      assert.equal(res.status, 404);
    } finally {
      await gw.stop();
    }
  });
});

// ============================================================================
// Voice/Browser/Memory routes return 501 when disabled
// ============================================================================

describe('v0.3.0 — Subsystem routes return 501 when disabled', () => {
  let gw, port;

  before(async () => {
    const { createHttpGateway } = await import('../src/channels/http-gateway.js');
    gw = createHttpGateway({
      port: 0,
      apiKeys: [{ key: API_KEY, name: 'test-admin', level: 'admin' }],
    });
    const addr = await gw.start();
    port = addr.port;
    // Do NOT call setSubsystems — subsystems remain null
  });

  after(async () => {
    await gw.stop();
  });

  // Voice routes
  it('GET /voice/status → 501', async () => {
    const res = await request(port, 'GET', '/voice/status', null, AUTH_HEADERS);
    assert.equal(res.status, 501);
    assert.ok(res.body.error.includes('Voice'));
  });

  it('POST /voice/transcribe → 501', async () => {
    const res = await request(port, 'POST', '/voice/transcribe', Buffer.alloc(0), AUTH_HEADERS);
    assert.equal(res.status, 501);
  });

  it('POST /voice/synthesize → 501', async () => {
    const res = await request(port, 'POST', '/voice/synthesize', { text: 'hello' }, AUTH_HEADERS);
    assert.equal(res.status, 501);
  });

  it('POST /voice/session/enable/:id → 501', async () => {
    const res = await request(port, 'POST', '/voice/session/enable/test-session', null, AUTH_HEADERS);
    assert.equal(res.status, 501);
  });

  it('POST /voice/session/disable/:id → 501', async () => {
    const res = await request(
      port,
      'POST',
      '/voice/session/disable/test-session',
      null,
      AUTH_HEADERS,
    );
    assert.equal(res.status, 501);
  });

  // Browser routes
  it('GET /browser/status → 501', async () => {
    const res = await request(port, 'GET', '/browser/status', null, AUTH_HEADERS);
    assert.equal(res.status, 501);
    assert.ok(res.body.error.includes('Browser'));
  });

  it('POST /browser/navigate → 501', async () => {
    const res = await request(
      port,
      'POST',
      '/browser/navigate',
      { url: 'https://example.com' },
      AUTH_HEADERS,
    );
    assert.equal(res.status, 501);
  });

  it('POST /browser/screenshot → 501', async () => {
    const res = await request(port, 'POST', '/browser/screenshot', null, AUTH_HEADERS);
    assert.equal(res.status, 501);
  });

  it('POST /browser/evaluate → 501', async () => {
    const res = await request(port, 'POST', '/browser/evaluate', { expression: '1+1' }, AUTH_HEADERS);
    assert.equal(res.status, 501);
  });

  it('POST /browser/click → 501', async () => {
    const res = await request(port, 'POST', '/browser/click', { selector: '#btn' }, AUTH_HEADERS);
    assert.equal(res.status, 501);
  });

  it('POST /browser/type → 501', async () => {
    const res = await request(
      port,
      'POST',
      '/browser/type',
      { selector: '#input', text: 'hi' },
      AUTH_HEADERS,
    );
    assert.equal(res.status, 501);
  });

  it('GET /browser/content → 501', async () => {
    const res = await request(port, 'GET', '/browser/content', null, AUTH_HEADERS);
    assert.equal(res.status, 501);
  });

  it('GET /browser/links → 501', async () => {
    const res = await request(port, 'GET', '/browser/links', null, AUTH_HEADERS);
    assert.equal(res.status, 501);
  });

  it('POST /browser/close → 501', async () => {
    const res = await request(port, 'POST', '/browser/close', null, AUTH_HEADERS);
    assert.equal(res.status, 501);
  });

  // Memory routes
  it('GET /memory/stats → 501', async () => {
    const res = await request(port, 'GET', '/memory/stats', null, AUTH_HEADERS);
    assert.equal(res.status, 501);
    assert.ok(res.body.error.includes('Memory'));
  });

  it('POST /memory/save → 501', async () => {
    const res = await request(port, 'POST', '/memory/save', { summary: 'test' }, AUTH_HEADERS);
    assert.equal(res.status, 501);
  });

  it('POST /memory/search → 501', async () => {
    const res = await request(port, 'POST', '/memory/search', { query: 'test' }, AUTH_HEADERS);
    assert.equal(res.status, 501);
  });

  it('POST /memory/vector-search → 501', async () => {
    const res = await request(port, 'POST', '/memory/vector-search', { query: 'test' }, AUTH_HEADERS);
    assert.equal(res.status, 501);
  });

  it('POST /memory/hybrid-search → 501', async () => {
    const res = await request(port, 'POST', '/memory/hybrid-search', { query: 'test' }, AUTH_HEADERS);
    assert.equal(res.status, 501);
  });

  it('GET /memory/recent/:channel/:senderId → 501', async () => {
    const res = await request(port, 'GET', '/memory/recent/http/user1', null, AUTH_HEADERS);
    assert.equal(res.status, 501);
  });

  it('POST /memory/backfill → 501', async () => {
    const res = await request(port, 'POST', '/memory/backfill', {}, AUTH_HEADERS);
    assert.equal(res.status, 501);
  });

  it('DELETE /memory/:id → 501', async () => {
    const res = await request(port, 'DELETE', '/memory/42', null, AUTH_HEADERS);
    assert.equal(res.status, 501);
  });
});

// ============================================================================
// Memory end-to-end (with in-memory SQLite)
// ============================================================================

describe('v0.3.0 — Memory API end-to-end', () => {
  let gw, port, memoryStore;
  const memDbPath = join(TMP_DIR, 'mem-test.db');

  before(async () => {
    try {
      mkdirSync(TMP_DIR, { recursive: true });
      const { createHttpGateway } = await import('../src/channels/http-gateway.js');
      const { getVectorMemoryStore, resetVectorMemoryStore } = await import('../src/memory/vector-store.js');
      const { resetMemoryStore } = await import('../src/memory/store.js');

      // Reset singletons to get fresh stores with a temp file DB
      // (in-memory won't work because vector store and memory store need the same DB)
      resetVectorMemoryStore();
      resetMemoryStore();
      memoryStore = getVectorMemoryStore({ dbPath: memDbPath });

      gw = createHttpGateway({
        port: 0,
        apiKeys: [{ key: API_KEY, name: 'test-admin', level: 'admin' }],
      });
      gw.setSubsystems({ memory: memoryStore });
      const addr = await gw.start();
      port = addr.port;
    } catch (error) {
      if (error?.code === 'ERR_DLOPEN_FAILED') {
        memoryAvailable = false;
        return;
      }
      throw error;
    }
  });

  after(async () => {
    if (!memoryAvailable) return;
    await gw.stop();
    memoryStore.close();
    try { rmSync(TMP_DIR, { recursive: true, force: true }); } catch {}
  });

  it('GET /memory/stats returns stats', async (t) => {
    if (!memoryAvailable) return t.skip(MEMORY_SKIP_REASON);
    const res = await request(port, 'GET', '/memory/stats', null, AUTH_HEADERS);
    assert.equal(res.status, 200);
    assert.ok('totalMemories' in res.body);
    assert.ok('totalVectors' in res.body);
    assert.ok('dim' in res.body);
  });

  it('POST /memory/save stores a memory', async (t) => {
    if (!memoryAvailable) return t.skip(MEMORY_SKIP_REASON);
    const res = await request(port, 'POST', '/memory/save', {
      summary: 'Customer asked about return policy for electronics',
      facts: 'return_window:30_days',
      channel: 'http',
      senderId: 'test-user',
    }, AUTH_HEADERS);
    assert.equal(res.status, 200);
    assert.ok(res.body.id);
  });

  it('POST /memory/save requires summary', async (t) => {
    if (!memoryAvailable) return t.skip(MEMORY_SKIP_REASON);
    const res = await request(port, 'POST', '/memory/save', { channel: 'http' }, AUTH_HEADERS);
    assert.equal(res.status, 400);
  });

  it('POST /memory/search finds saved memory', async (t) => {
    if (!memoryAvailable) return t.skip(MEMORY_SKIP_REASON);
    const res = await request(port, 'POST', '/memory/search', {
      query: 'return policy',
      channel: 'http',
      senderId: 'test-user',
    }, AUTH_HEADERS);
    assert.equal(res.status, 200);
    assert.ok(Array.isArray(res.body.results));
    assert.ok(res.body.results.length > 0);
    assert.ok(res.body.results[0].summary.includes('return policy'));
  });

  it('POST /memory/vector-search finds saved memory', async (t) => {
    if (!memoryAvailable) return t.skip(MEMORY_SKIP_REASON);
    const res = await request(port, 'POST', '/memory/vector-search', {
      query: 'electronics return policy',
    }, AUTH_HEADERS);
    assert.equal(res.status, 200);
    assert.ok(Array.isArray(res.body.results));
    assert.ok(res.body.results.length > 0);
  });

  it('POST /memory/hybrid-search finds saved memory', async (t) => {
    if (!memoryAvailable) return t.skip(MEMORY_SKIP_REASON);
    const res = await request(port, 'POST', '/memory/hybrid-search', {
      query: 'return policy',
      channel: 'http',
      senderId: 'test-user',
    }, AUTH_HEADERS);
    assert.equal(res.status, 200);
    assert.ok(Array.isArray(res.body.results));
    assert.ok(res.body.results.length > 0);
  });

  it('POST /memory/vector-search requires query', async (t) => {
    if (!memoryAvailable) return t.skip(MEMORY_SKIP_REASON);
    const res = await request(port, 'POST', '/memory/vector-search', {}, AUTH_HEADERS);
    assert.equal(res.status, 400);
  });

  it('POST /memory/backfill succeeds', async (t) => {
    if (!memoryAvailable) return t.skip(MEMORY_SKIP_REASON);
    const res = await request(port, 'POST', '/memory/backfill', {}, AUTH_HEADERS);
    assert.equal(res.status, 200);
    assert.ok('processed' in res.body);
    assert.ok('errors' in res.body);
  });

  it('DELETE /memory/:id deletes a memory', async (t) => {
    if (!memoryAvailable) return t.skip(MEMORY_SKIP_REASON);
    // Save one first
    const saveRes = await request(port, 'POST', '/memory/save', {
      summary: 'Temporary memory to delete',
      channel: 'http',
      senderId: 'test-user',
    }, AUTH_HEADERS);
    const id = saveRes.body.id;
    const res = await request(port, 'DELETE', `/memory/${id}`, null, AUTH_HEADERS);
    assert.equal(res.status, 200);
    assert.equal(res.body.ok, true);
    assert.equal(res.body.deleted, id);
  });
});

// ============================================================================
// WebChat HTML response support
// ============================================================================

describe('v0.3.0 — WebChat HTML response', () => {
  let gw, port;

  before(async () => {
    const { createHttpGateway } = await import('../src/channels/http-gateway.js');
    const { getPluginRegistry } = await import('../src/channels/plugin-api.js');

    gw = createHttpGateway({ port: 0 });
    const addr = await gw.start();
    port = addr.port;

    // Register a mock webchat route that returns _html
    getPluginRegistry()._routes.push({
      method: 'GET',
      path: '/chat',
      level: 'none',
      handler: async () => ({
        status: 200,
        _html: '<html><body><h1>Chat</h1></body></html>',
      }),
    });
  });

  after(async () => {
    await gw.stop();
  });

  it('GET /chat returns HTML content-type', async () => {
    const res = await request(port, 'GET', '/chat');
    assert.equal(res.status, 200);
    assert.ok(res.headers['content-type'].includes('text/html'));
    assert.ok(res.body.includes('<h1>Chat</h1>'));
  });
});

// ============================================================================
// Orchestrator getStatus() includes subsystems
// ============================================================================

describe('v0.3.0 — Orchestrator status includes subsystems', () => {
  it('getStatus() returns voice, browser, memory, httpGateway fields', async () => {
    const { ChannelOrchestrator } = await import('../src/channels/orchestrator.js');
    const orch = new ChannelOrchestrator({
      channels: {},
      httpGateway: { enabled: false },
    });
    const status = orch.getStatus();
    assert.ok('voice' in status);
    assert.ok('browser' in status);
    assert.ok('memory' in status);
    assert.ok('httpGateway' in status);
    assert.equal(status.voice.enabled, false);
    assert.equal(status.browser.enabled, false);
    assert.equal(status.memory.enabled, false);
    assert.equal(status.httpGateway.enabled, false);
  });
});
