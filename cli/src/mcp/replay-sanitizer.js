// Replay-log sanitization for the MCP agentic tool surface.
//
// The agentic runtime records each tool invocation to disk so that runs are
// replayable and auditable. We can't write raw arguments — they'd leak secrets
// (API keys, passwords, signatures) and unbounded payload sizes. This module
// is the boundary that:
//
//   1. Redacts well-known sensitive keys (api_key, password, secret, token, …).
//   2. Bounds breadth and depth (caps array length, object key count, string
//      length, recursion depth) so a single huge payload can't blow up the log.
//   3. Stably serializes objects so two semantically-equal payloads produce
//      identical replay rows (used for hashing / dedup).
//
// Extracted from `cli/src/mcp-server.js` to keep that file focused on
// orchestration. Imported back from there.

import { createHash } from 'node:crypto';

/**
 * Stably stringify a value: object keys are sorted at every level so the same
 * logical object always produces the same string. Used as the hash input.
 */
export const stableStringify = (value) => {
  const normalize = (input) => {
    if (input === null || input === undefined) return input;
    if (Array.isArray(input)) {
      return input.map((item) => normalize(item));
    }
    if (typeof input !== 'object') return input;
    const sorted = Object.keys(input)
      .sort()
      .reduce((acc, key) => {
        acc[key] = normalize(input[key]);
        return acc;
      }, {});
    return sorted;
  };

  return JSON.stringify(normalize(value));
};

/** SHA-256 hex digest of a value's String() form. */
export const sha256 = (value) => createHash('sha256').update(String(value)).digest('hex');

/**
 * Keys whose values are unconditionally replaced with "[REDACTED]".
 * Lowercased substring "secret" anywhere in a key name is also redacted.
 */
export const REDACT_REPLAY_KEYS = new Set([
  'api_key',
  'apiKey',
  'apikey',
  'auth',
  'authorization',
  'credential',
  'credentials',
  'password',
  'private',
  'private_key',
  'privateKey',
  'secret',
  'secret_key',
  'secretKey',
  'seed',
  'signature',
  'token',
  'wallet_private_key',
]);

/** Hard caps on replay-row size. */
export const MAX_REPLAY_ARRAY_ITEMS = 25;
export const MAX_REPLAY_OBJECT_KEYS = 80;
export const MAX_REPLAY_STRING_CHARS = 240;

/**
 * Recursively sanitize a value for the replay log.
 *
 * Handles strings (truncates), Date/Map/Set/Buffer (typed summaries), arrays
 * (delegates to compactReplayValue), objects (caps key count, redacts
 * sensitive keys, recurses with reduced depth), and detects cycles.
 */
export const sanitizeReplayValue = (value, depth = 4, seen = new Set()) => {
  if (value === null || value === undefined) return value;
  if (typeof value === 'string') {
    if (value.length <= MAX_REPLAY_STRING_CHARS) return value;
    return `${value.slice(0, MAX_REPLAY_STRING_CHARS)}...`;
  }
  if (typeof value === 'number' || typeof value === 'boolean') return value;
  if (typeof value === 'bigint') return `${value.toString()}n`;
  if (typeof value === 'symbol' || typeof value === 'function') return String(value);
  if (value instanceof Date) return value.toISOString();
  if (value instanceof Map)
    return {
      _type: 'Map',
      size: value.size,
      entries: Array.from(value.entries()).map(([k, v]) => [
        sanitizeReplayValue(k, depth - 1, seen),
        sanitizeReplayValue(v, depth - 1, seen),
      ]),
    };
  if (value instanceof Set)
    return {
      _type: 'Set',
      size: value.size,
      values: Array.from(value.values()).map((entry) =>
        sanitizeReplayValue(entry, depth - 1, seen),
      ),
    };
  if (Buffer.isBuffer(value)) return `<Buffer ${value.length}>`;
  if (Array.isArray(value)) return compactReplayValue(value, depth, seen);

  if (typeof value !== 'object') return String(value);
  if (depth <= 0 || seen.has(value)) return '[truncated]';
  seen.add(value);

  const output = {};
  const keys = Object.keys(value);
  const keysToCopy = keys.slice(0, MAX_REPLAY_OBJECT_KEYS);
  for (const key of keysToCopy) {
    if (REDACT_REPLAY_KEYS.has(key) || key.toLowerCase().includes('secret')) {
      output[key] = '[REDACTED]';
      continue;
    }
    output[key] = sanitizeReplayValue(value[key], depth - 1, seen);
  }
  if (keys.length > MAX_REPLAY_OBJECT_KEYS) {
    output.__truncatedKeys = keys.length - MAX_REPLAY_OBJECT_KEYS;
  }
  return output;
};

/**
 * Compact a value (typically an array) for replay: truncates to
 * MAX_REPLAY_ARRAY_ITEMS, recursing via sanitizeReplayValue for non-arrays.
 */
export const compactReplayValue = (value, depth = 4, seen = new Set()) => {
  if (value === null || value === undefined) return value;
  if (Array.isArray(value)) {
    if (depth <= 0 || seen.has(value)) return '[truncated]';
    seen.add(value);
    const values = value
      .slice(0, MAX_REPLAY_ARRAY_ITEMS)
      .map((entry) => compactReplayValue(entry, depth - 1, seen));
    if (value.length > MAX_REPLAY_ARRAY_ITEMS) {
      values.push(`[+${value.length - MAX_REPLAY_ARRAY_ITEMS} more items]`);
    }
    return values;
  }
  return sanitizeReplayValue(value, depth, seen);
};
