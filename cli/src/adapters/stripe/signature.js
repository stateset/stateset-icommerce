/**
 * Stripe Webhook Signature Verification
 *
 * Implements Stripe's v1 signature scheme:
 * 1. Extract timestamp and signatures from Stripe-Signature header
 * 2. Build signed payload: `${timestamp}.${rawBody}`
 * 3. HMAC-SHA256 with webhook endpoint secret
 * 4. Compare with timing-safe equality
 * 5. Reject if timestamp is too old (default: 300 seconds)
 */

import crypto from 'crypto';

const DEFAULT_TOLERANCE_SECONDS = 300; // 5 minutes

/**
 * Parse the Stripe-Signature header into its components.
 * Format: "t=<timestamp>,v1=<sig1>,v1=<sig2>,..."
 *
 * @param {string} header
 * @returns {{ timestamp: number, signatures: string[] }}
 */
export function parseStripeSignatureHeader(header) {
  if (!header || typeof header !== 'string') {
    throw new Error('Missing Stripe-Signature header');
  }

  let timestamp = 0;
  const signatures = [];

  const parts = header.split(',');
  for (const part of parts) {
    const [key, ...valueParts] = part.split('=');
    const value = valueParts.join('=');
    const trimmedKey = key.trim();

    if (trimmedKey === 't') {
      timestamp = parseInt(value, 10);
      if (Number.isNaN(timestamp)) {
        throw new Error('Invalid timestamp in Stripe-Signature header');
      }
    } else if (trimmedKey === 'v1') {
      if (value) signatures.push(value);
    }
  }

  if (!timestamp) {
    throw new Error('No timestamp found in Stripe-Signature header');
  }

  if (signatures.length === 0) {
    throw new Error('No v1 signatures found in Stripe-Signature header');
  }

  return { timestamp, signatures };
}

/**
 * Compute the expected HMAC-SHA256 signature for a Stripe webhook.
 *
 * @param {string} rawBody - Raw request body (string, NOT parsed JSON)
 * @param {number} timestamp - Unix timestamp from the header
 * @param {string} secret - Webhook endpoint secret (whsec_...)
 * @returns {string} Hex-encoded HMAC-SHA256
 */
export function computeSignature(rawBody, timestamp, secret) {
  const signedPayload = `${timestamp}.${rawBody}`;
  return crypto.createHmac('sha256', secret).update(signedPayload, 'utf-8').digest('hex');
}

/**
 * Verify a Stripe webhook signature.
 *
 * @param {string} rawBody - Raw request body string
 * @param {string} signatureHeader - Value of the Stripe-Signature header
 * @param {string} secret - Webhook endpoint secret
 * @param {number} [toleranceSeconds=300] - Max age of the event in seconds
 * @returns {{ valid: boolean, error?: string }}
 */
export function verifyStripeSignature(
  rawBody,
  signatureHeader,
  secret,
  toleranceSeconds = DEFAULT_TOLERANCE_SECONDS,
) {
  if (!rawBody || typeof rawBody !== 'string') {
    return { valid: false, error: 'Missing or invalid request body' };
  }

  if (!secret || typeof secret !== 'string') {
    return { valid: false, error: 'Missing webhook secret' };
  }

  let parsed;
  try {
    parsed = parseStripeSignatureHeader(signatureHeader);
  } catch (err) {
    return { valid: false, error: err.message };
  }

  const { timestamp, signatures } = parsed;

  // Check timestamp tolerance
  const now = Math.floor(Date.now() / 1000);
  if (Math.abs(now - timestamp) > toleranceSeconds) {
    return {
      valid: false,
      error: `Timestamp outside tolerance (${toleranceSeconds}s). Event age: ${Math.abs(now - timestamp)}s`,
    };
  }

  // Compute expected signature
  const expected = computeSignature(rawBody, timestamp, secret);

  // Timing-safe comparison against all v1 signatures
  const expectedBuf = Buffer.from(expected, 'utf-8');
  for (const sig of signatures) {
    const sigBuf = Buffer.from(sig, 'utf-8');
    if (expectedBuf.length === sigBuf.length && crypto.timingSafeEqual(expectedBuf, sigBuf)) {
      return { valid: true };
    }
  }

  return { valid: false, error: 'Signature mismatch' };
}
