#!/usr/bin/env node
/**
 * StateSet Commerce MCP Server — Streamable HTTP (sandbox mode)
 *
 * The hosted-agent entrypoint: serve the full commerce tool surface over the
 * MCP Streamable HTTP transport, with an ISOLATED, demo-seeded commerce store
 * per session. An agent (Claude, Cursor, any MCP-over-HTTP client) points at
 * the URL and immediately has a working store it can freely read AND write —
 * every session gets its own ephemeral database, so nothing leaks between
 * agents and nothing persists beyond the session.
 *
 *   stateset-mcp-http --host 0.0.0.0 --port 8090
 *   →  POST/GET/DELETE http://host:8090/mcp   (MCP Streamable HTTP)
 *   →  GET  http://host:8090/health           (JSON status)
 *
 * Contrast with the siblings:
 *   - `stateset-mcp`         stdio, one shared database, writes need --apply
 *   - `stateset-mcp-events`  stdio + HTTP event-stream sidecar
 *
 * Because each session's store is ephemeral and isolated, writes are ENABLED
 * by default here (that is the sandbox's point); pass --read-only to serve a
 * look-but-don't-touch sandbox instead.
 *
 * There is deliberately no auth: this is a public-sandbox design. To host a
 * private instance, put it behind your proxy of choice; the default bind is
 * loopback so nothing is exposed unless you ask for it.
 */
import { createServer } from 'node:http';
import { randomUUID } from 'node:crypto';
import { parseArgs } from 'node:util';
import { runMain } from '../src/graceful-shutdown.js';
import { CLI_VERSION } from '../src/config.js';

const HELP = `
StateSet Commerce MCP Server — Streamable HTTP sandbox  v${CLI_VERSION}

USAGE:
  stateset-mcp-http [options]

OPTIONS:
  --host <host>          Bind host (default: 127.0.0.1; use 0.0.0.0 to expose)
  --port <port>          Bind port (default: 8090)
  --no-seed              Start sessions with an EMPTY store (default: seeded demo data)
  --read-only            Disable write tools (default: writes enabled, per-session isolation)
  --session-ttl <min>    Idle minutes before a session is evicted (default: 60)
  --max-sessions <n>     Concurrent session cap (default: 100)
  -h, --help             Show this help

CONNECT (Claude Desktop / any Streamable-HTTP MCP client):
  { "mcpServers": { "stateset-sandbox": { "url": "http://localhost:8090/mcp" } } }
`;

async function main() {
  const { values } = parseArgs({
    options: {
      host: { type: 'string' },
      port: { type: 'string' },
      'no-seed': { type: 'boolean', default: false },
      'read-only': { type: 'boolean', default: false },
      'session-ttl': { type: 'string' },
      'max-sessions': { type: 'string' },
      help: { type: 'boolean', short: 'h', default: false },
    },
  });

  if (values.help) {
    console.error(HELP);
    process.exit(0);
  }

  const [
    { Commerce },
    { StreamableHTTPServerTransport },
    { createStatesetMcpServer },
    { seedDemoData },
  ] = await Promise.all([
    import('@stateset/embedded'),
    import('@modelcontextprotocol/sdk/server/streamableHttp.js'),
    import('../src/mcp-server.js'),
    import('../src/seeds/demo.js'),
  ]);

  const host = values.host || '127.0.0.1';
  const port = Number.parseInt(values.port || '8090', 10);
  const seed = !values['no-seed'];
  const allowApply = !values['read-only'];
  const ttlMs = Math.max(1, Number.parseInt(values['session-ttl'] || '60', 10)) * 60_000;
  const maxSessions = Math.max(1, Number.parseInt(values['max-sessions'] || '100', 10));

  /** @type {Map<string, {transport: any, lastSeen: number}>} */
  const sessions = new Map();
  const startedAt = Date.now();

  async function createSession() {
    // Each session gets its own ephemeral store (`:memory:` is backed by a
    // private temp file with full WAL semantics; it is deleted when the pool
    // drops with the session).
    const commerce = new Commerce(':memory:');
    if (seed) {
      await seedDemoData(commerce, { quiet: true });
    }
    const mcpServer = createStatesetMcpServer({ commerce, dbPath: ':memory:', allowApply });
    const mcpInstance = mcpServer?.instance || mcpServer?.server || mcpServer;
    const transport = new StreamableHTTPServerTransport({
      sessionIdGenerator: () => randomUUID(),
      onsessioninitialized: (sessionId) => {
        sessions.set(sessionId, { transport, lastSeen: Date.now() });
        console.error(
          `[stateset-mcp-http] session ${sessionId.slice(0, 8)} started (${sessions.size} active)`,
        );
      },
      onsessionclosed: (sessionId) => {
        sessions.delete(sessionId);
        console.error(
          `[stateset-mcp-http] session ${sessionId.slice(0, 8)} closed (${sessions.size} active)`,
        );
      },
    });
    await mcpInstance.connect(transport);
    return transport;
  }

  function evictIdleSessions() {
    const cutoff = Date.now() - ttlMs;
    for (const [id, entry] of sessions) {
      if (entry.lastSeen < cutoff) {
        sessions.delete(id);
        entry.transport.close?.().catch?.(() => {});
        console.error(`[stateset-mcp-http] session ${id.slice(0, 8)} evicted after idle TTL`);
      }
    }
  }
  const sweeper = setInterval(evictIdleSessions, 60_000);
  sweeper.unref();

  function sendJson(res, status, body) {
    res.writeHead(status, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify(body));
  }

  const server = createServer((req, res) => {
    void (async () => {
      const url = new URL(req.url || '/', `http://${host}:${port}`);

      if (url.pathname === '/health') {
        sendJson(res, 200, {
          status: 'ok',
          version: CLI_VERSION,
          sessions: sessions.size,
          seeded: seed,
          writes: allowApply ? 'per-session-isolated' : 'read-only',
          uptimeSeconds: Math.floor((Date.now() - startedAt) / 1000),
        });
        return;
      }

      if (url.pathname !== '/mcp') {
        sendJson(res, 404, { error: 'not found', hint: 'MCP endpoint is /mcp; status is /health' });
        return;
      }

      const sessionId = req.headers['mcp-session-id'];
      const existing = typeof sessionId === 'string' ? sessions.get(sessionId) : undefined;

      if (existing) {
        existing.lastSeen = Date.now();
        await existing.transport.handleRequest(req, res);
        return;
      }

      // No (valid) session: only an initialize POST may open one.
      if (req.method !== 'POST') {
        sendJson(res, 400, {
          error: 'unknown or expired session',
          hint: 'send initialize to open a new one',
        });
        return;
      }
      if (sessions.size >= maxSessions) {
        sendJson(res, 503, { error: 'session capacity reached, try again shortly' });
        return;
      }
      const transport = await createSession();
      await transport.handleRequest(req, res);
    })().catch((error) => {
      console.error(`[stateset-mcp-http] request error: ${error.message}`);
      if (!res.headersSent) {
        sendJson(res, 500, { error: 'internal error' });
      } else {
        res.end();
      }
    });
  });

  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(port, host, () => resolve());
  });

  console.error(
    `[stateset-mcp-http] sandbox listening on http://${host}:${port}/mcp ` +
      `(seed: ${seed ? 'demo data' : 'empty'}, writes: ${allowApply ? 'per-session' : 'read-only'}, ` +
      `ttl: ${ttlMs / 60_000}m, max sessions: ${maxSessions})`,
  );
}

runMain('stateset-mcp-http', main);
