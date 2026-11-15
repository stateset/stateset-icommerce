/**
 * Inclusion checks that take a leaf hash on trust: the
 * `verify_inclusion_proof` and `verify_chain_anchor` MCP tools and the
 * `proofs verify-proof` / `proofs verify-anchor` commands.
 *
 * A raw leaf hash carries nothing that says where in the tree it sits. Unless
 * the path is held to a tree size the caller knows independently of the
 * proof, an interior node's hash plus the part of the path above it walks to
 * the root exactly as a leaf does (and the root itself does with an empty
 * path). So a result that reaches the root without such a size only shows
 * that SOME node of the tree hashes to it, not that an event was committed.
 *
 * These helpers pass the caller's `leafCount` (and `leafIndex`) to the proof
 * generator's verifier as the caller's own knowledge, so `pathBound` is true
 * only then, and word the result so it claims an event only for a bound path.
 * `valid` and `anchored` keep their long-standing meaning: the hash chain
 * reaches the expected root.
 *
 * @module sync/raw-leaf-inclusion
 */

import { parseTreeInteger } from './merkle-shape.js';

/**
 * @typedef {object} RawLeafInput
 * @property {string} leafHash - hex leaf hash, taken on trust
 * @property {Array<{ position: 'left'|'right', hash: string }>} proof - sibling path, lowest level first
 * @property {string} expectedRoot - hex Merkle root
 * @property {string} [eventId]
 * @property {number|string} [leafIndex] - the position the caller expects; the path's positions must spell it
 * @property {number|string} [leafCount] - the tree size as the caller knows it, independently of the proof
 */

/**
 * @typedef {object} DetailOptions
 * @property {string} [leafCountHint] - how to supply a leaf count in this
 *   interface, e.g. 'pass --leaf-count <n>'
 */

const DEFAULT_HINT =
  'supply leafCount (the batch event count, known independently of the proof) to bind it';

function verify(pg, { leafHash, proof, expectedRoot, eventId, leafIndex, leafCount }) {
  // leafCount goes in the options, never in the proof data: a size stated by
  // the proof is the prover's claim, and only the caller's binds the path.
  return pg.verifyInclusionProof(
    { leafHash, proof, expectedRoot, eventId, leafIndex },
    { leafCount },
  );
}

function boundTree(input) {
  const count = parseTreeInteger(input.leafCount);
  return count === null ? 'the supplied tree size' : `a ${count}-leaf tree`;
}

function failure(prefix, result) {
  return result.reason ? `${prefix}: ${result.reason}` : prefix;
}

/**
 * Verify a raw leaf hash's inclusion path.
 *
 * @param {{ verifyInclusionProof: Function }} pg - a proof generator
 * @param {RawLeafInput} input
 * @param {DetailOptions} [options]
 * @returns {object} the verifier's result (valid, pathBound, leafCountSource, ...) plus `detail`
 */
export function verifyRawLeafInclusion(pg, input, { leafCountHint = DEFAULT_HINT } = {}) {
  const result = verify(pg, input);
  let detail;
  if (!result.valid) {
    detail = failure('The proof does not reach the expected root', result);
  } else if (result.pathBound) {
    detail =
      `The path is bound to ${boundTree(input)}: the leaf hash sits at a leaf position and ` +
      'reaches the expected root. The leaf hash itself is taken as given.';
  } else {
    detail =
      'The hash chain reaches the expected root, but the path is not bound to a tree size, so ' +
      'this only shows that some node of the tree (possibly an interior node, not a leaf) ' +
      `hashes to the root; ${leafCountHint}.`;
  }
  return { ...result, detail };
}

/**
 * Verify a raw leaf hash's inclusion path against the root an on-chain
 * anchor transaction committed to.
 *
 * `anchored` is the hash-chain result, as it has always been. `detail` says
 * the event was anchored only when the path was bound to a caller-supplied
 * tree size.
 *
 * @param {{ verifyInclusionProof: Function }} pg - a proof generator
 * @param {RawLeafInput & { anchorTxHash: string }} input
 * @param {DetailOptions} [options]
 * @returns {object} the verifier's result plus `anchorTxHash`, `anchored` and `detail`
 */
export function verifyRawLeafAnchor(pg, input, { leafCountHint = DEFAULT_HINT } = {}) {
  const result = verify(pg, input);
  const tx = input.anchorTxHash;
  let detail;
  if (!result.valid) {
    detail = failure('Merkle proof does not match the expected on-chain root', result);
  } else if (result.pathBound) {
    detail =
      `Event proof verified against on-chain root in tx ${tx} ` +
      `(path bound to ${boundTree(input)}; the leaf hash itself is taken as given)`;
  } else {
    detail =
      `The proof reaches the on-chain root in tx ${tx}, but the path is not bound to a tree ` +
      'size, so this only shows that some node of the anchored tree hashes to the root, not ' +
      `that this event was committed; ${leafCountHint}.`;
  }
  return { ...result, anchorTxHash: tx, anchored: result.valid, detail };
}
