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
import { getPluginRegistry } from './plugin-api.js';
import { getCommandRegistry } from './command-registry.js';
import { getMetrics } from './metrics.js';

// ============================================================================
// Helpers
// ============================================================================

/**
 * Parse JSON body from an incoming request.
 * @param {http.IncomingMessage} req
 * @returns {Promise<Object>}
 */
function parseBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on('data', (chunk) => chunks.push(chunk));
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
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type',
  });
  res.end(body);
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

// ============================================================================
// HttpGateway
// ============================================================================

const startTime = Date.now();

export class HttpGateway {
  /**
   * @param {Object} opts
   * @param {number} [opts.port=0] - Port (0 = random)
   * @param {string} [opts.host='127.0.0.1']
   * @param {boolean} [opts.verbose=false]
   * @param {import('./plugin-config.js').PluginConfigState} [opts.configState]
   */
  constructor(opts = {}) {
    this._port = opts.port || 0;
    this._host = opts.host || '127.0.0.1';
    this._verbose = opts.verbose || false;
    this._configState = opts.configState || null;
    this._server = null;
    this._address = null;
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
          console.log(`[HttpGateway] Listening on ${this._address.address}:${this._address.port}`);
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
          console.log('[HttpGateway] Stopped.');
        }
        this._server = null;
        this._address = null;
        resolve();
      });
    });
  }

  /**
   * Get the server address.
   * @returns {{ host: string, port: number }|null}
   */
  getAddress() {
    return this._address
      ? { host: this._address.address, port: this._address.port }
      : null;
  }

  // ============================================================================
  // Request Handler
  // ============================================================================

  /**
   * @private
   */
  async _handleRequest(req, res) {
    const url = new URL(req.url, `http://${req.headers.host || 'localhost'}`);
    const pathname = url.pathname;
    const method = req.method.toUpperCase();

    // CORS preflight
    if (method === 'OPTIONS') {
      sendJson(res, 204, {});
      return;
    }

    if (this._verbose) {
      console.log(`[HttpGateway] ${method} ${pathname}`);
    }

    try {
      // --- Built-in routes ---

      if (method === 'GET' && pathname === '/health') {
        return sendJson(res, 200, {
          status: 'ok',
          uptime: Date.now() - startTime,
          timestamp: new Date().toISOString(),
        });
      }

      if (method === 'GET' && pathname === '/metrics') {
        const metrics = getMetrics();
        return sendJson(res, 200, metrics.snapshot());
      }

      if (method === 'GET' && pathname === '/plugins') {
        const plugins = getPluginRegistry().listPlugins();
        return sendJson(res, 200, { plugins });
      }

      if (method === 'GET' && pathname === '/commands') {
        const commands = getCommandRegistry().list().map((cmd) => ({
          name: cmd.name,
          description: cmd.description,
          aliases: cmd.aliases,
          source: cmd.source,
          category: cmd.category,
          acceptsArgs: cmd.acceptsArgs,
        }));
        return sendJson(res, 200, { commands });
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

      // --- Plugin-registered routes ---
      const pluginRoutes = getPluginRegistry().getRoutes();
      for (const route of pluginRoutes) {
        if (method !== route.method.toUpperCase()) continue;

        const params = matchRoute(route.path, pathname);
        if (params) {
          const body = (method === 'POST' || method === 'PUT' || method === 'PATCH')
            ? await parseBody(req)
            : {};

          const result = await route.handler({
            method,
            pathname,
            params,
            body,
            query: Object.fromEntries(url.searchParams),
            headers: req.headers,
          });

          if (result && typeof result === 'object') {
            return sendJson(res, result.status || 200, result.body || result);
          }
          return sendJson(res, 200, { ok: true });
        }
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
