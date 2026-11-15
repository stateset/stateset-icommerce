/**
 * The inclusion check behind the `sync_verify_inclusion` MCP tool and the
 * `sync verify-inclusion` command.
 *
 * Its leaf is SHA-256(payloadPlainHash || agentSignature), a simplified leaf
 * and NOT the VES v1.0 leaf (client.js `verifyInclusion` is the spec
 * verifier). Because that leaf has no domain prefix, its preimage must never
 * be able to spell a node preimage, VES_NODE_V1 || left || right (75 bytes):
 * if it could, an interior node would pass for a leaf of any envelope. So the
 * leaf material has fixed lengths: a 32-byte payload hash and a 64-byte
 * (Ed25519, the legacy and hybrid profiles) or 3309-byte (ML-DSA-65, the
 * pqc-strict profile) signature, making the preimage 96 or 3341 bytes.
 *
 * @module sync/inline-inclusion
 */

import nodeCrypto from 'node:crypto';

import { computeNodeHash } from './crypto.js';
import { checkIndexedPath, decodeHash32, foldIndexedPath } from './merkle-shape.js';

/** Signature lengths an envelope's `agentSignature` can have. */
export const AGENT_SIGNATURE_LENGTHS = Object.freeze({ ed25519: 64, mlDsa65: 3309 });

const ALLOWED_SIGNATURE_LENGTHS = new Set(Object.values(AGENT_SIGNATURE_LENGTHS));
const EVEN_HEX = /^(?:0x)?(?:[0-9a-fA-F]{2})+$/;

/**
 * Decode an agent signature: hex (optional `0x`) of exactly 64 or 3309 bytes.
 *
 * @param {unknown} value
 * @returns {Buffer|null}
 */
function decodeAgentSignature(value) {
  if (typeof value !== 'string' || !EVEN_HEX.test(value)) return null;
  const buf = Buffer.from(value.startsWith('0x') ? value.slice(2) : value, 'hex');
  return ALLOWED_SIGNATURE_LENGTHS.has(buf.length) ? buf : null;
}

/**
 * Verify an envelope's inclusion under `expectedRoot`.
 *
 * `leafCount` is the batch size as the caller knows it, independently of the
 * proof (e.g. from the commitment). Only that binds the path: `pathBound` is
 * true only when it was given. A `proof.leafCount` the proof states about
 * itself is still applied as a sanity bound (and must agree with the caller's
 * when both are present), but leaves `pathBound` false.
 *
 * @param {{ envelope: any, proof: any, expectedRoot: unknown, leafCount?: unknown }} input
 * @returns {{ valid: boolean, pathBound: boolean, leafCountSource: 'caller'|'proof'|null, eventId: unknown, expectedRoot: unknown, reason?: string }}
 */
export function verifyInlineInclusion({ envelope, proof, expectedRoot, leafCount }) {
  const shape = checkIndexedPath({
    leafIndex: proof?.leafIndex,
    siblings: proof?.proofHashes,
    callerLeafCount: leafCount,
    statedLeafCount: proof?.leafCount,
  });
  const result = (valid, reason) => ({
    valid,
    pathBound: shape.pathBound,
    leafCountSource: shape.leafCountSource,
    eventId: envelope?.eventId,
    expectedRoot,
    ...(reason && { reason }),
  });

  if (!shape.ok) return result(false, shape.reason);

  const payloadPlainHash = decodeHash32(envelope?.payloadPlainHash);
  if (payloadPlainHash === null || typeof envelope.payloadPlainHash !== 'string') {
    return result(false, 'payloadPlainHash is not 32 bytes of hex');
  }
  const agentSignature = decodeAgentSignature(envelope.agentSignature);
  if (agentSignature === null) {
    return result(false, 'agentSignature is not 64 or 3309 bytes of hex');
  }
  const root = decodeHash32(expectedRoot);
  if (root === null || typeof expectedRoot !== 'string') {
    return result(false, 'expectedRoot is not 32 bytes of hex');
  }

  const leaf = nodeCrypto
    .createHash('sha256')
    .update(payloadPlainHash)
    .update(agentSignature)
    .digest();
  const reached = foldIndexedPath(leaf, shape.leafIndex, shape.siblings, computeNodeHash);
  return result(reached.equals(root));
}
