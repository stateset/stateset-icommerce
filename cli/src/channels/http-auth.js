/**
 * HTTP Authentication & Route Permissions for StateSet HTTP Gateway
 *
 * Provides API key authentication, per-route permission enforcement,
 * and sandbox mode to block dangerous routes.
 *
 * Uses Node.js built-in crypto for timing-safe key comparison.
 * No external dependencies.
 */

import { timingSafeEqual } from 'node:crypto';

// ============================================================================
// Permission Levels (aligned with src/permissions.js)
// ============================================================================

export const LEVELS = {
  none: 0,
  read: 1,
  preview: 2,
  write: 3,
  delete: 4,
  admin: 5,
};

// ============================================================================
// Route Permission Map
// ============================================================================

/**
 * Map of route prefixes to required permission levels.
 * Uses longest-prefix matching. Optional `methods` overrides per HTTP method.
 *
 * @type {Object<string, { level: string, methods?: Object<string, string> }>}
 */
export const ROUTE_PERMISSIONS = {
  // Public
  '/health':         { level: 'none' },

  // Read-only
  '/metrics':        { level: 'read' },
  '/commands':       { level: 'read' },
  '/skills':         { level: 'read' },

  // Plugin management
  '/plugins':        { level: 'read', methods: { POST: 'admin' } },

  // Admin-only
  '/daemon':         { level: 'admin' },
  '/remote-access':  { level: 'admin' },

  // Voice
  '/voice':          { level: 'read', methods: { POST: 'write' } },

  // Browser (write by default, GET overridden to read)
  '/browser':        { level: 'write', methods: { GET: 'read' } },

  // Memory
  '/memory':         { level: 'read', methods: { POST: 'write', DELETE: 'delete' } },

  // Heartbeat
  '/heartbeat':      { level: 'read', methods: { POST: 'write' } },
};

// ============================================================================
// Sandbox Blocked Routes
// ============================================================================

/**
 * Routes blocked when sandbox restrictions are active.
 * @type {Object<string, string[]>}
 */
export const SANDBOX_BLOCKED_ROUTES = {
  browser: [
    '/browser/evaluate',
    '/browser/navigate',
    '/browser/click',
    '/browser/type',
    '/browser/close',
  ],
  shell: [
    '/daemon',
  ],
};

// ============================================================================
// API Key Authentication
// ============================================================================

/**
 * Create an API key authenticator.
 *
 * When apiKeys is empty, authentication is disabled and all requests
 * are treated as admin (backwards compatible).
 *
 * @param {Array<{ key: string, name: string, level?: string, channels?: string[] }>} apiKeys
 * @returns {{ authenticate: (req: import('http').IncomingMessage, url: URL) => { authenticated: boolean, identity?: Object } }}
 */
export function createApiKeyAuth(apiKeys = []) {
  const keyEntries = apiKeys
    .filter((e) => e.key && e.key.length > 0)
    .map((entry) => ({
      keyBuffer: Buffer.from(entry.key, 'utf-8'),
      name: entry.name || 'unnamed',
      level: entry.level || 'read',
      channels: entry.channels || null,
    }));

  function authenticate(req, url) {
    // No keys configured = auth disabled (backwards compatible)
    if (keyEntries.length === 0) {
      return { authenticated: true, identity: { name: 'anonymous', level: 'admin' } };
    }

    // Extract token from Bearer header or query param
    let token = null;
    const authHeader = req.headers['authorization'];
    if (authHeader && authHeader.startsWith('Bearer ')) {
      token = authHeader.slice(7);
    }
    if (!token) {
      token = url.searchParams.get('api_key');
    }

    if (!token) {
      return { authenticated: false };
    }

    const tokenBuffer = Buffer.from(token, 'utf-8');

    for (const entry of keyEntries) {
      if (tokenBuffer.length === entry.keyBuffer.length &&
          timingSafeEqual(tokenBuffer, entry.keyBuffer)) {
        return {
          authenticated: true,
          identity: {
            name: entry.name,
            level: entry.level,
            channels: entry.channels,
          },
        };
      }
    }

    return { authenticated: false };
  }

  return { authenticate };
}

// ============================================================================
// Route Permission Checker
// ============================================================================

/**
 * Check if an identity has permission for a route.
 *
 * @param {{ name: string, level: string }} identity
 * @param {string} pathname
 * @param {string} method - HTTP method (GET, POST, DELETE, etc.)
 * @returns {{ allowed: boolean, reason?: string }}
 */
export function checkRoutePermission(identity, pathname, method) {
  const identityLevel = LEVELS[identity.level] ?? 0;

  // Find best matching route permission by longest prefix
  let matchedPermission = null;
  let matchedLength = 0;

  for (const [prefix, perm] of Object.entries(ROUTE_PERMISSIONS)) {
    if (pathname === prefix || pathname.startsWith(prefix + '/')) {
      if (prefix.length > matchedLength) {
        matchedPermission = perm;
        matchedLength = prefix.length;
      }
    }
  }

  // No matching route = default to read
  if (!matchedPermission) {
    matchedPermission = { level: 'read' };
  }

  // Method-specific override or base level
  let requiredLevelName = matchedPermission.level;
  if (matchedPermission.methods && matchedPermission.methods[method]) {
    requiredLevelName = matchedPermission.methods[method];
  }

  const requiredLevel = LEVELS[requiredLevelName] ?? 0;

  if (identityLevel < requiredLevel) {
    return {
      allowed: false,
      reason: `Route ${method} ${pathname} requires '${requiredLevelName}' permission (your level: '${identity.level}')`,
    };
  }

  return { allowed: true };
}

// ============================================================================
// Sandbox Checker
// ============================================================================

/**
 * Check if a route is blocked by sandbox restrictions.
 *
 * @param {{ browser?: boolean, shell?: boolean }} sandbox
 * @param {string} pathname
 * @returns {{ blocked: boolean, reason?: string }}
 */
export function checkSandbox(sandbox, pathname) {
  if (!sandbox) return { blocked: false };

  if (sandbox.browser) {
    for (const blocked of SANDBOX_BLOCKED_ROUTES.browser) {
      if (pathname === blocked || pathname.startsWith(blocked + '/')) {
        return { blocked: true, reason: `Route '${pathname}' is blocked by browser sandbox` };
      }
    }
  }

  if (sandbox.shell) {
    for (const blocked of SANDBOX_BLOCKED_ROUTES.shell) {
      if (pathname === blocked || pathname.startsWith(blocked + '/')) {
        return { blocked: true, reason: `Route '${pathname}' is blocked by shell sandbox` };
      }
    }
  }

  return { blocked: false };
}
