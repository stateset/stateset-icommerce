/**
 * Integration test for the stdio MCP entrypoint (stateset-mcp).
 *
 * Boots the real binary and drives it over stdio in both protocol eras:
 *
 *   2026-07-28 — no handshake; each request carries its own `_meta` envelope.
 *   2025-era   — the classic `initialize` exchange, served from the SAME tool
 *                factory so the two eras expose an identical surface.
 *
 * `serveStdio` pins one server instance per connection based on the opening
 * exchange, so each case gets its own child process.
 */

import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { ALL_DOMAIN_TOOLS } from '../../src/tools/domain-registry.js';
import { AGENTIC_RUNTIME_TOOLS } from '../../src/mcp/agentic-runtime-tools.js';
import { toolTier } from '../../src/tools/tool-tiers.js';
import { startupBudgetMs } from '../helpers/startup-budget.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const BIN = path.resolve(__dirname, '../../bin/stateset-mcp.js');

const EVERY_TOOL = [...ALL_DOMAIN_TOOLS, ...AGENTIC_RUNTIME_TOOLS];
/** What a server started with no --profile must expose: the core tier, exactly. */
const CORE_TIER = EVERY_TOOL.filter((tool) => toolTier(tool.name) === 'core')
  .map((tool) => tool.name)
  .sort();

/** The per-request envelope every 2026-07-28 message carries. */
const ENVELOPE = {
  'io.modelcontextprotocol/protocolVersion': '2026-07-28',
  'io.modelcontextprotocol/clientInfo': { name: 'stdio-test', version: '0' },
  'io.modelcontextprotocol/clientCapabilities': {},
};

/**
 * A line-delimited JSON-RPC client over a child process's stdio, with a
 * per-id waiter so tests never race the server's startup cost.
 */
function createClient(args = []) {
  const child = spawn(process.execPath, [BIN, ...args], { stdio: ['pipe', 'pipe', 'pipe'] });
  const pending = new Map();
  let buffer = '';
  let stderr = '';

  child.stderr.on('data', (chunk) => {
    stderr += chunk;
  });

  child.stdout.on('data', (chunk) => {
    buffer += chunk;
    let newline;
    while ((newline = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, newline).trim();
      buffer = buffer.slice(newline + 1);
      if (!line) continue;
      const message = JSON.parse(line);
      const waiter = pending.get(message.id);
      if (waiter) {
        pending.delete(message.id);
        clearTimeout(waiter.timer);
        waiter.resolve(message);
      }
    }
  });

  const rejectPending = (reason) => {
    for (const { reject, timer } of pending.values()) {
      clearTimeout(timer);
      reject(reason);
    }
    pending.clear();
  };

  child.on('error', rejectPending);
  child.on('exit', (code, signal) => {
    rejectPending(
      new Error(
        `stateset-mcp exited before responding (code=${code}, signal=${signal})${stderr ? `: ${stderr.trim()}` : ''}`,
      ),
    );
  });

  // The first request on a fresh child also absorbs the server's boot cost.
  const request = (message, timeoutMs = startupBudgetMs(30_000)) =>
    new Promise((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error(`timed out waiting for id ${message.id} after ${timeoutMs}ms`)),
        timeoutMs,
      );
      pending.set(message.id, { resolve, reject, timer });
      child.stdin.write(`${JSON.stringify(message)}\n`);
    });

  return {
    request,
    notify: (message) => child.stdin.write(`${JSON.stringify(message)}\n`),
    close: () => child.kill('SIGTERM'),
  };
}

const modern = (client, id, method, params = {}) =>
  client.request({ jsonrpc: '2.0', id, method, params: { ...params, _meta: ENVELOPE } });

describe('mcp stdio — protocol 2026-07-28', () => {
  let client;

  before(() => {
    client = createClient(['--db', ':memory:']);
  });

  after(() => client?.close());

  it('lists tools with no handshake -- by default, exactly the core tier', async () => {
    const res = await modern(client, 1, 'tools/list');
    assert.ok(!res.error, `tools/list failed: ${res.error?.message}`);
    assert.deepEqual(res.result.tools.map((tool) => tool.name).sort(), CORE_TIER);
  });

  it('calls a tool', async () => {
    const res = await modern(client, 2, 'tools/call', { name: 'list_customers', arguments: {} });
    assert.ok(!res.error, `tools/call failed: ${res.error?.message}`);
    assert.equal(JSON.parse(res.result.content[0].text).success, true);
  });
});

describe('mcp stdio — 2025-era clients', () => {
  let client;

  before(() => {
    client = createClient(['--db', ':memory:']);
  });

  after(() => client?.close());

  it('completes the classic initialize handshake', async () => {
    const res = await client.request({
      jsonrpc: '2.0',
      id: 1,
      method: 'initialize',
      params: {
        protocolVersion: '2025-06-18',
        capabilities: {},
        clientInfo: { name: 'legacy-test', version: '0' },
      },
    });
    assert.equal(res.result.protocolVersion, '2025-06-18');
    assert.equal(res.result.serverInfo.name, 'stateset-commerce');
  });

  it('serves the same tool surface as the modern era', async () => {
    client.notify({ jsonrpc: '2.0', method: 'notifications/initialized' });
    const res = await client.request({ jsonrpc: '2.0', id: 2, method: 'tools/list', params: {} });
    assert.deepEqual(res.result.tools.map((tool) => tool.name).sort(), CORE_TIER);
  });
});

describe('mcp stdio — --profile all', () => {
  let client;

  before(() => {
    client = createClient(['--db', ':memory:', '--profile', 'all']);
  });

  after(() => client?.close());

  it('exposes every tier', { timeout: 150_000 }, async () => {
    // Rendering ~950 schemas is the slowest list there is; allow for a loaded host.
    const res = await client.request(
      { jsonrpc: '2.0', id: 1, method: 'tools/list', params: { _meta: ENVELOPE } },
      120_000,
    );
    assert.ok(!res.error, `tools/list failed: ${res.error?.message}`);
    assert.equal(res.result.tools.length, EVERY_TOOL.length);
  });
});

describe('mcp stdio — --strict-protocol', () => {
  let client;

  before(() => {
    client = createClient(['--db', ':memory:', '--strict-protocol']);
  });

  after(() => client?.close());

  it('rejects a 2025-era opening', async () => {
    const res = await client.request({
      jsonrpc: '2.0',
      id: 1,
      method: 'initialize',
      params: {
        protocolVersion: '2025-06-18',
        capabilities: {},
        clientInfo: { name: 'legacy-test', version: '0' },
      },
    });
    assert.ok(res.error, 'a legacy initialize must be refused in strict mode');
  });

  it('still serves 2026-07-28 on the same connection', async () => {
    const res = await modern(client, 2, 'tools/list');
    assert.ok(
      !res.error,
      `modern traffic must survive a rejected legacy opening: ${res.error?.message}`,
    );
    assert.deepEqual(res.result.tools.map((tool) => tool.name).sort(), CORE_TIER);
  });
});
