/**
 * Integration test for the Streamable-HTTP MCP sandbox (stateset-mcp-http).
 *
 * Boots the real server on an ephemeral port and verifies the properties the
 * sandbox exists for:
 *   - health endpoint reports sanely
 *   - a session initializes over Streamable HTTP and gets seeded demo data
 *   - writes succeed inside a session
 *   - sessions are ISOLATED: another session never sees the first one's writes
 */

import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const BIN = path.resolve(__dirname, '../../bin/stateset-mcp-http.js');
const PORT = 18091 + (process.pid % 500);
const BASE = `http://127.0.0.1:${PORT}`;

let child;

async function waitForHealth(timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`${BASE}/health`);
      if (res.ok) return res.json();
    } catch {
      // not up yet
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error('sandbox did not become healthy in time');
}

async function rpc(payload, session) {
  const headers = {
    'Content-Type': 'application/json',
    Accept: 'application/json, text/event-stream',
  };
  if (session) headers['mcp-session-id'] = session;
  const res = await fetch(`${BASE}/mcp`, {
    method: 'POST',
    headers,
    body: JSON.stringify(payload),
  });
  const sid = res.headers.get('mcp-session-id');
  let body = await res.text();
  if (body.includes('data:')) {
    const datas = body.split('\n').filter((l) => l.startsWith('data:'));
    body = datas.length > 0 ? datas[datas.length - 1].slice(5).trim() : '{}';
  }
  return { json: body.trim() ? JSON.parse(body) : {}, sid, status: res.status };
}

const INIT = {
  jsonrpc: '2.0',
  id: 1,
  method: 'initialize',
  params: {
    protocolVersion: '2024-11-05',
    capabilities: {},
    clientInfo: { name: 'sandbox-test', version: '0' },
  },
};

function toolText(result) {
  return JSON.parse(result.json.result.content[0].text);
}

async function openSession() {
  const init = await rpc(INIT);
  assert.equal(init.json.result.serverInfo.name, 'stateset-commerce');
  assert.ok(init.sid, 'initialize response must carry a session id');
  await rpc({ jsonrpc: '2.0', method: 'notifications/initialized' }, init.sid);
  return init.sid;
}

describe('mcp http sandbox', () => {
  before(async () => {
    child = spawn(process.execPath, [BIN, '--port', String(PORT)], {
      stdio: ['ignore', 'ignore', 'pipe'],
    });
    await waitForHealth();
  });

  after(() => {
    child?.kill('SIGTERM');
  });

  it('reports health', async () => {
    const health = await waitForHealth();
    assert.equal(health.status, 'ok');
    assert.equal(health.seeded, true);
    assert.equal(health.writes, 'per-session-isolated');
  });

  it('seeds demo data and isolates writes between sessions', async () => {
    const sidA = await openSession();

    const seededA = toolText(
      await rpc(
        {
          jsonrpc: '2.0',
          id: 2,
          method: 'tools/call',
          params: { name: 'list_customers', arguments: {} },
        },
        sidA,
      ),
    );
    assert.ok(seededA.count >= 5, `session A should see seeded customers, got ${seededA.count}`);

    const marker = `iso-${Date.now()}@sandbox.test`;
    const created = toolText(
      await rpc(
        {
          jsonrpc: '2.0',
          id: 3,
          method: 'tools/call',
          params: {
            name: 'create_customer',
            arguments: { email: marker, firstName: 'Iso', lastName: 'Lated' },
          },
        },
        sidA,
      ),
    );
    assert.equal(created.success, true, 'sandbox writes must succeed inside a session');

    const sidB = await openSession();
    assert.notEqual(sidA, sidB);
    const seenByB = toolText(
      await rpc(
        {
          jsonrpc: '2.0',
          id: 4,
          method: 'tools/call',
          params: { name: 'list_customers', arguments: {} },
        },
        sidB,
      ),
    );
    const emails = (seenByB.customers || []).map((c) => c.email);
    assert.ok(!emails.includes(marker), 'session B must NOT see session A writes');
  });

  it('rejects non-initialize traffic without a session', async () => {
    const res = await fetch(`${BASE}/mcp`, { method: 'GET' });
    assert.equal(res.status, 400);
  });
});
