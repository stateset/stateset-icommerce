/**
 * API-key authentication for `stateset-mcp-http`.
 *
 * Thin layer over the gateway authenticator in `channels/http-auth.js`, which
 * owns the constant-time comparison and Bearer parsing. This module adds:
 *   - key collection from `--api-key` flags, `STATESET_MCP_API_KEYS` (comma
 *     separated) and `--api-key-file` (one key per line, `#` comments allowed);
 *   - `X-API-Key` as an alternative to `Authorization: Bearer`;
 *   - a request guard that answers 401 with a JSON-RPC-shaped body and a
 *     `WWW-Authenticate: Bearer` challenge;
 *   - short fingerprints so keys can be named in logs without being revealed.
 */

import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { createApiKeyAuth } from '../channels/http-auth.js';

/** Environment variable holding comma-separated keys. */
export const API_KEYS_ENV = 'STATESET_MCP_API_KEYS';

/**
 * Short, log-safe identifier for a key: the first 6 hex chars of its SHA-256.
 * Never log the key itself.
 * @param {string} key
 * @returns {string}
 */
export function keyFingerprint(key) {
  return createHash('sha256').update(key, 'utf-8').digest('hex').slice(0, 6);
}

/**
 * Split a comma- or newline-separated key list, dropping blanks and `#` comments.
 * @param {string} text
 * @returns {string[]}
 */
function splitKeys(text) {
  return text
    .split(/[\n,]/)
    .map((k) => k.trim())
    .filter((k) => k.length > 0 && !k.startsWith('#'));
}

/**
 * Collect API keys from every configured source, de-duplicated, in order.
 *
 * @param {object} [sources]
 * @param {string[]} [sources.flags]      values of repeatable `--api-key`
 * @param {string}   [sources.env]        raw `STATESET_MCP_API_KEYS` value
 * @param {string}   [sources.file]       path passed to `--api-key-file`
 * @param {(path: string) => string} [sources.readFile]  injectable for tests
 * @returns {string[]}
 */
export function collectApiKeys(sources = {}) {
  const keys = [];
  for (const flag of sources.flags ?? []) keys.push(...splitKeys(flag));
  if (sources.env) keys.push(...splitKeys(sources.env));
  if (sources.file) {
    const read = sources.readFile ?? ((p) => readFileSync(p, 'utf-8'));
    let text;
    try {
      text = read(sources.file);
    } catch (error) {
      throw new Error(`--api-key-file ${sources.file}: ${error.message}`);
    }
    keys.push(...splitKeys(text));
  }
  for (const key of keys) {
    if (key.length < 16) {
      throw new Error(
        `API key ${keyFingerprint(key)} is too short (${key.length} chars); use at least 16`,
      );
    }
  }
  return [...new Set(keys)];
}

/**
 * Pull the presented credential off a request: `Authorization: Bearer <key>`
 * first, then `X-API-Key`. Returns null when neither is present.
 * @param {import('node:http').IncomingMessage} req
 * @returns {string|null}
 */
export function extractPresentedKey(req) {
  const auth = req.headers['authorization'];
  if (typeof auth === 'string' && /^Bearer\s+\S/i.test(auth)) {
    return auth.replace(/^Bearer\s+/i, '').trim();
  }
  const apiKey = req.headers['x-api-key'];
  if (typeof apiKey === 'string' && apiKey.trim()) return apiKey.trim();
  return null;
}

/**
 * Build the `/mcp` auth guard. With no keys the guard is `null` (auth off).
 *
 * The returned function follows the SDK guard contract: it returns `true` to
 * let the request through, or writes the 401 response and returns `false`.
 *
 * @param {string[]} keys
 * @returns {null | ((req: import('node:http').IncomingMessage, res: import('node:http').ServerResponse) => boolean)}
 */
export function createApiKeyGuard(keys) {
  if (!keys || keys.length === 0) return null;
  const { authenticate } = createApiKeyAuth(
    keys.map((key) => ({ key, name: keyFingerprint(key) })),
  );

  return function apiKeyGuard(req, res) {
    const presented = extractPresentedKey(req);
    // Route both header forms through the gateway's constant-time path.
    const result = presented
      ? authenticate({ headers: { authorization: `Bearer ${presented}` } }, null)
      : { authenticated: false };
    if (result.authenticated) return true;

    res.writeHead(401, {
      'Content-Type': 'application/json',
      'WWW-Authenticate': 'Bearer realm="stateset-mcp", error="invalid_token"',
    });
    res.end(
      JSON.stringify({
        jsonrpc: '2.0',
        id: null,
        error: {
          code: -32001,
          message: presented
            ? 'Unauthorized: invalid API key'
            : 'Unauthorized: missing API key (Authorization: Bearer <key> or X-API-Key)',
        },
      }),
    );
    return false;
  };
}
