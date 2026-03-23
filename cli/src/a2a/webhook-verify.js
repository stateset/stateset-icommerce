/**
 * A2A Webhook Signature Verification SDK
 *
 * Helper module for receiving agents to verify HMAC-SHA256 signed webhook
 * deliveries from the StateSet notification service.  Provides constant-time
 * signature comparison, timestamp freshness checks, replay detection, and a
 * convenient factory that wraps all checks into a single `verify(req)` call.
 *
 * @example
 * ```javascript
 * import { createWebhookVerifier } from './webhook-verify.js';
 *
 * const verifier = createWebhookVerifier('whsec_abc123');
 *
 * // Express / Node HTTP handler
 * app.post('/webhooks', (req, res) => {
 *   const result = verifier.verify(req);
 *   if (!result.valid) return res.status(401).json({ error: result.error });
 *   // result.event, result.payload, result.idempotencyKey …
 *   res.sendStatus(200);
 * });
 * ```
 */

import { createHmac, timingSafeEqual } from 'node:crypto';

// ─── Header names ────────────────────────────────────────────────────────────

/** @type {string} */
const HEADER_SIGNATURE = 'x-stateset-signature';
/** @type {string} */
const HEADER_TIMESTAMP = 'x-stateset-timestamp';
/** @type {string} */
const HEADER_EVENT = 'x-stateset-event';
/** @type {string} */
const HEADER_IDEMPOTENCY_KEY = 'x-stateset-idempotency-key';
/** @type {string} */
const HEADER_DELIVERY_ID = 'x-stateset-delivery-id';

/** Default timestamp tolerance: 5 minutes (300 000 ms) */
const DEFAULT_TIMESTAMP_TOLERANCE_MS = 300_000;

// ─── Public helpers ──────────────────────────────────────────────────────────

/**
 * Verify an HMAC-SHA256 webhook signature using constant-time comparison.
 *
 * @param {string} rawBody          – The raw JSON string body (as sent over the wire)
 * @param {string} signatureHeader  – The `X-StateSet-Signature` header value (`sha256=<hex>`)
 * @param {string} secret           – The webhook secret configured for this agent
 * @returns {{ valid: boolean, error?: string }}
 */
export function verifyWebhookSignature(rawBody, signatureHeader, secret) {
  if (!secret || typeof secret !== 'string') {
    return { valid: false, error: 'Webhook secret is required' };
  }
  if (!rawBody || typeof rawBody !== 'string') {
    return { valid: false, error: 'Raw body is required' };
  }
  if (!signatureHeader || typeof signatureHeader !== 'string') {
    return { valid: false, error: 'Signature header is required' };
  }

  // The header MUST follow the `sha256=<hex>` format
  if (!signatureHeader.startsWith('sha256=')) {
    return { valid: false, error: 'Malformed signature header: must start with "sha256="' };
  }

  const receivedHex = signatureHeader.slice('sha256='.length);
  if (!/^[0-9a-f]+$/i.test(receivedHex)) {
    return { valid: false, error: 'Malformed signature header: invalid hex characters' };
  }

  const expectedHex = createHmac('sha256', secret).update(rawBody).digest('hex');

  // Constant-time comparison — both buffers must be the same length
  const receivedBuf = Buffer.from(receivedHex, 'hex');
  const expectedBuf = Buffer.from(expectedHex, 'hex');

  if (receivedBuf.length !== expectedBuf.length) {
    return { valid: false, error: 'Signature length mismatch' };
  }

  const match = timingSafeEqual(receivedBuf, expectedBuf);
  if (!match) {
    return { valid: false, error: 'Signature mismatch' };
  }

  return { valid: true };
}

/**
 * Check whether a webhook timestamp is fresh enough to prevent replay attacks.
 *
 * @param {string} timestampHeader  – The `X-StateSet-Timestamp` header value (ISO 8601 string)
 * @param {number} [toleranceMs]    – Max age in milliseconds (default 300 000 = 5 minutes)
 * @returns {{ valid: boolean, error?: string, ageMs: number }}
 */
export function verifyWebhookTimestamp(
  timestampHeader,
  toleranceMs = DEFAULT_TIMESTAMP_TOLERANCE_MS,
) {
  if (!timestampHeader || typeof timestampHeader !== 'string') {
    return { valid: false, error: 'Timestamp header is required', ageMs: -1 };
  }

  const ts = new Date(timestampHeader);
  if (Number.isNaN(ts.getTime())) {
    return { valid: false, error: 'Invalid timestamp format', ageMs: -1 };
  }

  const ageMs = Date.now() - ts.getTime();

  // Reject timestamps from the future (with small tolerance for clock skew)
  if (ageMs < -toleranceMs) {
    return { valid: false, error: 'Timestamp is in the future', ageMs };
  }

  if (ageMs > toleranceMs) {
    return { valid: false, error: 'Timestamp is too old', ageMs };
  }

  return { valid: true, ageMs };
}

/**
 * Extract all StateSet webhook headers from a headers object into a typed structure.
 *
 * Accepts both plain objects (lowercased keys) and Node.js `IncomingMessage`-style
 * objects with a `.headers` property.
 *
 * @param {Object} headers – Headers object (lowercased keys expected)
 * @returns {{ signature: string|undefined, timestamp: string|undefined, event: string|undefined, idempotencyKey: string|undefined, deliveryId: string|undefined }}
 */
export function extractWebhookHeaders(headers) {
  if (!headers || typeof headers !== 'object') {
    return {
      signature: undefined,
      timestamp: undefined,
      event: undefined,
      idempotencyKey: undefined,
      deliveryId: undefined,
    };
  }

  // Support both raw headers map and IncomingMessage-style
  const h = headers.headers || headers;

  return {
    signature: h[HEADER_SIGNATURE] ?? undefined,
    timestamp: h[HEADER_TIMESTAMP] ?? undefined,
    event: h[HEADER_EVENT] ?? undefined,
    idempotencyKey: h[HEADER_IDEMPOTENCY_KEY] ?? undefined,
    deliveryId: h[HEADER_DELIVERY_ID] ?? undefined,
  };
}

/**
 * Check whether an idempotency key has already been seen (replay detection).
 *
 * @param {string} idempotencyKey – The `X-StateSet-Idempotency-Key` header value
 * @param {Set<string>} seenKeys – A Set of previously processed idempotency keys
 * @returns {boolean} `true` if this is a replay (duplicate), `false` if fresh
 */
export function isReplayAttack(idempotencyKey, seenKeys) {
  if (!idempotencyKey || !seenKeys) {
    return false;
  }
  return seenKeys.has(idempotencyKey);
}

/**
 * Create a pre-configured webhook verifier factory.
 *
 * Returns a `verify(req)` method that validates signature, timestamp, and
 * parses all relevant headers in a single call.
 *
 * @param {string} secret   – The webhook secret for HMAC verification
 * @param {Object} [options]
 * @param {number} [options.timestampToleranceMs=300000] – Max age in ms
 * @param {boolean} [options.requireTimestamp=true] – Whether to reject requests without a timestamp
 * @returns {{ verify: (req: Object) => Object }}
 */
export function createWebhookVerifier(secret, options = {}) {
  const { timestampToleranceMs = DEFAULT_TIMESTAMP_TOLERANCE_MS, requireTimestamp = true } =
    options;

  /**
   * Verify an incoming webhook request.
   *
   * @param {Object} req             – HTTP request-like object
   * @param {Object} req.headers     – Headers (lowercased keys)
   * @param {string} req.body        – Raw body string
   * @returns {{ valid: boolean, error?: string, event?: string, timestamp?: string, idempotencyKey?: string, payload?: Object }}
   */
  function verify(req) {
    if (!req || typeof req !== 'object') {
      return { valid: false, error: 'Request object is required' };
    }

    const rawBody = typeof req.body === 'string' ? req.body : undefined;
    if (!rawBody) {
      return { valid: false, error: 'Request body must be a string' };
    }

    const hdrs = extractWebhookHeaders(req.headers || req);

    // 1. Verify signature
    const sigResult = verifyWebhookSignature(rawBody, hdrs.signature, secret);
    if (!sigResult.valid) {
      return { valid: false, error: sigResult.error };
    }

    // 2. Verify timestamp (if required)
    if (requireTimestamp && !hdrs.timestamp) {
      return { valid: false, error: 'Timestamp header is required' };
    }

    if (hdrs.timestamp) {
      const tsResult = verifyWebhookTimestamp(hdrs.timestamp, timestampToleranceMs);
      if (!tsResult.valid) {
        return { valid: false, error: tsResult.error };
      }
    }

    // 3. Parse body into payload
    let payload;
    try {
      payload = JSON.parse(rawBody);
    } catch {
      return { valid: false, error: 'Failed to parse request body as JSON' };
    }

    return {
      valid: true,
      event: hdrs.event,
      timestamp: hdrs.timestamp,
      idempotencyKey: hdrs.idempotencyKey,
      deliveryId: hdrs.deliveryId,
      payload,
    };
  }

  return { verify };
}

export default {
  verifyWebhookSignature,
  verifyWebhookTimestamp,
  extractWebhookHeaders,
  isReplayAttack,
  createWebhookVerifier,
};
