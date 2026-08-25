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
 * `--apply`.
 *
 * Three request guards run in front of the SDK handler, which is deliberately
 * validation-free, in the order Host → Origin → Auth:
 *   - Host (DNS rebinding): a loopback bind accepts only localhost Host values;
 *     any other bind REQUIRES `--allowed-host` (repeatable) and refuses to start
 *     without it, unless `--insecure-allow-any-host` is passed explicitly.
 *   - Origin (browser cross-origin): a request that carries an `Origin` header is
 *     rejected with 403 unless its hostname is allowed. A loopback bind allows
 *     localhost origins; anything else must be listed via `--allowed-origin`
 *     (repeatable). Requests with no `Origin` — every non-browser MCP client —
 *     always pass.
 *   - Auth (API key): when any key is configured (`--api-key`, repeatable;
 *     `STATESET_MCP_API_KEYS`, comma-separated; `--api-key-file`), every `/mcp`
 *     request must present `Authorization: Bearer <key>` or `X-API-Key: <key>`
 *     or it gets 401 with a JSON-RPC error body and a `WWW-Authenticate`
 *     challenge. Keys are compared in constant time and never logged (only a
 *     6-char sha256 fingerprint). A non-loopback bind REQUIRES a key and
 *     refuses to start without one, unless `--insecure-no-auth` is passed
 *     explicitly. On a loopback bind auth is optional (off by default).
 *     `/health` stays open but reports only `status`/`version`/`protocol`
 *     to unauthenticated callers once auth is on.
 *
 * Contrast with the siblings:
 *   - `stateset-mcp`         stdio, one shared database, writes need --apply
 *   - `stateset-mcp-events`  stdio + HTTP event-stream sidecar
 */
import { createServer } from 'node:http';
import { parseArgs } from 'node:util';
import { runMain } from '../src/graceful-shutdown.js';
import { CLI_VERSION } from '../src/config.js';
import {
  API_KEYS_ENV,
  collectApiKeys,
  createApiKeyGuard,
  keyFingerprint,
} from '../src/mcp/http-api-keys.js';

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
  --allowed-host <host>  Allowed Host header hostname (repeatable); DNS-rebinding
                         protection. Default: localhost-only on a loopback bind.
                         REQUIRED for any non-loopback --host.
  --insecure-allow-any-host
                         Skip Host validation on a non-loopback bind. Only for
                         a server that sits behind a proxy which already pins
                         the Host header. Never expose such a server directly.
  --allowed-origin <origin>
                         Allowed browser Origin (repeatable; a full origin such
                         as https://agent.example.com or a bare hostname —
                         matched by hostname, port-agnostic). Requests carrying
                         any other Origin get 403. Default: localhost origins
                         on a loopback bind, none otherwise. Requests with no
                         Origin header (non-browser clients) always pass.
  --api-key <key>        API key clients must present (repeatable). Also read
                         from STATESET_MCP_API_KEYS (comma-separated). Clients
                         send "Authorization: Bearer <key>" or "X-API-Key".
                         REQUIRED for any non-loopback --host; optional on
                         loopback. Keys must be at least 16 characters.
  --api-key-file <path>  File of API keys, one per line (# comments allowed).
  --insecure-no-auth     Serve a non-loopback bind with NO authentication. Only
                         behind a proxy that authenticates every request itself.
  -h, --help             Show this help

CONNECT (Claude Desktop / any Streamable-HTTP MCP client):
  { "mcpServers": { "stateset": { "url": "http://localhost:8090/mcp" } } }
  With a key:
  { "mcpServers": { "stateset": { "url": "https://mcp.example.com/mcp",
      "headers": { "Authorization": "Bearer <key>" } } } }
`;

/**
 * Reduce an `--allowed-origin` value to the hostname the SDK guard compares on.
 * Accepts a full origin (`https://agent.example.com:8443`) or a bare hostname.
 * @param {string} value
 * @returns {string}
 */
function originHostname(value) {
  const trimmed = value.trim();
  if (!trimmed) {
    throw new Error('--allowed-origin must not be empty');
  }
  if (!trimmed.includes('://')) return trimmed;
  try {
    const { hostname } = new URL(trimmed);
    if (!hostname) throw new Error('no hostname');
    return hostname;
  } catch {
    throw new Error(`--allowed-origin "${value}" is not a valid origin URL or hostname`);
  }
}

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
      'allowed-origin': { type: 'string', multiple: true },
      'insecure-allow-any-host': { type: 'boolean', default: false },
      'api-key': { type: 'string', multiple: true },
      'api-key-file': { type: 'string' },
      'insecure-no-auth': { type: 'boolean', default: false },
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
    { toNodeHandler, hostHeaderValidation, localhostHostValidation, originValidation },
    { localhostAllowedOrigins },
    { createStatesetMcpServer },
    { createStatesetV2McpServer },
    { seedDemoData },
  ] = await Promise.all([
    import('@stateset/embedded'),
    import('@modelcontextprotocol/server'),
    import('@modelcontextprotocol/node'),
    import('@modelcontextprotocol/server'),
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
  // needs explicit --apply regardless of auth: a key proves who the caller is,
  // not that the operator wants this replica mutating a real store.
  const allowApply = values['read-only'] ? false : ephemeralDb || values.apply;

  // DNS-rebinding protection: a browser on a victim's machine must not reach a
  // loopback MCP server by resolving an attacker domain to 127.0.0.1. A loopback
  // bind gets the localhost allowlist for free. An exposed bind usually sits
  // behind a proxy whose Host header we cannot guess, so it needs an explicit
  // --allowed-host — and FAILS CLOSED without one rather than silently serving
  // any Host. `--insecure-allow-any-host` is the deliberate escape hatch.
  const explicitHosts = values['allowed-host'] ?? [];
  const loopback = host === '127.0.0.1' || host === 'localhost' || host === '::1';
  let validateHost;
  if (explicitHosts.length > 0) {
    validateHost = hostHeaderValidation(explicitHosts);
  } else if (loopback) {
    validateHost = localhostHostValidation();
  } else if (values['insecure-allow-any-host']) {
    validateHost = null;
    console.error(
      `[stateset-mcp-http] WARNING: --insecure-allow-any-host on ${host}: Host header is not ` +
        'validated; DNS-rebinding protection is OFF. Only run this behind a proxy that pins Host.',
    );
  } else {
    throw new Error(
      `refusing to bind ${host}:${port} without a Host allowlist. A non-loopback bind must pass ` +
        '--allowed-host <hostname> (repeatable) with every hostname clients will use, e.g. ' +
        '--allowed-host mcp.example.com. To skip Host validation entirely (behind a proxy that ' +
        'already pins the Host header) pass --insecure-allow-any-host.',
    );
  }

  // Browser cross-origin protection: a web page must not drive this server from
  // an origin the operator has not listed. Non-browser clients send no Origin
  // and always pass. Values may be full origins or bare hostnames; the SDK
  // matches by hostname, so scheme and port are irrelevant.
  const explicitOrigins = (values['allowed-origin'] ?? []).map(originHostname);
  const allowedOrigins =
    explicitOrigins.length > 0 ? explicitOrigins : loopback ? localhostAllowedOrigins() : [];
  const validateOrigin = originValidation(allowedOrigins);

  // Authentication: an exposed bind is reachable by anyone who can route to
  // it, so it FAILS CLOSED without a key. `--insecure-no-auth` is the
  // deliberate escape hatch for a proxy that authenticates upstream. Loopback
  // is local-only by construction; keys there are honoured but optional.
  const apiKeys = collectApiKeys({
    flags: values['api-key'],
    env: process.env[API_KEYS_ENV],
    file: values['api-key-file'],
  });
  const validateAuth = createApiKeyGuard(apiKeys);
  if (!validateAuth && !loopback) {
    if (values['insecure-no-auth']) {
      console.error(
        `[stateset-mcp-http] WARNING: --insecure-no-auth on ${host}: /mcp accepts requests from ` +
          'ANYONE who can reach this port. Only run this behind a proxy that authenticates every request.',
      );
    } else {
      throw new Error(
        `refusing to bind ${host}:${port} without authentication. A non-loopback bind must ` +
          `configure at least one API key via --api-key <key>, ${API_KEYS_ENV}=<key,...> or ` +
          '--api-key-file <path>. To serve without auth (behind a proxy that authenticates ' +
          'every request) pass --insecure-no-auth.',
      );
    }
  }

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

  /** Throwaway response so the auth guard can be consulted without writing a 401. */
  const nullRes = () => ({ writeHead() {}, end() {} });

  const server = createServer((req, res) => {
    void (async () => {
      const url = new URL(req.url || '/', `http://${host}:${port}`);

      if (url.pathname === '/health') {
        // Open for deploy probes, but once auth is on the operational details
        // (store path, write mode) are only for authenticated callers.
        const authed = !validateAuth || validateAuth(req, nullRes());
        if (!authed) {
          sendJson(res, 200, {
            status: 'ok',
            version: CLI_VERSION,
            protocol: '2026-07-28',
            auth: 'required',
          });
          return;
        }
        sendJson(res, 200, {
          status: 'ok',
          version: CLI_VERSION,
          protocol: '2026-07-28',
          auth: validateAuth ? 'required' : 'off',
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
      if (!validateOrigin(req, res)) return;
      if (validateAuth && !validateAuth(req, res)) return;

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
      `legacy 2025 clients: ${values['strict-protocol'] ? 'rejected' : 'served statelessly'}, ` +
      `hosts: ${explicitHosts.length > 0 ? explicitHosts.join(',') : loopback ? 'localhost' : 'ANY (insecure)'}, ` +
      `auth: ${validateAuth ? `required (${apiKeys.length} key${apiKeys.length === 1 ? '' : 's'}: ${apiKeys.map(keyFingerprint).join(',')})` : loopback ? 'off' : 'OFF (insecure)'}, ` +
      `origins: ${allowedOrigins.length > 0 ? allowedOrigins.join(',') : 'none (Origin-bearing requests rejected)'})`,
  );
}

runMain('stateset-mcp-http', main);
