#!/usr/bin/env node

/**
 * StateSet MCP Event Stream Gateway
 *
 * Starts MCP over stdio and exposes an HTTP API for MCP event streaming:
 * - SSE delivery via /events
 * - Event replay via /history
 * - Subscription inspection via /subscriptions
 * - Health checks via /health
 */

import { createServer } from 'node:http';
import { parseArgs } from 'node:util';
import { runMain } from '../src/graceful-shutdown.js';
import { CLI_VERSION } from '../src/config.js';

const HELP = `
StateSet MCP Event Stream Gateway

USAGE:
  stateset-mcp-events [options]

DESCRIPTION:
  Start the StateSet MCP server on stdio and expose HTTP endpoints for live MCP
  execution events.

ENDPOINTS:
  GET /events?session=<id>&types=a,b  SSE stream for matching events
  GET /history                        Fetch recent events
  GET /subscriptions                  List active subscriptions
  GET /health                         Service health
  GET /ready                          Readiness status
  GET /metrics                        Prometheus metrics

OPTIONS:
  --db <path>             Database path (default: $DB_PATH or ./store.db)
  --host <host>           HTTP host (default: 127.0.0.1)
  --port <port>           HTTP port (default: 8081, 0 picks random port)
  --history-limit <n>     In-memory event history size (default: 500)
  --stream-name <name>    Event stream name (default: stateset-mcp)
  --structured-tool-results  Include machine-readable _agentic metadata in MCP tool results
  --help, -h              Show this help message
  --version, -v           Show version

ENVIRONMENT:
  DB_PATH                         Fallback database path when --db is omitted
  STATESET_MCP_ALLOWED_ORIGINS   Comma-separated extra origins allowed for CORS

QUERY PARAMS:
  /events
    session: session id (optional, defaults to global stream)
    types: comma-separated event types, supports '*' wildcard

  /history
    session: session id (optional, defaults to global stream)
    types: comma-separated event types, supports '*' wildcard
    since: ISO timestamp to fetch events after
    limit: max number of events to return

  /subscriptions
    session: session id filter (optional, defaults to global only)
`;

const normalizePort = (value, fallback = 8081) => {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed < 0 || parsed > 65535) return fallback;
  return parsed;
};

const parseTypes = (raw) => {
  if (typeof raw !== 'string') return ['*'];
  const types = raw
    .split(',')
    .map((entry) => entry.trim())
    .filter(Boolean);
  return types.length > 0 ? types : ['*'];
};

const parseLimit = (value) => {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) return null;
  return Math.max(1, parsed);
};

const normalizePath = (pathname) => {
  if (!pathname) return '/';
  if (pathname.length > 1 && pathname.endsWith('/')) {
    return pathname.slice(0, -1);
  }
  return pathname;
};

const LOOPBACK_HOSTS = new Set(['localhost', '127.0.0.1', '::1', '[::1]']);

const parseAllowedOrigins = (raw) => {
  if (typeof raw !== 'string' || !raw.trim()) return new Set();
  return new Set(
    raw
      .split(',')
      .map((entry) => entry.trim())
      .filter(Boolean),
  );
};

const EXTRA_ALLOWED_ORIGINS = parseAllowedOrigins(process.env.STATESET_MCP_ALLOWED_ORIGINS);

const isAllowedCorsOrigin = (origin) => {
  if (typeof origin !== 'string' || !origin.trim()) return false;
  try {
    const parsed = new URL(origin);
    if (LOOPBACK_HOSTS.has(parsed.hostname)) {
      return true;
    }
  } catch {
    return false;
  }
  return EXTRA_ALLOWED_ORIGINS.has(origin);
};

const buildCorsHeaders = (req, baseHeaders = {}) => {
  const origin = req.headers.origin;
  if (!isAllowedCorsOrigin(origin)) {
    return baseHeaders;
  }
  return {
    ...baseHeaders,
    'Access-Control-Allow-Origin': origin,
    'Access-Control-Allow-Methods': 'GET, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type, Accept, Last-Event-ID',
    Vary: 'Origin',
  };
};

const sendJson = (req, res, statusCode, payload) => {
  const body = JSON.stringify(payload);
  res.writeHead(
    statusCode,
    buildCorsHeaders(req, {
      'Content-Type': 'application/json',
      'Cache-Control': 'no-store',
    }),
  );
  res.end(body);
};

const sendText = (
  req,
  res,
  statusCode,
  body,
  contentType = 'text/plain; version=0.0.4; charset=utf-8',
) => {
  res.writeHead(
    statusCode,
    buildCorsHeaders(req, {
      'Content-Type': contentType,
      'Cache-Control': 'no-store',
    }),
  );
  res.end(body);
};

const escapeLabelValue = (value) =>
  String(value).replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\n/g, '\\n');

const runtime = {
  server: null,
  mcpInstance: null,
  startedAt: Date.now(),
  requestCounts: new Map(),
  requestErrors: 0,
};

async function main() {
  const { values } = parseArgs({
    options: {
      db: { type: 'string', default: process.env.DB_PATH || './store.db' },
      host: { type: 'string', default: '127.0.0.1' },
      port: { type: 'string', default: '8081' },
      'history-limit': { type: 'string', default: '500' },
      'stream-name': { type: 'string', default: 'stateset-mcp' },
      'structured-tool-results': { type: 'boolean', short: 's', default: false },
      help: { type: 'boolean', short: 'h', default: false },
      version: { type: 'boolean', short: 'v', default: false },
    },
    allowPositionals: true,
  });

  if (values.help) {
    console.log(HELP);
    return;
  }

  if (values.version) {
    console.log(`stateset-mcp-events v${CLI_VERSION}`);
    return;
  }

  const [
    { Commerce },
    { StdioServerTransport },
    { createStatesetMcpServer },
    { createMcpEventStreamer },
  ] = await Promise.all([
    import('@stateset/embedded'),
    import('@modelcontextprotocol/sdk/server/stdio.js'),
    import('../src/mcp-server.js'),
    import('../src/mcp-event-streamer.js'),
  ]);

  const host = values.host || '127.0.0.1';
  const port = normalizePort(values.port, 8081);
  const historyLimit = parseLimit(values['history-limit']) || 500;
  const streamName = values['stream-name'];
  const dbPath = values.db || process.env.DB_PATH || './store.db';
  runtime.startedAt = Date.now();

  let commerce;
  try {
    commerce = new Commerce(dbPath);
  } catch (error) {
    console.error(`[stateset-mcp-events] database init error: ${error.message}`);
    process.exit(1);
  }

  const eventStreamer = createMcpEventStreamer({
    historyLimit,
    streamName,
  });

  const mcpServer = createStatesetMcpServer({
    commerce,
    dbPath,
    structuredToolResults: values['structured-tool-results'],
    mcpEventStream: eventStreamer,
  });
  const mcpInstance = mcpServer?.instance || mcpServer?.server || mcpServer;
  if (!mcpInstance || typeof mcpInstance.connect !== 'function') {
    throw new Error('Failed to initialize MCP server instance');
  }

  const transport = new StdioServerTransport();
  await mcpInstance.connect(transport);
  runtime.mcpInstance = mcpInstance;
  const server = createServer((req, res) => {
    void (async () => {
      const requestUrl = new URL(req.url || '/', `http://${host}:${port}`);
      const pathname = normalizePath(requestUrl.pathname);
      const method = (req.method || 'GET').toUpperCase();
      const sessionId = requestUrl.searchParams.get('session') || undefined;
      const countRequest = (statusCode) => {
        const key = `${method}|${pathname}|${statusCode}`;
        runtime.requestCounts.set(key, (runtime.requestCounts.get(key) || 0) + 1);
      };
      const sendJsonWithMetrics = (statusCode, payload) => {
        countRequest(statusCode);
        sendJson(req, res, statusCode, payload);
      };
      const sendTextWithMetrics = (statusCode, body, contentType) => {
        countRequest(statusCode);
        sendText(req, res, statusCode, body, contentType);
      };

      if (method === 'OPTIONS') {
        countRequest(204);
        res.writeHead(204, buildCorsHeaders(req, { 'Cache-Control': 'no-store' }));
        res.end();
        return;
      }

      if (pathname === '/health') {
        if (method !== 'GET') {
          sendJsonWithMetrics(405, { error: 'Method not allowed' });
          return;
        }

        sendJsonWithMetrics(200, {
          status: 'ok',
          uptimeMs: Date.now() - runtime.startedAt,
          stream: streamName,
          host,
          port,
          historyLimit,
        });
        return;
      }

      if (pathname === '/ready') {
        if (method !== 'GET') {
          sendJsonWithMetrics(405, { error: 'Method not allowed' });
          return;
        }

        sendJsonWithMetrics(200, {
          status: 'ready',
          timestamp: new Date().toISOString(),
          checks: {
            database: 'ok',
            mcp: 'ok',
            eventStream: 'ok',
          },
        });
        return;
      }

      if (pathname === '/metrics') {
        if (method !== 'GET') {
          sendJsonWithMetrics(405, { error: 'Method not allowed' });
          return;
        }

        const subscriptions = await eventStreamer.listSubscriptions({});
        const lines = [
          '# HELP stateset_mcp_uptime_seconds MCP gateway uptime in seconds.',
          '# TYPE stateset_mcp_uptime_seconds gauge',
          `stateset_mcp_uptime_seconds ${Math.floor((Date.now() - runtime.startedAt) / 1000)}`,
          '# HELP stateset_mcp_active_subscriptions Active event subscriptions.',
          '# TYPE stateset_mcp_active_subscriptions gauge',
          `stateset_mcp_active_subscriptions ${subscriptions.length}`,
          '# HELP stateset_mcp_request_errors_total Unhandled request errors.',
          '# TYPE stateset_mcp_request_errors_total counter',
          `stateset_mcp_request_errors_total ${runtime.requestErrors}`,
          '# HELP stateset_mcp_http_requests_total Total HTTP requests by method/path/status.',
          '# TYPE stateset_mcp_http_requests_total counter',
        ];

        for (const [key, value] of runtime.requestCounts.entries()) {
          const [requestMethod, requestPath, statusCode] = key.split('|');
          lines.push(
            `stateset_mcp_http_requests_total{method="${escapeLabelValue(requestMethod)}",path="${escapeLabelValue(requestPath)}",status="${escapeLabelValue(statusCode)}"} ${value}`,
          );
        }

        sendTextWithMetrics(200, `${lines.join('\n')}\n`);
        return;
      }

      if (pathname === '/events') {
        if (method !== 'GET') {
          sendJsonWithMetrics(405, { error: 'Method not allowed' });
          return;
        }

        const eventTypes = parseTypes(requestUrl.searchParams.get('types'));
        const subscription = await eventStreamer.subscribe({
          sessionId,
          eventTypes,
          exposeInListings: Boolean(sessionId),
        });

        if (!subscription.success) {
          sendJsonWithMetrics(500, {
            error: 'Failed to create event subscription',
            details: subscription.error || null,
          });
          return;
        }

        const subscriptionId = subscription.subscription.id;
        eventStreamer.handleSSEConnection(req, res, {
          sessionId,
          subscriptionId,
          headers: buildCorsHeaders(req),
        });

        let closed = false;
        req.on('close', () => {
          if (closed) return;
          closed = true;
          eventStreamer.unsubscribe(subscriptionId).catch(() => {
            // best-effort cleanup
          });
        });
        return;
      }

      if (pathname === '/history') {
        if (method !== 'GET') {
          sendJsonWithMetrics(405, { error: 'Method not allowed' });
          return;
        }

        const eventTypes = parseTypes(requestUrl.searchParams.get('types'));
        const limit = parseLimit(requestUrl.searchParams.get('limit'));
        const events = await eventStreamer.getEventHistory({
          sessionId,
          eventTypes,
          since: requestUrl.searchParams.get('since') || undefined,
          limit,
        });

        sendJsonWithMetrics(200, { events, count: events.length });
        return;
      }

      if (pathname === '/subscriptions') {
        if (method !== 'GET') {
          sendJsonWithMetrics(405, { error: 'Method not allowed' });
          return;
        }

        const subscriptions = await eventStreamer.listSubscriptions({ sessionId });
        sendJsonWithMetrics(200, { subscriptions, count: subscriptions.length });
        return;
      }

      sendJsonWithMetrics(404, {
        error: 'Not found',
        path: pathname,
      });
    })().catch((error) => {
      if (res.writableEnded) return;
      runtime.requestErrors += 1;
      sendJson(req, res, 500, {
        error: 'Request failed',
        details: error?.message || String(error),
      });
    });
  });

  await new Promise((resolve, reject) => {
    const onError = (error) => {
      reject(error);
    };

    server.once('error', onError);
    server.listen(port, host, () => {
      server.off('error', onError);
      resolve();
    });
  });

  runtime.server = server;
  const address = server.address();
  const advertisedPort =
    typeof address === 'object' && address && address.port ? address.port : port;
  console.error(
    `[stateset-mcp-events] MCP stdio and event gateway active on http://${host}:${advertisedPort}`,
  );
  console.error(
    '[stateset-mcp-events] Endpoints: /events, /history, /subscriptions, /health, /ready, /metrics',
  );
}

runMain('stateset-mcp-events', main, {
  cleanup: async () => {
    const server = runtime.server;
    const mcpInstance = runtime.mcpInstance;

    if (server && server.listening) {
      await new Promise((resolve) => {
        server.close(resolve);
      });
    }

    if (mcpInstance?.close) {
      await mcpInstance.close();
    }
  },
});
