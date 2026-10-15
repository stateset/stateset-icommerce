/** Exercise bind-address handling against the real HTTP/native MCP server. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { request } from 'node:http';
import { createServer } from 'node:net';
import { fileURLToPath } from 'node:url';
import { startupBudgetMs } from '../helpers/startup-budget.js';

const BIN = fileURLToPath(new URL('../../bin/stateset-mcp-http.js', import.meta.url));
const ENVELOPE = {
  'io.modelcontextprotocol/protocolVersion': '2026-07-28',
  'io.modelcontextprotocol/clientInfo': { name: 'listen-test', version: '0' },
  'io.modelcontextprotocol/clientCapabilities': {},
};

function spawnServer(args) {
  return spawn(process.execPath, [BIN, ...args], {
    stdio: ['ignore', 'ignore', 'pipe'],
    env: { ...process.env, STATESET_MCP_API_KEYS: '' },
  });
}

async function stopServer(child) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  const closed = once(child, 'close');
  const timer = setTimeout(() => child.kill('SIGKILL'), 5000);
  timer.unref();
  child.kill('SIGTERM');
  try {
    await closed;
  } finally {
    clearTimeout(timer);
  }
}

function listeningEndpoint(child) {
  return new Promise((resolve, reject) => {
    let stderr = '';
    const timer = setTimeout(
      () => reject(new Error(`MCP startup timed out: ${stderr}`)),
      startupBudgetMs(30_000),
    );
    child.stderr.on('data', (chunk) => {
      stderr += chunk.toString();
      const match = stderr.match(/stateless MCP .* on (http:\/\/\S+)\/mcp /);
      if (match) {
        clearTimeout(timer);
        resolve(match[1]);
      }
    });
    child.once('error', (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.once('exit', (code, signal) => {
      clearTimeout(timer);
      reject(new Error(`MCP exited (${code ?? signal}): ${stderr}`));
    });
  });
}

function malformedRequest(base) {
  const url = new URL(base);
  return new Promise((resolve, reject) => {
    const req = request(
      {
        hostname: url.hostname.replace(/^\[|\]$/g, ''),
        port: url.port,
        path: 'http://[',
        method: 'POST',
      },
      (res) => {
        let body = '';
        res.setEncoding('utf8');
        res.on('data', (chunk) => {
          body += chunk;
        });
        res.once('end', () => {
          try {
            resolve({ status: res.statusCode, body: JSON.parse(body) });
          } catch (error) {
            reject(error);
          }
        });
        res.once('error', reject);
      },
    );
    req.once('error', reject);
    req.setTimeout(5000, () => req.destroy(new Error('malformed-target request timed out')));
    req.end();
  });
}

for (const host of ['127.0.0.1', '::1']) {
  it(
    `serves MCP on ${host} and advertises the actual port selected by --port 0`,
    { timeout: startupBudgetMs(60_000) },
    async (t) => {
      if (host === '::1') {
        const probe = createServer();
        try {
          probe.listen(0, host);
          await once(probe, 'listening');
        } catch (error) {
          if (!['EAFNOSUPPORT', 'EADDRNOTAVAIL', 'EPROTONOSUPPORT'].includes(error.code))
            throw error;
          t.skip(`IPv6 loopback unavailable: ${error.code}`);
          return;
        } finally {
          if (probe.listening) await new Promise((resolve) => probe.close(resolve));
        }
      }
      const child = spawnServer(['--host', host, '--port', '0', '--no-seed']);
      t.after(() => stopServer(child));
      const base = await listeningEndpoint(child);
      assert.ok(Number(new URL(base).port) > 0, 'advertise the assigned port, not zero');
      assert.equal(new URL(base).hostname, host === '::1' ? '[::1]' : host);
      const health = await fetch(`${base}/health?probe=1`);
      assert.equal(health.status, 200);
      assert.equal((await health.json()).stateless, true);
      const tools = await fetch(`${base}/mcp`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Accept: 'application/json, text/event-stream',
          'Mcp-Method': 'tools/list',
        },
        body: JSON.stringify({
          jsonrpc: '2.0',
          id: 1,
          method: 'tools/list',
          params: { _meta: ENVELOPE },
        }),
      });
      assert.equal(tools.status, 200);
      const text = await tools.text();
      const events = text.split('\n').filter((line) => line.startsWith('data:'));
      const result = JSON.parse(events.length ? events.at(-1).slice(5).trim() : text);
      assert.ok(result.result.tools.some((tool) => tool.name === 'list_customers'));
      assert.equal(tools.headers.get('mcp-session-id'), null);
      assert.deepEqual(await malformedRequest(base), {
        status: 400,
        body: { error: 'invalid request target' },
      });
      assert.equal((await fetch(`${base}/health`)).status, 200);
    },
  );
}

for (const port of ['8090garbage', '1.5', '65536']) {
  it(
    `refuses invalid --port ${port} before starting a listener`,
    { timeout: startupBudgetMs(20_000) },
    async (t) => {
      const child = spawnServer(['--port', port]);
      t.after(() => stopServer(child));
      let stderr = '';
      child.stderr.on('data', (chunk) => {
        stderr += chunk.toString();
      });
      const [code] = await once(child, 'close');
      assert.notEqual(code, 0);
      assert.match(stderr, /--port.*integer.*0.*65535/);
      assert.doesNotMatch(stderr, /stateless MCP .* on /);
    },
  );
}
