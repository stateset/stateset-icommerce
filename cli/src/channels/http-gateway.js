/**
 * HTTP Gateway for StateSet iCommerce Plugin System
 *
 * Minimal HTTP server exposing:
 * - Health check
 * - Metrics
 * - Plugin management
 * - Plugin-registered HTTP routes
 *
 * Uses Node.js built-in http module. No external dependencies.
 */

import http from 'http';
import { isIP } from 'node:net';
import { getPluginRegistry } from './plugin-api.js';
import { getCommandRegistry } from './command-registry.js';
import { getMetrics } from './metrics.js';
import {
  LEVELS,
  createApiKeyAuth,
  checkRoutePermission,
  checkSandbox,
  getRequiredLevel,
} from './http-auth.js';
import { createRateLimiter } from './rate-limiter.js';
import { validateBrowserExpression } from './browser-evaluate-policy.js';

// ============================================================================
// Helpers
// ============================================================================

/** Maximum request body size (1 MB) */
const MAX_BODY_SIZE = 1_048_576;

/** Common security headers applied to every response */
const SECURITY_HEADERS = {
  'X-Content-Type-Options': 'nosniff',
  'X-Frame-Options': 'DENY',
  'X-XSS-Protection': '0',
  'Referrer-Policy': 'strict-origin-when-cross-origin',
  'Content-Security-Policy': "default-src 'none'; frame-ancestors 'none'",
};

const MAX_BROWSER_URL_LENGTH = 2_048;
const ALLOWED_BROWSER_PROTOCOLS = new Set(['http:', 'https:']);
const ALLOW_PRIVATE_BROWSER_URLS = process.env.STATESET_ALLOW_PRIVATE_BROWSER_URLS === '1';

function isPrivateIpv4(hostname) {
  const parts = hostname.split('.').map((part) => Number.parseInt(part, 10));
  if (
    parts.length !== 4 ||
    parts.some((part) => !Number.isFinite(part) || part < 0 || part > 255)
  ) {
    return false;
  }
  const [a, b] = parts;
  return (
    a === 10 ||
    a === 127 ||
    (a === 169 && b === 254) ||
    (a === 172 && b >= 16 && b <= 31) ||
    (a === 192 && b === 168)
  );
}

function isPrivateIpv6(hostname) {
  const normalized = hostname.toLowerCase();
  return (
    normalized === '::1' ||
    normalized.startsWith('fc') ||
    normalized.startsWith('fd') ||
    normalized.startsWith('fe80:') ||
    normalized.startsWith('::ffff:127.') ||
    normalized.startsWith('::ffff:10.') ||
    normalized.startsWith('::ffff:192.168.') ||
    /^::ffff:172\.(1[6-9]|2\d|3[0-1])\./.test(normalized)
  );
}

function isBlockedBrowserHostname(hostname) {
  if (ALLOW_PRIVATE_BROWSER_URLS) {
    return false;
  }

  const normalized = String(hostname || '')
    .trim()
    .toLowerCase()
    .replace(/\.$/, '');
  if (!normalized) {
    return true;
  }

  if (
    normalized === 'localhost' ||
    normalized.endsWith('.localhost') ||
    normalized.endsWith('.local') ||
    normalized.endsWith('.internal')
  ) {
    return true;
  }

  const ipVersion = isIP(normalized);
  if (ipVersion === 4) {
    return isPrivateIpv4(normalized);
  }
  if (ipVersion === 6) {
    return isPrivateIpv6(normalized);
  }

  // Disallow single-label hostnames by default to reduce internal DNS SSRF risk.
  return !normalized.includes('.');
}

function sanitizeHostHeader(hostHeader) {
  if (typeof hostHeader !== 'string') return 'localhost';
  const trimmed = hostHeader.trim();
  if (!trimmed) return 'localhost';

  const ipv6 = trimmed.match(/^\[([^\]]+)\]/);
  const host = ipv6 ? ipv6[1] : trimmed.split(':')[0];
  return host.replace(/[^A-Za-z0-9\-.:\u005B\u005D]/g, '') || 'localhost';
}

function safeParseUrl(reqUrl, hostHeader) {
  try {
    return new URL(reqUrl, `http://${sanitizeHostHeader(hostHeader)}`);
  } catch {
    return null;
  }
}

function validateBrowserUrl(value) {
  if (typeof value !== 'string') {
    return 'Missing required field: url';
  }

  const trimmed = value.trim();
  if (!trimmed) {
    return 'Missing required field: url';
  }

  if (trimmed.length > MAX_BROWSER_URL_LENGTH) {
    return `URL exceeds maximum length of ${MAX_BROWSER_URL_LENGTH} characters`;
  }

  let parsed;
  try {
    parsed = new URL(trimmed);
  } catch {
    return 'Invalid URL';
  }

  if (!ALLOWED_BROWSER_PROTOCOLS.has(parsed.protocol)) {
    return 'Only http(s) URLs are allowed';
  }

  if (!parsed.hostname) {
    return 'Invalid URL';
  }

  if (isBlockedBrowserHostname(parsed.hostname)) {
    return 'URL host is not allowed by browser safety policy';
  }

  return null;
}

/**
 * Parse JSON body from an incoming request.
 * @param {http.IncomingMessage} req
 * @param {number} [maxSize] - Maximum body size in bytes
 * @returns {Promise<Object>}
 */
function parseBody(req, maxSize = MAX_BODY_SIZE) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    let size = 0;
    req.on('data', (chunk) => {
      size += chunk.length;
      if (size > maxSize) {
        req.destroy();
        reject(new Error(`Request body exceeds maximum size of ${maxSize} bytes`));
        return;
      }
      chunks.push(chunk);
    });
    req.on('end', () => {
      try {
        const raw = Buffer.concat(chunks).toString('utf-8');
        resolve(raw ? JSON.parse(raw) : {});
      } catch (err) {
        reject(new Error(`Invalid JSON body: ${err.message}`));
      }
    });
    req.on('error', reject);
  });
}

/**
 * Send a JSON response.
 * @param {http.ServerResponse} res
 * @param {number} status
 * @param {Object} data
 */
function sendJson(res, status, data) {
  const body = JSON.stringify(data, null, 2);
  res.writeHead(status, {
    'Content-Type': 'application/json',
    ...SECURITY_HEADERS,
  });
  res.end(body);
}

/**
 * Parse raw body as a Buffer from an incoming request.
 * @param {http.IncomingMessage} req
 * @returns {Promise<Buffer>}
 */
function parseRawBody(req, maxSize = MAX_BODY_SIZE) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    let size = 0;
    req.on('data', (chunk) => {
      size += chunk.length;
      if (size > maxSize) {
        req.destroy();
        reject(new Error(`Request body exceeds maximum size of ${maxSize} bytes`));
        return;
      }
      chunks.push(chunk);
    });
    req.on('end', () => resolve(Buffer.concat(chunks)));
    req.on('error', reject);
  });
}

/**
 * Send an HTML response.
 * @param {http.ServerResponse} res
 * @param {number} status
 * @param {string} html
 */
function sendHtml(res, status, html) {
  res.writeHead(status, {
    'Content-Type': 'text/html; charset=utf-8',
    ...SECURITY_HEADERS,
  });
  res.end(html);
}

/**
 * Send a binary response (e.g. audio buffer).
 * @param {http.ServerResponse} res
 * @param {number} status
 * @param {Buffer} data
 * @param {string} contentType
 */
function sendBinary(res, status, data, contentType) {
  res.writeHead(status, {
    'Content-Type': contentType,
    'Content-Length': data.length,
    ...SECURITY_HEADERS,
  });
  res.end(data);
}

/**
 * Extract path parameters using a simple pattern matcher.
 * Pattern: "/plugins/:id/enable" matches "/plugins/my-plugin/enable"
 * @param {string} pattern
 * @param {string} pathname
 * @returns {Object|null} - Params or null if no match
 */
function matchRoute(pattern, pathname) {
  const patternParts = pattern.split('/');
  const pathParts = pathname.split('/');

  if (patternParts.length !== pathParts.length) return null;

  const params = {};
  for (let i = 0; i < patternParts.length; i++) {
    if (patternParts[i].startsWith(':')) {
      params[patternParts[i].slice(1)] = decodeURIComponent(pathParts[i]);
    } else if (patternParts[i] !== pathParts[i]) {
      return null;
    }
  }

  return params;
}

function isLoopbackAddress(addr) {
  if (!addr) return false;
  // Common forms:
  // - 127.0.0.1
  // - ::1
  // - ::ffff:127.0.0.1
  return addr === '127.0.0.1' || addr === '::1' || addr.startsWith('::ffff:127.');
}

function isLoopbackOrigin(origin) {
  try {
    const u = new URL(origin);
    return u.hostname === 'localhost' || u.hostname === '127.0.0.1' || u.hostname === '::1';
  } catch (err) {
    console.debug('[http-gateway] Origin URL parse failed:', err.message || err);
    return false;
  }
}

function normalizeCorsOrigins(origins) {
  if (origins === null || origins === false) return [];
  if (origins === undefined) return ['loopback'];
  if (typeof origins === 'string') {
    return origins
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean);
  }
  if (Array.isArray(origins)) {
    return origins.map((s) => String(s).trim()).filter(Boolean);
  }
  return ['loopback'];
}

function getCorsAllowOrigin(origin, allowlist) {
  if (!origin) return null;
  if (!allowlist || allowlist.length === 0) return null;
  if (allowlist.includes('*')) return '*';
  if (allowlist.includes(origin)) return origin;
  if (allowlist.includes('loopback') && isLoopbackOrigin(origin)) return origin;
  return null;
}

async function defaultDatabaseReadinessCheck(dbPath) {
  const { Commerce } = await import('@stateset/embedded');
  if (!Commerce) {
    throw new Error('Embedded commerce engine is unavailable');
  }

  const commerce = new Commerce(dbPath);
  await commerce.orders.list({ limit: 1 });
}

// ============================================================================
// HttpGateway
// ============================================================================

const startTime = Date.now();

export class HttpGateway {
  /**
   * @param {Object} opts
   * @param {number} [opts.port=0] - Port (0 = random)
   * @param {string} [opts.host='127.0.0.1']
   * @param {string} [opts.dbPath='./store.db']
   * @param {boolean} [opts.verbose=false]
   * @param {import('./plugin-config.js').PluginConfigState} [opts.configState]
   * @param {Array<{ key: string, name: string, level?: string, channels?: string[] }>} [opts.apiKeys]
   * @param {boolean} [opts.allowAnonymous=false] - Insecure: allow requests without keys when no keys configured
   * @param {{ name?: string, level?: string, channels?: string[] }} [opts.anonymousIdentity]
   * @param {boolean} [opts.allowQueryParamAuth=false] - Insecure: allow `?api_key=...` authentication
   * @param {string[]|string} [opts.corsOrigins] - Allowed CORS origins; supports special value "loopback"
   * @param {boolean} [opts.allowRemoteAdminEndpoints=false] - Allow /daemon and /remote-access from non-loopback clients
   * @param {{ browser?: boolean, shell?: boolean }} [opts.sandbox]
   * @param {number} [opts.rateLimitAuth=60]
   * @param {number} [opts.rateLimitUnauth=30]
   * @param {number} [opts.rateLimitWindowMs=60000]
   * @param {boolean} [opts.allowBrowserEvaluate=false] - Enable read-only /browser/evaluate route
   * @param {(dbPath: string) => Promise<void>} [opts.databaseReadinessCheck]
   */
  constructor(opts = {}) {
    this._port = opts.port || 0;
    this._host = opts.host || '127.0.0.1';
    this._dbPath = opts.dbPath || process.env.DB_PATH || './store.db';
    this._databaseReadinessCheck = opts.databaseReadinessCheck || defaultDatabaseReadinessCheck;
    this._verbose = opts.verbose || false;
    this._configState = opts.configState || null;
    this._server = null;
    this._address = null;
    this._subsystems = { voice: null, browser: null, memory: null, heartbeat: null };

    // Auth & sandbox
    const rawApiKeys = Array.isArray(opts.apiKeys) ? opts.apiKeys : [];
    this._validApiKeyCount = rawApiKeys.filter(
      (e) => e && typeof e.key === 'string' && e.key.length > 0,
    ).length;
    this._allowAnonymous = opts.allowAnonymous === true;
    this._allowQueryParamAuth = opts.allowQueryParamAuth === true;
    this._auth = createApiKeyAuth(opts.apiKeys || [], {
      allowAnonymous: this._allowAnonymous,
      anonymousIdentity: opts.anonymousIdentity,
      allowQueryParam: this._allowQueryParamAuth,
    });
    this._sandbox = opts.sandbox || null;
    this._allowBrowserEvaluate = opts.allowBrowserEvaluate === true;
    this._orchestratorStatus = null;
    this._allowRemoteAdminEndpoints = opts.allowRemoteAdminEndpoints === true;
    this._corsOrigins = normalizeCorsOrigins(opts.corsOrigins);

    // Rate limiting
    this._rateLimiter = createRateLimiter({
      authenticatedMax: opts.rateLimitAuth || 60,
      unauthenticatedMax: opts.rateLimitUnauth || 30,
      windowMs: opts.rateLimitWindowMs || 60_000,
    });
  }

  /**
   * Set orchestrator status callback for /health reporting.
   * @param {() => Object} fn - Returns channel/subsystem status
   */
  setOrchestratorStatus(fn) {
    this._orchestratorStatus = fn;
  }

  /**
   * Store references to subsystems.
   * @param {{ voice?: Object, browser?: Object, memory?: Object, heartbeat?: Object }} subs
   */
  setSubsystems(subs = {}) {
    if (subs.voice !== undefined) this._subsystems.voice = subs.voice;
    if (subs.browser !== undefined) this._subsystems.browser = subs.browser;
    if (subs.memory !== undefined) this._subsystems.memory = subs.memory;
    if (subs.heartbeat !== undefined) this._subsystems.heartbeat = subs.heartbeat;
  }

  /**
   * Start the HTTP gateway.
   * @returns {Promise<{ host: string, port: number }>}
   */
  async start() {
    this._server = http.createServer((req, res) => this._handleRequest(req, res));

    return new Promise((resolve, reject) => {
      this._server.listen(this._port, this._host, () => {
        this._address = this._server.address();
        if (this._verbose) {
          console.info(`[HttpGateway] Listening on ${this._address.address}:${this._address.port}`);
        }
        if (this._validApiKeyCount === 0 && !this._allowAnonymous) {
          console.warn(
            '[HttpGateway] No API keys configured; protected routes will return 401. Set httpGateway.apiKeys (or allowAnonymous for local debugging).',
          );
        }
        resolve({ host: this._address.address, port: this._address.port });
      });

      this._server.on('error', reject);
    });
  }

  /**
   * Stop the HTTP gateway.
   * @returns {Promise<void>}
   */
  async stop() {
    if (!this._server) return;

    return new Promise((resolve) => {
      this._server.close(() => {
        if (this._verbose) {
          console.info('[HttpGateway] Stopped.');
        }
        this._server = null;
        this._address = null;
        if (this._rateLimiter) {
          this._rateLimiter.destroy();
        }
        resolve();
      });
    });
  }

  /**
   * Get the server address.
   * @returns {{ host: string, port: number }|null}
   */
  getAddress() {
    return this._address ? { host: this._address.address, port: this._address.port } : null;
  }

  // ============================================================================
  // Request Handler
  // ============================================================================

  /**
   * @private
   */
  async _handleRequest(req, res) {
    const url = safeParseUrl(req.url, req.headers.host);
    if (!url) {
      return sendJson(res, 400, { error: 'Invalid request URL' });
    }

    const pathname = url.pathname;
    const method = req.method.toUpperCase();
    const clientIp = req.socket.remoteAddress || 'unknown';

    // CORS preflight
    if (method === 'OPTIONS') {
      const allowedOrigin = getCorsAllowOrigin(req.headers.origin, this._corsOrigins);
      res.writeHead(204, {
        ...(allowedOrigin
          ? {
              'Access-Control-Allow-Origin': allowedOrigin,
              ...(allowedOrigin === '*' ? {} : { Vary: 'Origin' }),
              'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS, PATCH',
              'Access-Control-Allow-Headers': 'Content-Type, Authorization',
              'Access-Control-Max-Age': '600',
            }
          : {}),
        ...SECURITY_HEADERS,
      });
      res.end();
      return;
    }

    // CORS response headers (only when Origin is allowed)
    {
      const allowedOrigin = getCorsAllowOrigin(req.headers.origin, this._corsOrigins);
      if (allowedOrigin) {
        res.setHeader('Access-Control-Allow-Origin', allowedOrigin);
        if (allowedOrigin !== '*') {
          res.setHeader('Vary', 'Origin');
        }
      }
    }

    if (this._verbose) {
      console.debug(`[HttpGateway] ${method} ${pathname}`);
    }

    try {
      // --- Route match / required level (used to decide if auth is required) ---
      const builtinRequiredLevel = getRequiredLevel(pathname, method);
      const pluginRoutes = builtinRequiredLevel ? null : getPluginRegistry().getRoutes();
      let matchedPlugin = null;
      let matchedPluginParams = null;
      if (pluginRoutes) {
        for (const route of pluginRoutes) {
          if (method !== route.method.toUpperCase()) continue;
          const params = matchRoute(route.path, pathname);
          if (params) {
            matchedPlugin = route;
            matchedPluginParams = params;
            break;
          }
        }
      }

      let requiredLevelName = builtinRequiredLevel;
      if (matchedPlugin) {
        const raw = matchedPlugin.level ? String(matchedPlugin.level).toLowerCase() : 'read';
        requiredLevelName = Object.hasOwn(LEVELS, raw) ? raw : 'read';
      }

      // Unknown route: return 404 without forcing auth.
      if (!requiredLevelName) {
        return sendJson(res, 404, { error: 'Not found', path: pathname });
      }

      // --- Rate limiting (IP-based for public routes & unauthenticated attempts) ---
      if (requiredLevelName === 'none') {
        const ipResult = this._rateLimiter.checkIp(clientIp);
        if (!ipResult.allowed) {
          res.setHeader('Retry-After', Math.ceil(ipResult.retryAfterMs / 1000));
          return sendJson(res, 429, {
            error: 'Too many requests',
            retryAfterMs: ipResult.retryAfterMs,
          });
        }
      }

      // --- Built-in routes ---

      if (method === 'GET' && pathname === '/health') {
        const health = {
          status: 'ok',
          uptime: Date.now() - startTime,
          timestamp: new Date().toISOString(),
          version: process.env.npm_package_version || '0.8.0',
          subsystems: {
            voice: this._subsystems.voice ? 'enabled' : 'disabled',
            browser: this._subsystems.browser ? 'enabled' : 'disabled',
            memory: this._subsystems.memory ? 'enabled' : 'disabled',
            heartbeat: this._subsystems.heartbeat ? 'enabled' : 'disabled',
          },
        };

        if (this._subsystems.memory) {
          try {
            health.memory = this._subsystems.memory.stats();
          } catch (err) {
            console.warn('[HttpGateway] Memory stats error:', err.message);
          }
        }

        if (this._orchestratorStatus) {
          try {
            health.channels = this._orchestratorStatus();
          } catch (err) {
            console.warn('[HttpGateway] Orchestrator status error:', err.message);
          }
        }

        return sendJson(res, 200, health);
      }

      if (method === 'GET' && pathname === '/ready') {
        const checks = {};
        let ready = true;

        // Database connectivity
        try {
          await this._databaseReadinessCheck(this._dbPath);
          checks.database = 'ok';
        } catch (err) {
          console.debug('[http-gateway] Database readiness check failed:', err.message || err);
          checks.database = 'unavailable';
          ready = false;
        }

        // Memory subsystem
        if (this._subsystems.memory) {
          try {
            this._subsystems.memory.stats();
            checks.memory = 'ok';
          } catch (err) {
            console.warn(
              '[http-gateway] Memory subsystem readiness check failed:',
              err.message || err,
            );
            checks.memory = 'error';
            ready = false;
          }
        }

        // Embedding service
        checks.embeddingService = process.env.OPENAI_API_KEY ? 'configured' : 'not_configured';

        return sendJson(res, ready ? 200 : 503, {
          status: ready ? 'ready' : 'not_ready',
          timestamp: new Date().toISOString(),
          checks,
        });
      }

      // --- Authentication & Permission Check (only when required) ---
      let authResult = null;
      if (requiredLevelName !== 'none') {
        authResult = this._auth.authenticate(req, url);
        if (!authResult.authenticated) {
          // Rate-limit unauthenticated attempts (brute-force guard)
          const ipResult = this._rateLimiter.checkIp(clientIp);
          if (!ipResult.allowed) {
            res.setHeader('Retry-After', Math.ceil(ipResult.retryAfterMs / 1000));
            return sendJson(res, 429, {
              error: 'Too many requests',
              retryAfterMs: ipResult.retryAfterMs,
            });
          }

          return sendJson(res, 401, {
            error: 'Authentication required',
            hint: this._allowQueryParamAuth
              ? 'Provide Authorization: Bearer <key> header or ?api_key=<key> query parameter'
              : 'Provide Authorization: Bearer <key> header',
          });
        }

        // Rate-limit authenticated requests by identity name
        const authRateResult = this._rateLimiter.checkAuth(authResult.identity.name);
        if (!authRateResult.allowed) {
          res.setHeader('Retry-After', Math.ceil(authRateResult.retryAfterMs / 1000));
          return sendJson(res, 429, {
            error: 'Too many requests',
            retryAfterMs: authRateResult.retryAfterMs,
          });
        }

        if (matchedPlugin) {
          const identityLevel = LEVELS[authResult.identity.level] ?? 0;
          const pluginRequiredLevel = LEVELS[requiredLevelName] ?? 0;
          if (identityLevel < pluginRequiredLevel) {
            return sendJson(res, 403, {
              error: 'Forbidden',
              reason: `Route ${method} ${pathname} requires '${requiredLevelName}' permission (your level: '${authResult.identity.level}')`,
            });
          }
        } else {
          const permResult = checkRoutePermission(authResult.identity, pathname, method);
          if (!permResult.allowed) {
            return sendJson(res, 403, { error: 'Forbidden', reason: permResult.reason });
          }
        }
      }

      const sandboxResult = checkSandbox(this._sandbox, pathname);
      if (sandboxResult.blocked) {
        return sendJson(res, 403, {
          error: 'Blocked by sandbox policy',
          reason: sandboxResult.reason,
        });
      }

      // Restrict sensitive admin endpoints to loopback by default
      if (
        !this._allowRemoteAdminEndpoints &&
        (pathname === '/daemon' || pathname === '/remote-access') &&
        !isLoopbackAddress(req.socket.remoteAddress)
      ) {
        return sendJson(res, 403, {
          error: 'Forbidden',
          reason: `Route ${method} ${pathname} is only available from loopback unless allowRemoteAdminEndpoints is enabled`,
        });
      }

      if (method === 'GET' && pathname === '/metrics') {
        const metrics = getMetrics();
        return sendJson(res, 200, metrics.getSummary());
      }

      if (method === 'GET' && pathname === '/agent/queue') {
        const laneId = url.searchParams.get('lane')?.trim() || null;
        try {
          const { getQueueStats } = await import('../claude-harness.js');
          const stats = getQueueStats(laneId);
          if (laneId && !stats) {
            return sendJson(res, 404, { error: `No queue lane found: ${laneId}` });
          }
          return sendJson(res, 200, stats);
        } catch (err) {
          console.debug('[http-gateway] Agent queue stats unavailable:', err.message || err);
          return sendJson(res, 501, { error: 'Agent queue stats are not available' });
        }
      }

      if (method === 'DELETE' && pathname === '/agent/queue') {
        const laneId = url.searchParams.get('lane')?.trim() || null;
        const force =
          url.searchParams.get('force') === '1' || url.searchParams.get('force') === 'true';

        try {
          const { removeQueueLane, clearQueueLanes } = await import('../claude-harness.js');

          if (laneId) {
            const result = removeQueueLane(laneId, { force });
            if (!result.found) {
              return sendJson(res, 404, {
                error: `Queue lane not found: ${laneId}`,
                removed: false,
              });
            }
            if (!result.removed) {
              return sendJson(res, 409, {
                error: `Queue lane is not idle: ${laneId}. Use force=true to remove it.`,
                removed: false,
                lane: result,
              });
            }
            return sendJson(res, 200, {
              removed: true,
              lane: result,
            });
          }

          const result = clearQueueLanes({ force });
          return sendJson(res, 200, {
            removed: true,
            queue: result,
          });
        } catch (error) {
          return sendJson(res, 501, {
            error: 'Agent queue controls are not available',
            message: error?.message,
          });
        }
      }

      if (method === 'GET' && pathname === '/plugins') {
        const plugins = getPluginRegistry().listPlugins();
        return sendJson(res, 200, { plugins });
      }

      if (method === 'GET' && pathname === '/commands') {
        const commands = getCommandRegistry()
          .list()
          .map((cmd) => ({
            name: cmd.name,
            description: cmd.description,
            aliases: cmd.aliases,
            source: cmd.source,
            category: cmd.category,
            acceptsArgs: cmd.acceptsArgs,
          }));
        return sendJson(res, 200, { commands });
      }

      // --- Skills marketplace API ---
      if (method === 'GET' && pathname === '/skills') {
        try {
          const { getSkillRegistry } = await import('../skills/registry.js');
          const skills = getSkillRegistry()
            .list()
            .map((s) => ({
              name: s.name,
              description: s.description,
              category: s.category,
              tags: s.tags,
              origin: s.origin,
            }));
          return sendJson(res, 200, { skills });
        } catch (err) {
          console.debug('[http-gateway] Skill system not available:', err.message || err);
          return sendJson(res, 501, { error: 'Skill system not available' });
        }
      }

      if (method === 'GET' && pathname === '/skills/marketplace') {
        try {
          const { getMarketplaceClient } = await import('../skills/marketplace.js');
          const catalog = getMarketplaceClient().loadLocalCatalog();
          return sendJson(res, 200, catalog);
        } catch (err) {
          console.debug('[http-gateway] Marketplace not available:', err.message || err);
          return sendJson(res, 501, { error: 'Marketplace not available' });
        }
      }

      if (method === 'GET' && pathname === '/skills/categories') {
        try {
          const { getSkillRegistry } = await import('../skills/registry.js');
          const stats = getSkillRegistry().getStats();
          return sendJson(res, 200, { categories: stats.categories });
        } catch (err) {
          console.debug('[http-gateway] Skill categories not available:', err.message || err);
          return sendJson(res, 501, { error: 'Skill system not available' });
        }
      }

      const skillInfoParams = matchRoute('/skills/:name', pathname);
      if (method === 'GET' && skillInfoParams) {
        try {
          const { getSkillRegistry } = await import('../skills/registry.js');
          const skill = getSkillRegistry().get(skillInfoParams.name);
          if (!skill)
            return sendJson(res, 404, { error: `Skill "${skillInfoParams.name}" not found` });
          return sendJson(res, 200, {
            name: skill.name,
            description: skill.description,
            category: skill.category,
            tags: skill.tags,
            origin: skill.origin,
            hasReferences: skill.hasReferences,
            hasScripts: skill.hasScripts,
            sections: skill.parsed.sections,
            mcpTools: skill.parsed.mcpTools,
            cliCommands: skill.parsed.cliCommands,
          });
        } catch (err) {
          console.debug('[http-gateway] Skill info not available:', err.message || err);
          return sendJson(res, 501, { error: 'Skill system not available' });
        }
      }

      // --- Daemon & Remote Access ---
      if (method === 'GET' && pathname === '/daemon') {
        const { execFileSync } = await import('node:child_process');
        const q = (cmd, args = []) => {
          try {
            return execFileSync(cmd, args, { encoding: 'utf-8', timeout: 5000 }).trim();
          } catch (err) {
            console.debug(`[http-gateway] Daemon command '${cmd}' failed:`, err.message || err);
            return null;
          }
        };

        const serviceName = 'stateset-gateway';
        const active = q('systemctl', ['is-active', serviceName]);
        const enabled = q('systemctl', ['is-enabled', serviceName]);

        const daemon = {
          service: serviceName,
          active: active || 'unknown',
          enabled: enabled || 'unknown',
        };

        if (active === 'active') {
          const pid = q('systemctl', ['show', '-p', 'MainPID', '--value', serviceName]);
          if (pid && pid !== '0') {
            daemon.pid = parseInt(pid, 10);
            const uptime = q('ps', ['-p', pid, '-o', 'etime=']);
            if (uptime) daemon.processUptime = uptime.trim();
          }
          const mem = q('systemctl', ['show', '-p', 'MemoryCurrent', '--value', serviceName]);
          if (mem && mem !== '[not set]') {
            daemon.memoryBytes = parseInt(mem, 10);
            daemon.memoryMB = Math.round(parseInt(mem, 10) / 1024 / 1024);
          }
        }

        // Tailscale info
        const tsStatus = q('tailscale', ['status', '--json']);
        if (tsStatus) {
          try {
            const ts = JSON.parse(tsStatus);
            daemon.tailscale = {
              connected: ts.BackendState === 'Running',
              hostname: ts.Self?.HostName,
              tailnet: ts.MagicDNSSuffix,
              ips: ts.Self?.TailscaleIPs || [],
              url:
                ts.Self?.HostName && ts.MagicDNSSuffix
                  ? `https://${ts.Self.HostName}.${ts.MagicDNSSuffix}`
                  : null,
            };
          } catch (err) {
            console.warn('[HttpGateway] Tailscale status parse error:', err.message);
            daemon.tailscale = { connected: false };
          }
        }

        // SSH tunnel count
        const tunnelProcs = q('pgrep', ['-c', '-f', 'ssh.*-[LR].*127.0.0.1']);
        daemon.sshTunnels = { activeProcesses: parseInt(tunnelProcs || '0', 10) };

        return sendJson(res, 200, daemon);
      }

      if (method === 'GET' && pathname === '/remote-access') {
        const { execFileSync } = await import('node:child_process');
        const q = (cmd, args = []) => {
          try {
            return execFileSync(cmd, args, { encoding: 'utf-8', timeout: 5000 }).trim();
          } catch (err) {
            console.debug(
              `[http-gateway] Remote-access command '${cmd}' failed:`,
              err.message || err,
            );
            return null;
          }
        };

        const access = { urls: [], tailscale: null, sshTunnels: [] };

        // Local URL
        const addr = this.getAddress();
        if (addr) {
          access.urls.push({
            type: 'local',
            url: `http://${addr.host}:${addr.port}`,
            description: 'Local HTTP gateway',
          });
        }

        // Tailscale
        const tsStatus = q('tailscale', ['status', '--json']);
        if (tsStatus) {
          try {
            const ts = JSON.parse(tsStatus);
            const hostname = ts.Self?.HostName;
            const tailnet = ts.MagicDNSSuffix;
            if (hostname && tailnet) {
              const tsUrl = `https://${hostname}.${tailnet}`;
              access.tailscale = {
                connected: true,
                hostname,
                tailnet,
                url: tsUrl,
                ips: ts.Self?.TailscaleIPs || [],
              };
              access.urls.push({
                type: 'tailscale',
                url: tsUrl,
                description: 'Tailscale HTTPS (tailnet only)',
              });

              const funnelStatus = q('tailscale', ['funnel', 'status', '--json']);
              if (funnelStatus) {
                try {
                  const funnel = JSON.parse(funnelStatus);
                  if (Object.keys(funnel).length > 0) {
                    access.tailscale.funnelActive = true;
                    access.urls.push({
                      type: 'funnel',
                      url: tsUrl,
                      description: 'Tailscale Funnel (public internet)',
                    });
                  }
                } catch (err) {
                  console.warn('[HttpGateway] Funnel status parse error:', err.message);
                }
              }
            }
          } catch (err) {
            console.warn('[HttpGateway] Tailscale remote-access error:', err.message);
          }
        }

        // SSH tunnels
        const tunnelServices = q('systemctl', [
          'list-units',
          '--type=service',
          '--all',
          'stateset-ssh-tunnel@*',
          '--no-legend',
          '--plain',
        ]);
        if (tunnelServices) {
          for (const line of tunnelServices.split('\n').filter(Boolean)) {
            const parts = line.trim().split(/\s+/);
            access.sshTunnels.push({
              service: parts[0],
              active: parts[2],
              state: parts[3],
            });
          }
        }

        return sendJson(res, 200, access);
      }

      // Plugin enable/disable
      const enableParams = matchRoute('/plugins/:id/enable', pathname);
      if (method === 'POST' && enableParams) {
        if (!this._configState) {
          return sendJson(res, 501, { error: 'Plugin config state not available' });
        }
        this._configState.enable(enableParams.id);
        return sendJson(res, 200, { id: enableParams.id, enabled: true });
      }

      const disableParams = matchRoute('/plugins/:id/disable', pathname);
      if (method === 'POST' && disableParams) {
        if (!this._configState) {
          return sendJson(res, 501, { error: 'Plugin config state not available' });
        }
        this._configState.disable(disableParams.id);
        return sendJson(res, 200, { id: disableParams.id, enabled: false });
      }

      // =================================================================
      // Voice routes (5 endpoints)
      // =================================================================

      if (method === 'GET' && pathname === '/voice/status') {
        if (!this._subsystems.voice) {
          return sendJson(res, 501, { error: 'Voice subsystem not enabled' });
        }
        const status = await this._subsystems.voice.getVoiceStatus();
        return sendJson(res, 200, status);
      }

      if (method === 'POST' && pathname === '/voice/transcribe') {
        if (!this._subsystems.voice) {
          return sendJson(res, 501, { error: 'Voice subsystem not enabled' });
        }
        const audioBuffer = await parseRawBody(req);
        const format = url.searchParams.get('format') || 'mp3';
        const result = await this._subsystems.voice.processVoiceMessage(audioBuffer, '_http', {
          format,
          skipTTS: true,
          agentHandler: async (text) => text,
        });
        return sendJson(res, 200, {
          text: result.transcription.text,
          language: result.transcription.language,
          duration: result.transcription.duration,
        });
      }

      if (method === 'POST' && pathname === '/voice/synthesize') {
        if (!this._subsystems.voice) {
          return sendJson(res, 501, { error: 'Voice subsystem not enabled' });
        }
        const body = await parseBody(req);
        if (!body.text) return sendJson(res, 400, { error: 'Missing required field: text' });
        // Use processVoiceMessage with a dummy transcription path to get TTS output
        const result = await this._subsystems.voice.processVoiceMessage(
          Buffer.alloc(0),
          '_http_tts',
          {
            format: 'mp3',
            skipTTS: false,
            voiceId: body.voiceId,
            agentHandler: async () => body.text,
          },
        );
        if (result.audioResponse) {
          return sendBinary(res, 200, result.audioResponse, 'audio/mpeg');
        }
        return sendJson(res, 500, { error: 'TTS synthesis failed' });
      }

      const voiceEnableParams = matchRoute('/voice/session/enable/:sessionId', pathname);
      if (method === 'POST' && voiceEnableParams) {
        if (!this._subsystems.voice) {
          return sendJson(res, 501, { error: 'Voice subsystem not enabled' });
        }
        const result = this._subsystems.voice.enableVoiceMode(voiceEnableParams.sessionId);
        return sendJson(res, 200, result);
      }

      const voiceDisableParams = matchRoute('/voice/session/disable/:sessionId', pathname);
      if (method === 'POST' && voiceDisableParams) {
        if (!this._subsystems.voice) {
          return sendJson(res, 501, { error: 'Voice subsystem not enabled' });
        }
        const result = this._subsystems.voice.disableVoiceMode(voiceDisableParams.sessionId);
        return sendJson(res, 200, result);
      }

      // =================================================================
      // Browser routes (9 endpoints)
      // =================================================================

      if (method === 'GET' && pathname === '/browser/status') {
        if (!this._subsystems.browser) {
          return sendJson(res, 501, { error: 'Browser subsystem not enabled' });
        }
        return sendJson(res, 200, { connected: true });
      }

      if (method === 'POST' && pathname === '/browser/navigate') {
        if (!this._subsystems.browser) {
          return sendJson(res, 501, { error: 'Browser subsystem not enabled' });
        }
        const body = await parseBody(req);
        const targetUrl = typeof body.url === 'string' ? body.url.trim() : body.url;
        const error = validateBrowserUrl(targetUrl);
        if (error) return sendJson(res, 400, { error });
        await this._subsystems.browser.navigate(targetUrl);
        return sendJson(res, 200, { ok: true, url: targetUrl });
      }

      if (method === 'POST' && pathname === '/browser/screenshot') {
        if (!this._subsystems.browser) {
          return sendJson(res, 501, { error: 'Browser subsystem not enabled' });
        }
        const buf = await this._subsystems.browser.screenshot();
        const base64 = buf.toString('base64');
        return sendJson(res, 200, { image: base64, format: 'png' });
      }

      if (method === 'POST' && pathname === '/browser/evaluate') {
        if (!this._subsystems.browser) {
          return sendJson(res, 501, { error: 'Browser subsystem not enabled' });
        }
        if (
          !authResult ||
          !authResult.identity ||
          LEVELS[authResult.identity.level] < LEVELS.admin
        ) {
          return sendJson(res, 403, {
            error: 'Forbidden',
            reason: 'Route /browser/evaluate requires admin permission',
          });
        }
        if (!this._allowBrowserEvaluate) {
          return sendJson(res, 403, {
            error: 'Forbidden',
            reason:
              'Route /browser/evaluate is disabled by default. Set httpGateway.allowBrowserEvaluate=true to enable read-only evaluation.',
          });
        }
        const body = await parseBody(req);
        const validation = validateBrowserExpression(body.expression);
        if (validation) return sendJson(res, 400, { error: validation });
        const expression = body.expression.trim();
        const result = await this._subsystems.browser.evaluate(expression);
        return sendJson(res, 200, { result });
      }

      if (method === 'POST' && pathname === '/browser/click') {
        if (!this._subsystems.browser) {
          return sendJson(res, 501, { error: 'Browser subsystem not enabled' });
        }
        const body = await parseBody(req);
        if (!body.selector)
          return sendJson(res, 400, { error: 'Missing required field: selector' });
        await this._subsystems.browser.click(body.selector);
        return sendJson(res, 200, { ok: true });
      }

      if (method === 'POST' && pathname === '/browser/type') {
        if (!this._subsystems.browser) {
          return sendJson(res, 501, { error: 'Browser subsystem not enabled' });
        }
        const body = await parseBody(req);
        if (!body.selector || !body.text)
          return sendJson(res, 400, { error: 'Missing required fields: selector, text' });
        await this._subsystems.browser.type(body.selector, body.text);
        return sendJson(res, 200, { ok: true });
      }

      if (method === 'GET' && pathname === '/browser/content') {
        if (!this._subsystems.browser) {
          return sendJson(res, 501, { error: 'Browser subsystem not enabled' });
        }
        const format = url.searchParams.get('format') || 'text';
        const content =
          format === 'html'
            ? await this._subsystems.browser.getPageHTML()
            : await this._subsystems.browser.getPageContent();
        return sendJson(res, 200, { content, format });
      }

      if (method === 'GET' && pathname === '/browser/links') {
        if (!this._subsystems.browser) {
          return sendJson(res, 501, { error: 'Browser subsystem not enabled' });
        }
        const links = await this._subsystems.browser.extractLinks();
        return sendJson(res, 200, { links });
      }

      if (method === 'POST' && pathname === '/browser/close') {
        if (!this._subsystems.browser) {
          return sendJson(res, 501, { error: 'Browser subsystem not enabled' });
        }
        await this._subsystems.browser.close();
        return sendJson(res, 200, { ok: true });
      }

      // =================================================================
      // Memory routes (8 endpoints)
      // =================================================================

      if (method === 'GET' && pathname === '/memory/stats') {
        if (!this._subsystems.memory) {
          return sendJson(res, 501, { error: 'Memory subsystem not enabled' });
        }
        const stats = this._subsystems.memory.stats();
        return sendJson(res, 200, stats);
      }

      if (method === 'POST' && pathname === '/memory/save') {
        if (!this._subsystems.memory) {
          return sendJson(res, 501, { error: 'Memory subsystem not enabled' });
        }
        const body = await parseBody(req);
        if (!body.summary) return sendJson(res, 400, { error: 'Missing required field: summary' });
        const result = this._subsystems.memory.save({
          summary: body.summary,
          facts: body.facts,
          channel: body.channel || 'http',
          senderId: body.senderId || 'api',
        });
        return sendJson(res, 200, result);
      }

      if (method === 'POST' && pathname === '/memory/search') {
        if (!this._subsystems.memory) {
          return sendJson(res, 501, { error: 'Memory subsystem not enabled' });
        }
        const body = await parseBody(req);
        if (!body.query) return sendJson(res, 400, { error: 'Missing required field: query' });
        const results = this._subsystems.memory.base.search(
          body.channel || 'http',
          body.senderId || 'api',
          body.query,
          body.limit || 10,
        );
        return sendJson(res, 200, { results });
      }

      if (method === 'POST' && pathname === '/memory/vector-search') {
        if (!this._subsystems.memory) {
          return sendJson(res, 501, { error: 'Memory subsystem not enabled' });
        }
        const body = await parseBody(req);
        if (!body.query) return sendJson(res, 400, { error: 'Missing required field: query' });
        const results = this._subsystems.memory.vectorSearch(body.query, {
          limit: body.limit || 5,
          channel: body.channel,
          senderId: body.senderId,
        });
        return sendJson(res, 200, { results });
      }

      if (method === 'POST' && pathname === '/memory/hybrid-search') {
        if (!this._subsystems.memory) {
          return sendJson(res, 501, { error: 'Memory subsystem not enabled' });
        }
        const body = await parseBody(req);
        if (!body.query) return sendJson(res, 400, { error: 'Missing required field: query' });
        const results = this._subsystems.memory.hybridSearch(body.query, {
          limit: body.limit || 5,
          channel: body.channel,
          senderId: body.senderId,
        });
        return sendJson(res, 200, { results });
      }

      const recentMemoryParams = matchRoute('/memory/recent/:channel/:senderId', pathname);
      if (method === 'GET' && recentMemoryParams) {
        if (!this._subsystems.memory) {
          return sendJson(res, 501, { error: 'Memory subsystem not enabled' });
        }
        const limit = parseInt(url.searchParams.get('limit') || '10', 10);
        const results = this._subsystems.memory.base.search(
          recentMemoryParams.channel,
          recentMemoryParams.senderId,
          '',
          limit,
        );
        return sendJson(res, 200, { results });
      }

      if (method === 'POST' && pathname === '/memory/backfill') {
        if (!this._subsystems.memory) {
          return sendJson(res, 501, { error: 'Memory subsystem not enabled' });
        }
        const body = await parseBody(req);
        const result = this._subsystems.memory.backfill(body.channel, body.senderId);
        return sendJson(res, 200, result);
      }

      const memoryDeleteParams = matchRoute('/memory/:id', pathname);
      if (method === 'DELETE' && memoryDeleteParams) {
        if (!this._subsystems.memory) {
          return sendJson(res, 501, { error: 'Memory subsystem not enabled' });
        }
        const id = parseInt(memoryDeleteParams.id, 10);
        this._subsystems.memory.deleteVector(id);
        this._subsystems.memory.base.delete(id);
        return sendJson(res, 200, { ok: true, deleted: id });
      }

      // =================================================================
      // Heartbeat routes (5 endpoints)
      // =================================================================

      if (method === 'GET' && pathname === '/heartbeat/status') {
        if (!this._subsystems.heartbeat) {
          return sendJson(res, 501, { error: 'Heartbeat subsystem not enabled' });
        }
        return sendJson(res, 200, this._subsystems.heartbeat.getStatus());
      }

      if (method === 'GET' && pathname === '/heartbeat/checks') {
        if (!this._subsystems.heartbeat) {
          return sendJson(res, 501, { error: 'Heartbeat subsystem not enabled' });
        }
        return sendJson(res, 200, { checks: this._subsystems.heartbeat.listChecks() });
      }

      const heartbeatRunParams = matchRoute('/heartbeat/checks/:id/run', pathname);
      if (method === 'POST' && heartbeatRunParams) {
        if (!this._subsystems.heartbeat) {
          return sendJson(res, 501, { error: 'Heartbeat subsystem not enabled' });
        }
        try {
          const result = await this._subsystems.heartbeat.runCheck(heartbeatRunParams.id);
          if (result === null) {
            return sendJson(res, 404, { error: `Check '${heartbeatRunParams.id}' not found` });
          }
          return sendJson(res, 200, { checkId: heartbeatRunParams.id, ...result });
        } catch (err) {
          return sendJson(res, 500, { error: err.message });
        }
      }

      const heartbeatEnableParams = matchRoute('/heartbeat/checks/:id/enable', pathname);
      if (method === 'POST' && heartbeatEnableParams) {
        if (!this._subsystems.heartbeat) {
          return sendJson(res, 501, { error: 'Heartbeat subsystem not enabled' });
        }
        try {
          this._subsystems.heartbeat.enableCheck(heartbeatEnableParams.id);
          return sendJson(res, 200, { checkId: heartbeatEnableParams.id, enabled: true });
        } catch (err) {
          return sendJson(res, 404, { error: err.message });
        }
      }

      const heartbeatDisableParams = matchRoute('/heartbeat/checks/:id/disable', pathname);
      if (method === 'POST' && heartbeatDisableParams) {
        if (!this._subsystems.heartbeat) {
          return sendJson(res, 501, { error: 'Heartbeat subsystem not enabled' });
        }
        try {
          this._subsystems.heartbeat.disableCheck(heartbeatDisableParams.id);
          return sendJson(res, 200, { checkId: heartbeatDisableParams.id, enabled: false });
        } catch (err) {
          return sendJson(res, 404, { error: err.message });
        }
      }

      // --- Plugin-registered routes ---
      if (matchedPlugin) {
        const body =
          method === 'POST' || method === 'PUT' || method === 'PATCH' ? await parseBody(req) : {};

        const result = await matchedPlugin.handler({
          method,
          pathname,
          params: matchedPluginParams,
          body,
          query: Object.fromEntries(url.searchParams),
          headers: req.headers,
          identity: authResult?.identity || null,
        });

        if (result && typeof result === 'object') {
          // Support _html flag for webchat HTML responses
          if (result._html) {
            return sendHtml(res, result.status || 200, result._html);
          }
          return sendJson(res, result.status || 200, result.body || result);
        }
        return sendJson(res, 200, { ok: true });
      }

      // 404
      sendJson(res, 404, { error: 'Not found', path: pathname });
    } catch (err) {
      console.error(`[HttpGateway] Error handling ${method} ${pathname}:`, err.message);
      sendJson(res, 500, { error: err.message });
    }
  }
}

// ============================================================================
// Factory
// ============================================================================

/**
 * Create an HTTP gateway instance.
 * @param {Object} opts
 * @returns {HttpGateway}
 */
export function createHttpGateway(opts) {
  return new HttpGateway(opts);
}
