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
import { Commerce } from '@stateset/embedded';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { createStatesetMcpServer } from '../src/mcp-server.js';
import { createMcpEventStreamer } from '../src/mcp-event-streamer.js';
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

OPTIONS:
  --db <path>             Database path (default: ./store.db)
  --host <host>           HTTP host (default: 127.0.0.1)
  --port <port>           HTTP port (default: 8081, 0 picks random port)
  --history-limit <n>     In-memory event history size (default: 500)
  --stream-name <name>    Event stream name (default: stateset-mcp)
  --structured-tool-results  Include machine-readable _agentic metadata in MCP tool results
  --help, -h              Show this help message
  --version, -v           Show version

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

const sendJson = (res, statusCode, payload) => {
  const body = JSON.stringify(payload);
  res.writeHead(statusCode, {
    'Content-Type': 'application/json',
    'Cache-Control': 'no-store',
    'Access-Control-Allow-Origin': '*',
  });
  res.end(body);
};

const runtime = {
  server: null,
  mcpInstance: null,
};

async function main() {
  const { values } = parseArgs({
    options: {
      db: { type: 'string', default: './store.db' },
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

  const host = values.host || '127.0.0.1';
  const port = normalizePort(values.port, 8081);
  const historyLimit = parseLimit(values['history-limit']) || 500;
  const streamName = values['stream-name'];

  let commerce;
  try {
    commerce = new Commerce(values.db);
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
    dbPath: values.db,
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

  const startedAt = Date.now();
  const server = createServer((req, res) => {
    void (async () => {
      const requestUrl = new URL(req.url || '/', `http://${host}:${port}`);
      const pathname = normalizePath(requestUrl.pathname);
      const sessionId = requestUrl.searchParams.get('session') || undefined;

      if (pathname === '/health') {
        if (req.method !== 'GET') {
          sendJson(res, 405, { error: 'Method not allowed' });
          return;
        }

        sendJson(res, 200, {
          status: 'ok',
          uptimeMs: Date.now() - startedAt,
          stream: streamName,
          host,
          port,
          historyLimit,
        });
        return;
      }

      if (pathname === '/events') {
        if (req.method !== 'GET') {
          sendJson(res, 405, { error: 'Method not allowed' });
          return;
        }

        const eventTypes = parseTypes(requestUrl.searchParams.get('types'));
        const subscription = await eventStreamer.subscribe({
          sessionId,
          eventTypes,
        });

        if (!subscription.success) {
          sendJson(res, 500, {
            error: 'Failed to create event subscription',
            details: subscription.error || null,
          });
          return;
        }

        const subscriptionId = subscription.subscription.id;
        eventStreamer.handleSSEConnection(req, res, sessionId);

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
        if (req.method !== 'GET') {
          sendJson(res, 405, { error: 'Method not allowed' });
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

        sendJson(res, 200, { events, count: events.length });
        return;
      }

      if (pathname === '/subscriptions') {
        if (req.method !== 'GET') {
          sendJson(res, 405, { error: 'Method not allowed' });
          return;
        }

        const subscriptions = await eventStreamer.listSubscriptions({ sessionId });
        sendJson(res, 200, { subscriptions, count: subscriptions.length });
        return;
      }

      sendJson(res, 404, {
        error: 'Not found',
        path: pathname,
      });
    })().catch((error) => {
      if (res.writableEnded) return;
      sendJson(res, 500, {
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
  console.log(
    `[stateset-mcp-events] MCP stdio and event gateway active on http://${host}:${advertisedPort}`,
  );
  console.log('[stateset-mcp-events] Endpoints: /events, /history, /subscriptions, /health');
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
