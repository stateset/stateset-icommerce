/**
 * Inclusion checks that take a leaf hash on trust: the
 * `verify_inclusion_proof`, `verify_chain_anchor` and `verify_receipt` MCP
 * tools and the `proofs verify-proof` / `verify-anchor` / `verify-receipt`
 * commands.
 *
 * A raw leaf hash carries nothing that says where in the tree it sits. Unless
 * the path is held to a tree size the caller knows independently of the
 * proof, an interior node's hash plus the part of the path above it walks to
 * the root exactly as a leaf does (and the root itself does with an empty
 * path). So a hash chain that reaches the root without such a size only shows
 * that SOME node of the tree hashes to it, not that a leaf -- an event -- was
 * committed.
 *
 * So the success booleans here mean bound inclusion:
 *
 * - `valid` (and, for an anchor, `anchored`) is true only when the caller
 *   supplied `leafCount` (`pathBound`) and the path leads from a leaf position
 *   of a tree of that size to the expected root.
 * - `rootReached` is the plain hash-chain result: the path walks from the
 *   given hash to the expected root, with no tree size or leaf index applied.
 *   On its own it proves only that some node of the tree hashes to the root.
 *
 * A receipt bundle is held to the same rule only where its leaf is taken on
 * trust. A ves-v1 bundle's leaf is recomputed from the event it proves, in the
 * VES_LEAF_V1 hash domain, and every interior node is hashed in the
 * VES_NODE_V1 domain, so the bundle's leaf hash cannot be an interior node and
 * the forgery above does not apply; `leafCount` is optional there and, when
 * given, also holds the path to the batch size. Any other format (legacy-v0,
 * or a bundle with no format) takes its leaf hash as given, so it is valid
 * only with a caller `leafCount`.
 *
 * The verifiers themselves (sync/proof-generator.js `verifyInclusionProof`,
 * `verifyReceiptBundle`) still report facts -- the hash-chain result,
 * `pathBound`, `leafBound` -- and these helpers turn them into a verdict, so
 * the tools and commands share one rule and one wording.
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

/** The `reason` an otherwise reachable but unbound result carries. */
export const UNBOUND_REASON = 'no caller leafCount bound the path to a tree size';

/**
 * @returns {{ result: object, shapeOk: boolean }} the verdict with `valid`
 *   (bound inclusion) and `rootReached` (plain hash chain); `shapeOk` is the
 *   verifier's own answer, true when the path met every shape rule it was
 *   given (the caller's leafCount and leafIndex, when supplied)
 */
function verify(pg, { leafHash, proof, expectedRoot, eventId, leafIndex, leafCount }) {
  // leafCount goes in the options, never in the proof data: a size stated by
  // the proof is the prover's claim, and only the caller's binds the path.
  const verdict = pg.verifyInclusionProof(
    { leafHash, proof, expectedRoot, eventId, leafIndex },
    { leafCount },
  );
  // The plain hash chain, with no tree size or leaf index applied.
  const rootReached = pg.verifyMerkleProof(leafHash, proof, expectedRoot);
  const unbound = verdict.valid && !verdict.pathBound;
  return {
    result: {
      ...verdict,
      valid: verdict.valid && verdict.pathBound,
      rootReached,
      ...(unbound && !verdict.reason ? { reason: UNBOUND_REASON } : {}),
    },
    shapeOk: verdict.valid,
  };
}

function boundTree(leafCount) {
  const count = parseTreeInteger(leafCount);
  return count === null ? 'the supplied tree size' : `a ${count}-leaf tree`;
}

function withReason(text, reason) {
  return reason ? `${text}: ${reason}` : text;
}

/**
 * Why a path that reaches the root was still refused: it does not have the
 * shape the caller's tree size (or leaf index) requires.
 */
function shapeFailure(result, input, subject) {
  const against = result.pathBound
    ? `a leaf position of ${boundTree(input.leafCount)}`
    : 'the supplied leafIndex';
  return withReason(
    `the hash chain reaches ${subject}, but not from ${against}, so the hash is not shown to be a leaf`,
    result.reason,
  );
}

/**
 * Verify a raw leaf hash's inclusion path.
 *
 * `valid` is true only for a caller-supplied `leafCount` and a path from a
 * leaf position of a tree of that size to `expectedRoot`. `rootReached` is
 * the plain hash-chain result.
 *
 * @param {{ verifyInclusionProof: Function, verifyMerkleProof: Function }} pg - a proof generator
 * @param {RawLeafInput} input
 * @param {DetailOptions} [options]
 * @returns {object} the verifier's result (format, pathBound, leafCountSource, ...)
 *   with `valid` (bound inclusion), `rootReached` (hash chain), `detail`, and a
 *   `reason` when not valid for a known reason
 */
export function verifyRawLeafInclusion(pg, input, { leafCountHint = DEFAULT_HINT } = {}) {
  const { result, shapeOk } = verify(pg, input);
  let detail;
  if (result.valid) {
    detail =
      `Valid: the path is bound to ${boundTree(input.leafCount)}, and the hash sits at a leaf ` +
      'position and reaches the expected root. The leaf hash itself is taken as given.';
  } else if (!result.rootReached) {
    detail = withReason('Not valid: the proof does not reach the expected root', result.reason);
  } else if (!shapeOk) {
    detail = `Not valid: ${shapeFailure(result, input, 'the expected root')}.`;
  } else {
    detail =
      'Not valid: the hash chain reaches the expected root (rootReached: true), but the path is ' +
      'not bound to a tree size, so this only shows that some node of the tree (possibly an ' +
      `interior node, not a leaf) hashes to the root; ${leafCountHint}.`;
  }
  return { ...result, detail };
}

/**
 * Verify a raw leaf hash's inclusion path against the root an on-chain
 * anchor transaction committed to.
 *
 * `anchored` and `valid` are true only for a caller-supplied `leafCount` and
 * a path from a leaf position of a tree of that size to the root.
 * `rootReached` is the plain hash-chain result.
 *
 * @param {{ verifyInclusionProof: Function, verifyMerkleProof: Function }} pg - a proof generator
 * @param {RawLeafInput & { anchorTxHash: string }} input
 * @param {DetailOptions} [options]
 * @returns {object} the verifier's result plus `anchorTxHash`, `anchored`,
 *   `rootReached` and `detail`
 */
export function verifyRawLeafAnchor(pg, input, { leafCountHint = DEFAULT_HINT } = {}) {
  const { result, shapeOk } = verify(pg, input);
  const tx = input.anchorTxHash;
  let detail;
  if (result.valid) {
    detail =
      `Event proof verified against on-chain root in tx ${tx} ` +
      `(path bound to ${boundTree(input.leafCount)}; the leaf hash itself is taken as given)`;
  } else if (!result.rootReached) {
    detail = withReason('Merkle proof does not match the expected on-chain root', result.reason);
  } else if (!shapeOk) {
    detail = `Not anchored: ${shapeFailure(result, input, `the on-chain root in tx ${tx}`)}.`;
  } else {
    detail =
      `Not anchored: the proof reaches the on-chain root in tx ${tx} (rootReached: true), but ` +
      'the path is not bound to a tree size, so this only shows that some node of the anchored ' +
      `tree hashes to the root, not that this event was committed; ${leafCountHint}.`;
  }
  return { ...result, anchorTxHash: tx, anchored: result.valid, detail };
}

/**
 * Verify a receipt bundle, holding a leaf taken on trust to a caller tree size.
 *
 * Runs the proof generator's `verifyReceiptBundle` (leaf, inclusion, payload
 * hash and signature checks). When the bundle's leaf is not recomputed from
 * its event -- any format but ves-v1 -- a `path_bound` check is added that
 * passes only for a caller-supplied `leafCount`, so `valid` (every check
 * passed) cannot be true for an interior node offered as the leaf. A ves-v1
 * bundle needs no such check: its leaf_hash check recomputes the leaf in the
 * leaf domain, which no interior node can match.
 *
 * `rootReached` is the plain hash chain from the bundle's leafHash to its
 * merkleRoot. The merkleRoot is the bundle's own: comparing it with the
 * anchored batch root is the caller's job.
 *
 * @param {{ verifyReceiptBundle: Function, verifyMerkleProof: Function, PROOF_FORMATS: object }} pg
 * @param {object} bundle - a receipt bundle
 * @param {Buffer|string|object} [publicKey]
 * @param {{ leafCount?: number|string } & DetailOptions} [options]
 * @returns {object} the verifier's result with `valid` recomputed over the
 *   checks, `rootReached` and `detail`
 */
export function verifyReceipt(pg, bundle, publicKey, options = {}) {
  const { leafCount, leafCountHint = DEFAULT_HINT } = options;
  const result = pg.verifyReceiptBundle(bundle, publicKey, { leafCount });
  const rootReached = pg.verifyMerkleProof(
    bundle?.leafHash,
    bundle?.inclusionProof,
    bundle?.merkleRoot,
  );
  const leafOnTrust = result.format !== pg.PROOF_FORMATS.VES_V1;
  const checks = [...result.checks];
  if (leafOnTrust) {
    checks.push({
      check: 'path_bound',
      passed: result.pathBound,
      detail: result.pathBound
        ? `The inclusion path is held to the caller's tree size (${boundTree(leafCount)})`
        : `The ${result.format} leaf hash is taken as given and no caller leafCount bound the ` +
          'path to a tree size, so the inclusion proof only shows that some node of the tree ' +
          `(possibly an interior node, not a leaf) hashes to the root; ${leafCountHint}`,
    });
  }
  const valid = checks.every((c) => c.passed);
  const rootNote = "merkleRoot is the bundle's own: compare it with the anchored batch root";

  let detail;
  if (!valid) {
    const failed = checks.filter((c) => !c.passed);
    const onlyUnbound = failed.length === 1 && failed[0].check === 'path_bound';
    detail = onlyUnbound
      ? `Not valid: ${failed[0].detail}.`
      : `Not valid: ${failed.map((c) => `${c.check}: ${c.detail ?? 'failed'}`).join('; ')}.`;
  } else if (leafOnTrust) {
    detail =
      `Valid: the bundle's leafHash sits at a leaf position of ${boundTree(leafCount)} and ` +
      `reaches merkleRoot. The ${result.format} leaf is taken as given (leafBound: false), so ` +
      `this does not show it is this event's leaf; ${rootNote}.`;
  } else if (result.pathBound) {
    detail =
      'Valid: the leaf recomputed from the event sits at a leaf position of ' +
      `${boundTree(leafCount)} and reaches merkleRoot; ${rootNote}.`;
  } else {
    detail =
      'Valid: the leaf recomputed from the event reaches merkleRoot. A ves-v1 leaf is hashed in ' +
      'its own domain, so it cannot be an interior node; no caller leafCount held the path to ' +
      `the batch size (pathBound: false), which leafCount would add; ${rootNote}.`;
  }
  return { ...result, valid, rootReached, checks, detail };
}
