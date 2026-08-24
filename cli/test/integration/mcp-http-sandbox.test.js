/**
 * Integration test for the Streamable-HTTP MCP server (stateset-mcp-http).
 *
 * Boots the real server and verifies the two properties it is built for:
 *
 *   protocol revision 2026-07-28 — requests carry the `_meta` envelope and the
 *     `Mcp-Method` / `Mcp-Name` headers; no initialize handshake exists.
 *
 *   statelessness — no `Mcp-Session-Id` is ever issued, a bare request needs no
 *     prior handshake, the 2025 session verbs GET/DELETE are refused, and every
 *     request sees the one shared store.
 *
 * 2025-era clients are served on the SDK's stateless legacy leg unless
 * `--strict-protocol` is passed, which is covered too.
 */

import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const BIN = path.resolve(__dirname, '../../bin/stateset-mcp-http.js');
const DEFAULT_PORT = 18091 + (process.pid % 400);
const STRICT_PORT = DEFAULT_PORT + 400;

/** The per-request envelope every 2026-07-28 message carries. */
const ENVELOPE = {
  'io.modelcontextprotocol/protocolVersion': '2026-07-28',
  'io.modelcontextprotocol/clientInfo': { name: 'stateless-test', version: '0' },
  'io.modelcontextprotocol/clientCapabilities': {},
};

async function waitForHealth(base, timeoutMs = 60_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`${base}/health`);
      if (res.ok) return res.json();
    } catch {
      // not up yet
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error('server did not become healthy in time');
}

function parseBody(text) {
  // A modern exchange may answer as JSON or upgrade to SSE; accept both.
  if (text.includes('data:')) {
    const datas = text.split('\n').filter((l) => l.startsWith('data:'));
    return datas.length > 0 ? JSON.parse(datas[datas.length - 1].slice(5).trim()) : {};
  }
  return text.trim() ? JSON.parse(text) : {};
}

/** Send a 2026-07-28 request: envelope in params, method/name in headers. */
async function modern(base, { id = 1, method, params = {} }) {
  const headers = {
    'Content-Type': 'application/json',
    Accept: 'application/json, text/event-stream',
    'Mcp-Method': method,
  };
  if (params.name) headers['Mcp-Name'] = params.name;

  const res = await fetch(`${base}/mcp`, {
    method: 'POST',
    headers,
    body: JSON.stringify({
      jsonrpc: '2.0',
      id,
      method,
      params: { ...params, _meta: ENVELOPE },
    }),
  });
  return {
    json: parseBody(await res.text()),
    sid: res.headers.get('mcp-session-id'),
    status: res.status,
  };
}

/** Send a 2025-era initialize, the way an older client would. */
async function legacyInitialize(base) {
  const res = await fetch(`${base}/mcp`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/json, text/event-stream',
    },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 1,
      method: 'initialize',
      params: {
        protocolVersion: '2025-06-18',
        capabilities: {},
        clientInfo: { name: 'legacy-test', version: '0' },
      },
    }),
  });
  return {
    json: parseBody(await res.text()),
    sid: res.headers.get('mcp-session-id'),
    status: res.status,
  };
}

const toolText = (result) => JSON.parse(result.json.result.content[0].text);

const callTool = (base, name, args = {}, id = 10) =>
  modern(base, { id, method: 'tools/call', params: { name, arguments: args } });

function spawnServer(args) {
  return spawn(process.execPath, [BIN, ...args], { stdio: ['ignore', 'ignore', 'pipe'] });
}

describe('mcp http — protocol 2026-07-28, stateless', () => {
  const BASE = `http://127.0.0.1:${DEFAULT_PORT}`;
  let child;

  before(async () => {
    child = spawnServer(['--port', String(DEFAULT_PORT)]);
    await waitForHealth(BASE);
  });

  after(() => {
    child?.kill('SIGTERM');
  });

  it('reports the served protocol revision', async () => {
    const health = await waitForHealth(BASE);
    assert.equal(health.status, 'ok');
    assert.equal(health.protocol, '2026-07-28');
    assert.equal(health.stateless, true);
    assert.equal(health.legacy, 'stateless');
  });

  it('lists tools over the 2026-07-28 envelope', async () => {
    const res = await modern(BASE, { method: 'tools/list' });
    const tools = res.json.result.tools;
    assert.ok(tools.length > 500, `expected the full tool surface, got ${tools.length}`);
    const listCustomers = tools.find((t) => t.name === 'list_customers');
    assert.ok(listCustomers, 'list_customers must be advertised');
    assert.equal(listCustomers.inputSchema.type, 'object', 'schemas must render as JSON Schema');
  });

  it('never issues a session id', async () => {
    const res = await modern(BASE, { method: 'tools/list' });
    assert.equal(res.sid, null, 'a stateless server must not return Mcp-Session-Id');
  });

  it('serves a tool call with no prior handshake', async () => {
    const seeded = toolText(await callTool(BASE, 'list_customers'));
    assert.ok(seeded.count >= 5, `expected seeded customers, got ${seeded.count}`);
  });

  it('shares one store across requests', async () => {
    const marker = `shared-${Date.now()}@stateless.test`;
    const created = toolText(
      await callTool(BASE, 'create_customer', {
        email: marker,
        firstName: 'Share',
        lastName: 'Ed',
      }),
    );
    assert.equal(created.success, true, 'writes against the shared ephemeral store must succeed');

    const listed = toolText(await callTool(BASE, 'list_customers', {}, 11));
    const emails = (listed.customers || []).map((c) => c.email);
    assert.ok(emails.includes(marker), 'a later request must see the earlier write');
  });

  it('refuses the 2025 session verbs GET and DELETE', async () => {
    for (const method of ['GET', 'DELETE']) {
      const res = await fetch(`${BASE}/mcp`, { method });
      assert.equal(res.status, 405, `${method} must be rejected on a stateless endpoint`);
    }
  });

  it('still serves 2025-era clients on the legacy stateless leg', async () => {
    const init = await legacyInitialize(BASE);
    assert.equal(init.json.result.serverInfo.name, 'stateset-commerce');
    assert.equal(init.json.result.protocolVersion, '2025-06-18');
    assert.equal(init.sid, null, 'the legacy leg is stateless too — no session id');
  });
});

describe('mcp http — --strict-protocol', () => {
  const BASE = `http://127.0.0.1:${STRICT_PORT}`;
  let child;

  before(async () => {
    child = spawnServer(['--port', String(STRICT_PORT), '--strict-protocol']);
    await waitForHealth(BASE);
  });

  after(() => {
    child?.kill('SIGTERM');
  });

  it('reports legacy rejection', async () => {
    const health = await waitForHealth(BASE);
    assert.equal(health.legacy, 'reject');
  });

  it('still serves 2026-07-28 traffic', async () => {
    const res = await modern(BASE, { method: 'tools/list' });
    assert.ok(res.json.result.tools.length > 500);
  });

  it('rejects a 2025-era client', async () => {
    const init = await legacyInitialize(BASE);
    assert.ok(init.json.error, 'a legacy initialize must be refused in strict mode');
  });
});

// ---------------------------------------------------------------------------
// Browser-origin and bind-safety guards
// ---------------------------------------------------------------------------

const ORIGIN_PORT = DEFAULT_PORT + 800;
const ANY_HOST_PORT = DEFAULT_PORT + 1200;

/** Collect stderr and the exit code of a server that is expected to die. */
function spawnAndCollect(args) {
  return new Promise((resolve) => {
    const child = spawnServer(args);
    let stderr = '';
    child.stderr.on('data', (chunk) => {
      stderr += chunk.toString();
    });
    const killer = setTimeout(() => child.kill('SIGKILL'), 20_000);
    child.once('close', (code) => {
      clearTimeout(killer);
      resolve({ code, stderr });
    });
  });
}

const listWithOrigin = (base, origin) =>
  fetch(`${base}/mcp`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/json, text/event-stream',
      'Mcp-Method': 'tools/list',
      ...(origin ? { Origin: origin } : {}),
    },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 1,
      method: 'tools/list',
      params: { _meta: ENVELOPE },
    }),
  });

describe('mcp http — Origin validation', () => {
  const BASE = `http://127.0.0.1:${ORIGIN_PORT}`;
  let child;

  before(async () => {
    child = spawnServer([
      '--port',
      String(ORIGIN_PORT),
      '--allowed-origin',
      'https://agent.example.com',
      '--allowed-origin',
      'http://studio.example.net:3000',
    ]);
    await waitForHealth(BASE);
  });

  after(() => {
    child?.kill('SIGTERM');
  });

  it('rejects a browser request from an unlisted Origin with 403', async () => {
    const res = await listWithOrigin(BASE, 'https://evil.example.org');
    assert.equal(res.status, 403);
    const body = await res.json();
    assert.match(body.error.message, /Invalid Origin/);
  });

  it('rejects an unparseable Origin with 403', async () => {
    const res = await listWithOrigin(BASE, 'not a url');
    assert.equal(res.status, 403);
  });

  it('serves a request from an allowed Origin', async () => {
    const res = await listWithOrigin(BASE, 'https://agent.example.com');
    assert.equal(res.status, 200);
    assert.ok(parseBody(await res.text()).result.tools.length > 500);
  });

  it('matches allowed origins by hostname, ignoring scheme and port', async () => {
    const res = await listWithOrigin(BASE, 'https://studio.example.net');
    assert.equal(res.status, 200);
  });

  it('serves a request with no Origin header (non-browser client)', async () => {
    const res = await listWithOrigin(BASE, null);
    assert.equal(res.status, 200);
    assert.ok(parseBody(await res.text()).result.tools.length > 500);
  });
});

describe('mcp http — default Origin policy on a loopback bind', () => {
  const BASE = `http://127.0.0.1:${DEFAULT_PORT}`;
  let child;

  before(async () => {
    child = spawnServer(['--port', String(DEFAULT_PORT)]);
    await waitForHealth(BASE);
  });

  after(() => {
    child?.kill('SIGTERM');
  });

  it('rejects a cross-origin browser request when no --allowed-origin is set', async () => {
    const res = await listWithOrigin(BASE, 'https://evil.example.org');
    assert.equal(res.status, 403);
  });

  it('accepts a localhost Origin', async () => {
    const res = await listWithOrigin(BASE, 'http://localhost:5173');
    assert.equal(res.status, 200);
  });
});

describe('mcp http — non-loopback bind fails closed', () => {
  it('exits non-zero without --allowed-host and names the fix', async () => {
    const { code, stderr } = await spawnAndCollect([
      '--host',
      '0.0.0.0',
      '--port',
      String(ANY_HOST_PORT),
    ]);
    assert.notEqual(code, 0, 'a non-loopback bind with no Host allowlist must not start');
    assert.match(stderr, /--allowed-host/);
    assert.match(stderr, /--insecure-allow-any-host/);
  });

  it('starts with --insecure-allow-any-host', async () => {
    const child = spawnServer([
      '--host',
      '0.0.0.0',
      '--port',
      String(ANY_HOST_PORT),
      '--insecure-allow-any-host',
    ]);
    try {
      const health = await waitForHealth(`http://127.0.0.1:${ANY_HOST_PORT}`);
      assert.equal(health.status, 'ok');
    } finally {
      child.kill('SIGTERM');
    }
  });
});
