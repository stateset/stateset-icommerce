// Sign audit artifacts produced by agentic plan execution.
//
// Each tool call's audit envelope (params, policy, permission, charge,
// rollback contract, etc.) is hashed and signed so downstream auditors
// can verify the artifact wasn't tampered with after emission. The
// signing strategy is environment-driven:
//
//   - If a signing key is provided → HMAC-SHA256 (`signed: true`).
//   - Otherwise → a deterministic SHA-256 marker
//     (`unsigned-deterministic`). This is intentionally NOT a
//     cryptographic signature; it just gives every artifact a consistent
//     `signature` field shape so downstream code doesn't have to handle
//     two different result types. Auditors check `signed: false` to
//     reject unsigned artifacts in production.
//
// Extracted from mcp-server.js. The env-var read is moved into the
// orchestrator's wrapper so this module is pure and unit-testable
// without `process.env` mocking.

import { createHmac } from 'node:crypto';

import { sha256, stableStringify } from './replay-sanitizer.js';

/**
 * @typedef {Object} AuditSignature
 * @property {string} payloadHash       - hex SHA-256 of the canonical payload
 * @property {string} signature         - HMAC-SHA256 hex if signed, deterministic SHA-256 hex otherwise
 * @property {'hmac-sha256'|'sha256'} algorithm
 * @property {string} keyId             - the signing key identifier (or `'unsigned-deterministic'`)
 * @property {boolean} signed           - whether a real signing key was used
 */

/**
 * Compute the audit signature envelope for an arbitrary JSON payload.
 *
 * @param {unknown} payload - the audit envelope to sign (any
 *   JSON-serializable value; canonicalized internally via `stableStringify`)
 * @param {object} [opts]
 * @param {string} [opts.signingKey=''] - HMAC key. When non-empty, the
 *   result is a real HMAC-SHA256. When empty, the result falls back to a
 *   deterministic-but-unsigned marker (`signed: false`).
 * @param {string} [opts.keyId='stateset-default'] - identifier echoed in
 *   the signature for downstream key-rotation tracking. Ignored on the
 *   unsigned path (which always reports `'unsigned-deterministic'`).
 * @returns {AuditSignature}
 */
export function signAuditArtifact(payload, opts = {}) {
  const { signingKey = '', keyId = 'stateset-default' } = opts;
  const canonical = stableStringify(payload);
  const payloadHash = sha256(canonical);

  if (signingKey) {
    return {
      payloadHash,
      signature: createHmac('sha256', signingKey).update(canonical).digest('hex'),
      algorithm: 'hmac-sha256',
      keyId,
      signed: true,
    };
  }

  return {
    payloadHash,
    signature: sha256(`unsigned:${payloadHash}`),
    algorithm: 'sha256',
    keyId: 'unsigned-deterministic',
    signed: false,
  };
}
