import { createHash, randomUUID } from 'node:crypto';
import { performance } from 'node:perf_hooks';
import { createApiKeyAuth } from '../channels/http-auth.js';
import { createToolInputSchema } from '../tool-schema.js';
import { createMuseRequestControls } from './muse-request-controls.js';

export const MUSE_DEFAULT_TOOLS = Object.freeze([
  'list_products',
  'get_product',
  'get_stock',
  'list_orders',
  'get_order',
  'get_sales_summary',
  'create_product',
  'update_order_status',
]);

/** A standard HTTP/OpenAPI service for Muse custom connectors; not a Meta manifest format. */
export function createMuseConnector({
  toolkit,
  apiKeys,
  accounts,
  publicUrl,
  allowedHosts = [],
  allowApply = false,
  tools = MUSE_DEFAULT_TOOLS,
  maxBodyBytes = 65536,
  requestsPerMinute = 60,
  publicRequestsPerMinute = 120,
  maxInFlight = 16,
  maxAccountInFlight = 4,
  rateLimitWindowMs = 60000,
  onAudit,
}) {
  const base = new URL(publicUrl);
  const loopback = ['localhost', '127.0.0.1', '[::1]'].includes(base.hostname);
  if (
    (base.protocol !== 'https:' && !(base.protocol === 'http:' && loopback)) ||
    base.username ||
    base.password ||
    base.pathname !== '/' ||
    base.search ||
    base.hash
  ) {
    throw new Error(
      'publicUrl must be an HTTPS origin (HTTP is allowed only for loopback testing).',
    );
  }
  if (!Array.isArray(tools) || tools.length === 0 || new Set(tools).size !== tools.length) {
    throw new Error('tools must be a nonempty, unique allowlist.');
  }
  if (accounts !== undefined && apiKeys !== undefined) {
    throw new Error('Configure accounts or API keys, not both.');
  }
  const configuredAccounts = accounts ?? [{ id: 'muse-store', apiKeys, tools, allowApply }];
  if (!Array.isArray(configuredAccounts) || configuredAccounts.length === 0) {
    throw new Error('accounts must be a nonempty array.');
  }
  const accountIds = new Set();
  const credentialKeys = new Set();
  for (const account of configuredAccounts) {
    if (
      !account ||
      typeof account.id !== 'string' ||
      !/^[a-zA-Z0-9_-]{1,64}$/.test(account.id) ||
      accountIds.has(account.id)
    ) {
      throw new Error('Account IDs must be unique names of 1–64 characters.');
    }
    accountIds.add(account.id);
    if (account.allowApply !== undefined && typeof account.allowApply !== 'boolean') {
      throw new Error('Account allowApply must be a boolean.');
    }
    if (
      !Array.isArray(account.tools) ||
      !account.tools.length ||
      new Set(account.tools).size !== account.tools.length ||
      account.tools.some((name) => !tools.includes(name))
    ) {
      throw new Error('Account tools must be a nonempty, unique subset of the service allowlist.');
    }
    if (
      !Array.isArray(account.apiKeys) ||
      account.apiKeys.length === 0 ||
      account.apiKeys.some(
        (key) => typeof key !== 'string' || key.length < 16 || credentialKeys.has(key),
      )
    ) {
      throw new Error('Muse connector requires unique API keys of at least 16 characters.');
    }
    for (const key of account.apiKeys) {
      if (credentialKeys.has(key)) throw new Error('Duplicate API keys are not allowed.');
      credentialKeys.add(key);
    }
  }
  if (!Number.isSafeInteger(maxBodyBytes) || maxBodyBytes < 1) {
    throw new Error('maxBodyBytes must be a positive integer.');
  }
  const hosts = new Set([base.hostname, ...allowedHosts]);
  if (onAudit !== undefined && typeof onAudit !== 'function')
    throw new Error('onAudit must be a function.');
  const auth = createApiKeyAuth(
    configuredAccounts.flatMap((account) =>
      account.apiKeys.map((key) => ({ key, name: account.id })),
    ),
  );
  const available = new Map(
    toolkit.getTools({ format: 'generic' }).map((tool) => [tool.name, tool]),
  );
  const selected = tools.map((name) => {
    if (!/^[a-zA-Z0-9_]+$/.test(name) || !available.has(name)) {
      throw new Error(`Unknown or unavailable connector tool '${name}'.`);
    }
    const tool = available.get(name);
    const raw = toolkit.getRawTool(name);
    if (!raw) throw new Error(`Missing validation schema for '${name}'.`);
    return { ...tool, validator: createToolInputSchema(raw.inputSchema).strict() };
  });
  const byName = new Map(selected.map((tool) => [tool.name, tool]));
  const catalog = selected.map(({ name, description, inputSchema, permission }) => ({
    name,
    description,
    inputSchema,
    permission,
    previewOnly: permission !== 'read' && allowApply !== true,
  }));
  const errorResponse = {
    description: 'Connector error',
    headers: { 'X-Request-Id': { schema: { type: 'string', format: 'uuid' } } },
    content: {
      'application/json': { schema: { type: 'object', properties: { error: { type: 'string' } } } },
    },
  };
  const buildOpenapi = (selected, allowApply) => ({
    openapi: '3.1.0',
    info: {
      title: 'StateSet iCommerce for Muse',
      version: '1.0.0',
      description:
        'Store-scoped commerce tools. Writes preview by default. Tool results retain StateSet policy and audit evidence.',
    },
    servers: [{ url: base.origin }],
    security: [{ bearerAuth: [] }],
    components: { securitySchemes: { bearerAuth: { type: 'http', scheme: 'bearer' } } },
    paths: Object.fromEntries(
      selected.map((tool) => [
        `/v1/tools/${tool.name}`,
        {
          post: {
            operationId: tool.name,
            summary: tool.description?.split('\n')[0],
            description: tool.description,
            ...(tool.permission !== 'read' && allowApply === true
              ? {
                  parameters: [
                    {
                      name: 'Idempotency-Key',
                      in: 'header',
                      required: true,
                      description:
                        'Reuse this key only for retries of the same write and arguments.',
                      schema: {
                        type: 'string',
                        minLength: 8,
                        maxLength: 128,
                        pattern: '^[a-zA-Z0-9_.:-]+$',
                      },
                    },
                  ],
                }
              : {}),
            'x-stateset-permission': tool.permission,
            'x-stateset-preview-only': tool.permission !== 'read' && allowApply !== true,
            requestBody: {
              required: true,
              content: {
                'application/json': {
                  schema: {
                    ...tool.inputSchema,
                    additionalProperties: false,
                  },
                },
              },
            },
            responses: {
              200: {
                headers: { 'X-Request-Id': { schema: { type: 'string', format: 'uuid' } } },
                description:
                  'StateSet tool result or write preview. Inspect the result for commercial success and kernel receipts.',
                content: { 'application/json': { schema: {} } },
              },
              400: errorResponse,
              401: errorResponse,
              403: errorResponse,
              413: errorResponse,
              415: errorResponse,
              429: {
                ...errorResponse,
                description: 'Request budget exhausted; wait before retrying',
                headers: {
                  ...errorResponse.headers,
                  'Retry-After': { schema: { type: 'integer', minimum: 1 } },
                },
              },
              503: {
                ...errorResponse,
                description: 'Concurrent tool request capacity reached; wait before retrying',
                headers: {
                  ...errorResponse.headers,
                  'Retry-After': { schema: { type: 'integer', minimum: 1 } },
                },
              },
              500: errorResponse,
            },
          },
        },
      ]),
    ),
  });
  const openapi = buildOpenapi(selected, allowApply);
  const scopes = new Map(
    configuredAccounts.map((account) => {
      const canApply = allowApply === true && account.allowApply === true;
      const accountTools = account.tools.map((name) => byName.get(name));
      return [
        account.id,
        {
          allowApply: canApply,
          tools: new Set(account.tools),
          catalog: catalog
            .filter((tool) => account.tools.includes(tool.name))
            .map((tool) => ({ ...tool, previewOnly: tool.permission !== 'read' && !canApply })),
          openapi: buildOpenapi(accountTools, canApply),
        },
      ];
    }),
  );
  const controls = createMuseRequestControls({
    accountIds,
    requestsPerMinute,
    publicRequestsPerMinute,
    maxInFlight,
    maxAccountInFlight,
    windowMs: rateLimitWindowMs,
  });
  const json = (res, status, value, headers = {}) => {
    if (res.destroyed) return;
    res.writeHead(status, {
      'Content-Type': 'application/json',
      'Cache-Control': 'no-store',
      'X-Content-Type-Options': 'nosniff',
      ...headers,
    });
    res.end(JSON.stringify(value));
  };
  const handler = async (req, res) => {
    const requestId = randomUUID();
    const started = performance.now();
    const timestamp = new Date().toISOString();
    let accountId = null;
    let auditTool = null;
    let mode = null;
    let outcome = 'request_rejected';
    let release;
    let disconnected = false;
    res.once('close', () => {
      if (!res.writableFinished) disconnected = true;
    });
    res.setHeader('X-Request-Id', requestId);
    const rateResponse = (id) => {
      const rate = controls.checkRate(id);
      if (rate.allowed) return false;
      outcome = 'rate_limited';
      json(
        res,
        429,
        { error: 'Request limit exceeded' },
        { 'Retry-After': String(Math.ceil(rate.retryAfterMs / 1000)) },
      );
      return true;
    };
    const execute = async (...args) => {
      const result = await toolkit.executeTool(...args);
      outcome =
        result?.success === false || result?.result?.success === false
          ? 'commerce_denied'
          : 'tool_completed';
      return result;
    };
    try {
      let host;
      try {
        const authority = new URL(`http://${req.headers.host}`);
        if (
          !authority.username &&
          !authority.password &&
          authority.pathname === '/' &&
          !authority.search &&
          !authority.hash
        )
          host = authority.hostname;
      } catch {
        /* rejected below */
      }
      if (!hosts.has(host)) return json(res, 403, { error: 'Host not allowed' });
      // This is a server-to-server connector. Browser origins must match the service origin.
      if (req.headers.origin && req.headers.origin !== base.origin) {
        return json(res, 403, { error: 'Origin not allowed' });
      }
      const url = new URL(req.url, base);
      if (req.method === 'GET' && url.pathname === '/health') {
        outcome = 'health';
        return json(res, 200, { status: 'ok' });
      }
      if (req.method === 'GET' && url.pathname === '/openapi.json' && !req.headers.authorization) {
        if (rateResponse()) return;
        outcome = 'discovery';
        return json(res, 200, openapi);
      }
      const identity = auth.authenticate(req, url);
      if (!identity.authenticated) {
        if (rateResponse()) return;
        return json(
          res,
          401,
          { error: 'Bearer authentication required' },
          { 'WWW-Authenticate': 'Bearer' },
        );
      }
      accountId = identity.identity.name;
      if (rateResponse(accountId)) return;
      const scope = scopes.get(identity.identity.name);
      if (req.method === 'GET' && url.pathname === '/openapi.json') {
        outcome = 'discovery';
        return json(res, 200, scope.openapi);
      }
      if (req.method === 'GET' && url.pathname === '/v1/tools') {
        outcome = 'discovery';
        return json(res, 200, { tools: scope.catalog });
      }
      const match = /^\/v1\/tools\/([a-zA-Z0-9_]+)$/.exec(url.pathname);
      const tool = match && byName.get(match[1]);
      if (!tool || !scope.tools.has(tool.name)) return json(res, 404, { error: 'Tool not found' });
      auditTool = tool.name;
      mode = tool.permission === 'read' ? 'read' : scope.allowApply ? 'apply' : 'preview';
      if (req.method !== 'POST') return json(res, 405, { error: 'Use POST' }, { Allow: 'POST' });
      release = controls.acquire(accountId);
      if (!release) {
        outcome = 'capacity_limited';
        return json(res, 503, { error: 'Commerce tool capacity reached' }, { 'Retry-After': '1' });
      }
      if (req.headers['content-type']?.split(';')[0].trim().toLowerCase() !== 'application/json') {
        return json(res, 415, { error: 'Expected application/json' });
      }
      let size = 0;
      const chunks = [];
      for await (const chunk of req) {
        size += chunk.length;
        if (size > maxBodyBytes) return json(res, 413, { error: 'Request too large' });
        chunks.push(chunk);
      }
      let params;
      try {
        params = JSON.parse(Buffer.concat(chunks).toString('utf8'));
      } catch {
        return json(res, 400, { error: 'Invalid JSON' });
      }
      const validated = tool.validator.safeParse(params);
      if (!validated.success)
        return json(res, 400, {
          error: 'Invalid tool arguments',
          issues: validated.error.issues.map(({ path, message }) => ({ path, message })),
        });
      if (tool.permission !== 'read' && !scope.allowApply) {
        outcome = 'preview';
        return json(res, 200, {
          preview: true,
          tool: tool.name,
          params: validated.data,
          note: 'Write not executed. The operator must enable governed writes for the service and this account.',
        });
      }
      if (tool.permission !== 'read') {
        const key = req.headers['idempotency-key'];
        if (typeof key !== 'string' || !/^[a-zA-Z0-9_.:-]{8,128}$/.test(key)) {
          return json(res, 400, {
            error: 'Writes require a valid Idempotency-Key header (8–128 characters).',
          });
        }
        // The only caller-controlled execution option is the validated retry key.
        return json(
          res,
          200,
          await execute(tool.name, validated.data, {
            idempotencyKey:
              accounts === undefined
                ? key
                : `muse:${createHash('sha256')
                    .update(JSON.stringify([identity.identity.name, key]))
                    .digest('hex')}`,
          }),
        );
      }
      // No model-supplied execution options, principal, policy, or database path are accepted.
      return json(res, 200, await execute(tool.name, validated.data));
    } catch {
      outcome = 'internal_error';
      // Avoid exposing credentials, local database paths, or internal stack traces.
      if (!res.headersSent && !res.destroyed)
        json(res, 500, { error: 'Commerce tool execution failed' });
    } finally {
      release?.();
      if (onAudit) {
        const connectionLost = disconnected || (res.destroyed && !res.writableFinished);
        const event = Object.freeze({
          timestamp,
          requestId,
          accountId,
          tool: auditTool,
          mode,
          outcome,
          statusCode: connectionLost ? null : res.statusCode,
          durationMs: Math.round((performance.now() - started) * 1000) / 1000,
          disconnected: connectionLost,
        });
        try {
          // Transport telemetry is best effort; sink failures must never replay a mutation.
          Promise.resolve(onAudit(event)).catch(() => {});
        } catch {
          /* A failing telemetry sink cannot change an executed commerce command. */
        }
      }
    }
  };
  return { handler, openapi, catalog, close: () => controls.close() };
}
