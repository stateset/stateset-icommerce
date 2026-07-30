#!/usr/bin/env node
/**
 * StateSet Commerce MCP Server — Streamable HTTP, protocol revision 2026-07-28
 *
 * The hosted-agent entrypoint. It is STATELESS by construction: `createMcpHandler`
 * serves the 2026-07-28 revision from a per-request server factory, so every
 * exchange gets a freshly built MCP server, no `Mcp-Session-Id` is issued, and
 * nothing is retained between requests. Any request can land on any replica.
 *
 *   stateset-mcp-http --host 0.0.0.0 --port 8090
 *   →  POST http://host:8090/mcp     (MCP Streamable HTTP)
 *   →  GET  http://host:8090/health  (JSON status)
 *
 * 2025-era clients (those that predate the `_meta` envelope) are still served,
 * on the SDK's stateless legacy leg, from the SAME tool factory — the two eras
 * cannot drift apart. Pass `--strict-protocol` for a modern-only endpoint that
 * rejects them. Because serving is per-request in both eras, the 2025 session
 * operations `GET` and `DELETE` answer `405`.
 *
 * The protocol layer holds no state, so the commerce store is the only state,
 * and it is shared by every request: `--db` (default `:memory:`, a private
 * temp-file-backed store seeded with demo data once at boot). Point `--db` at a
 * real file to serve a durable store; writes are then disabled unless you pass
 * `--apply`, since this server has no auth of its own.
 *
 * Contrast with the siblings:
 *   - `stateset-mcp`         stdio, one shared database, writes need --apply
 *   - `stateset-mcp-events`  stdio + HTTP event-stream sidecar
 */
import { createServer } from 'node:http';
import { parseArgs } from 'node:util';
import { runMain } from '../src/graceful-shutdown.js';
import { CLI_VERSION } from '../src/config.js';

const HELP = `
StateSet Commerce MCP Server — Streamable HTTP (protocol 2026-07-28)  v${CLI_VERSION}

USAGE:
  stateset-mcp-http [options]

OPTIONS:
  --host <host>          Bind host (default: 127.0.0.1; use 0.0.0.0 to expose)
  --port <port>          Bind port (default: 8090)
  --db <path>            Shared store path (default: :memory:, seeded at boot)
  --apply                Enable write tools against a durable --db
  --read-only            Disable write tools (default for a durable --db)
  --no-seed              Start with an EMPTY store (default: seeded demo data)
  --strict-protocol      Serve ONLY 2026-07-28; reject 2025-era clients
  --allowed-host <host>  Allowed Host header (repeatable); enables DNS-rebinding
                         protection. Default: localhost-only on a loopback bind.
  -h, --help             Show this help

CONNECT (Claude Desktop / any Streamable-HTTP MCP client):
  { "mcpServers": { "stateset": { "url": "http://localhost:8090/mcp" } } }
`;

async function main() {
  const { values } = parseArgs({
    options: {
      host: { type: 'string' },
      port: { type: 'string' },
      db: { type: 'string' },
      apply: { type: 'boolean', default: false },
      'read-only': { type: 'boolean', default: false },
      'no-seed': { type: 'boolean', default: false },
      'strict-protocol': { type: 'boolean', default: false },
      'allowed-host': { type: 'string', multiple: true },
      help: { type: 'boolean', short: 'h', default: false },
    },
  });

  if (values.help) {
    console.error(HELP);
    process.exit(0);
  }

  const [
    { Commerce },
    { createMcpHandler },
    { toNodeHandler, hostHeaderValidation, localhostHostValidation },
    { createStatesetMcpServer },
    { createStatesetV2McpServer },
    { seedDemoData },
  ] = await Promise.all([
    import('@stateset/embedded'),
    import('@modelcontextprotocol/server'),
    import('@modelcontextprotocol/node'),
    import('../src/mcp-server.js'),
    import('../src/mcp/v2-server.js'),
    import('../src/seeds/demo.js'),
  ]);

  const host = values.host || '127.0.0.1';
  const port = Number.parseInt(values.port || '8090', 10);
  const dbPath = values.db || ':memory:';
  const ephemeralDb = dbPath === ':memory:';
  const seed = !values['no-seed'];

  // Writes are safe by default only when the store is ephemeral. A durable --db
  // needs explicit --apply, because this server ships no authentication.
  const allowApply = values['read-only'] ? false : ephemeralDb || values.apply;

  // DNS-rebinding protection: a browser on a victim's machine must not reach a
  // loopback MCP server by resolving an attacker domain to 127.0.0.1. Only
  // meaningful for a loopback bind — an exposed server usually sits behind a
  // proxy whose Host header we cannot guess, so there we require an explicit
  // --allowed-host rather than rejecting every real request.
  const explicitHosts = values['allowed-host'] ?? [];
  const loopback = host === '127.0.0.1' || host === 'localhost' || host === '::1';
  const validateHost =
    explicitHosts.length > 0
      ? hostHeaderValidation(explicitHosts)
      : loopback
        ? localhostHostValidation()
        : null;

  // The store is the only state, shared by every request.
  const commerce = new Commerce(dbPath);
  if (seed && ephemeralDb) {
    await seedDemoData(commerce, { quiet: true });
  }

  // Per-request server factory: this is what makes the endpoint stateless. The
  // same factory backs both the 2026-07-28 path and the legacy stateless leg.
  const handler = createMcpHandler(
    () =>
      createStatesetV2McpServer({
        commerce,
        dbPath,
        allowApply,
        createServer: createStatesetMcpServer,
      }),
    {
      legacy: values['strict-protocol'] ? 'reject' : 'stateless',
      onerror: (error) => console.error(`[stateset-mcp-http] ${error.message}`),
    },
  );
  const mcpHandler = toNodeHandler(handler);

  // Build one server at boot so the first real request does not pay for the
  // ~940 one-time schema conversions cached inside the factory.
  createStatesetV2McpServer({
    commerce,
    dbPath,
    allowApply,
    createServer: createStatesetMcpServer,
  });

  const startedAt = Date.now();

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
          protocol: '2026-07-28',
          legacy: values['strict-protocol'] ? 'reject' : 'stateless',
          stateless: true,
          db: dbPath,
          seeded: seed && ephemeralDb,
          writes: allowApply ? 'shared-store' : 'read-only',
          uptimeSeconds: Math.floor((Date.now() - startedAt) / 1000),
        });
        return;
      }

      if (url.pathname !== '/mcp') {
        sendJson(res, 404, { error: 'not found', hint: 'MCP endpoint is /mcp; status is /health' });
        return;
      }

      // Guards run in front of the handler: the SDK entry is deliberately
      // validation-free. A guard that rejects has already written the response.
      if (validateHost && !validateHost(req, res)) return;

      await mcpHandler(req, res);
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
    `[stateset-mcp-http] stateless MCP (protocol 2026-07-28) on http://${host}:${port}/mcp ` +
      `(db: ${dbPath}, seed: ${seed && ephemeralDb ? 'demo data' : 'none'}, ` +
      `writes: ${allowApply ? 'enabled' : 'read-only'}, ` +
      `legacy 2025 clients: ${values['strict-protocol'] ? 'rejected' : 'served statelessly'})`,
  );
}

runMain('stateset-mcp-http', main);
