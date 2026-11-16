/**
 * Canonical shape of a Merkle inclusion path.
 *
 * The tree is padded to a power of two, so a path has exactly one sibling per
 * level. Accepting any other length lets a leaf hash stand in for a root, or
 * an interior node for a leaf. This is the length and index rule
 * `verify_command_inclusion_proof` enforces in
 * crates/stateset-sync/src/attestation.rs; the JavaScript verifiers share it
 * from here. (That function also requires siblings past the last leaf to be
 * the canonical padding subtree; these verifiers do not check that yet.)
 *
 * A tree size only binds a path when it comes from somewhere other than the
 * proof. A size the proof states about itself is chosen by whoever made the
 * proof, so it is applied as a sanity bound but is never reported as binding:
 * `pathBound` is true only for a size the caller supplied. `leafCountSource`
 * says where the size came from.
 *
 * @module sync/merkle-shape
 */

/** Deepest possible path: the Rust verifier counts leaves in a u32. */
export const MAX_MERKLE_DEPTH = 32;

const MAX_LEAF_COUNT = 0xffffffff;

const CANONICAL_DECIMAL = /^(0|[1-9][0-9]*)$/;
const HASH32_HEX = /^(?:0x)?[0-9a-fA-F]{64}$/;

/**
 * Read a leaf index or a leaf count.
 *
 * Accepts a nonnegative safe integer, either as a number or as a canonical
 * decimal string (`/^(0|[1-9][0-9]*)$/`, at most Number.MAX_SAFE_INTEGER):
 * the gRPC client loads uint64 fields as strings. Everything else -- floats,
 * negatives, NaN, BigInt, and strings such as "05", " 5", "5.0" or "0x5" --
 * is refused, so one value has exactly one accepted spelling.
 *
 * This is the only place the verifiers parse these values.
 *
 * @param {unknown} value
 * @returns {number|null} the integer, or null when `value` is not one
 */
export function parseTreeInteger(value) {
  if (typeof value === 'number') {
    if (!Number.isSafeInteger(value) || value < 0) return null;
    return value === 0 ? 0 : value; // -0 reads as 0
  }
  if (typeof value === 'string' && CANONICAL_DECIMAL.test(value)) {
    const parsed = Number(value);
    // Above MAX_SAFE_INTEGER, Number() rounds to a value that is not safe.
    return Number.isSafeInteger(parsed) ? parsed : null;
  }
  return null;
}

/**
 * Number of siblings in an inclusion path of a tree with `leafCount` leaves:
 * `ceil(log2(leafCount))`, so 0 for a single leaf.
 *
 * Computed by doubling rather than shifting, because `<<` and `>>` work on
 * 32-bit integers and wrap.
 *
 * @param {unknown} leafCount - read with parseTreeInteger
 * @returns {number|null} the depth, or null when `leafCount` is not a usable tree size
 */
export function merklePathDepth(leafCount) {
  const count = parseTreeInteger(leafCount);
  if (count === null || count < 1 || count > MAX_LEAF_COUNT) {
    return null;
  }
  let depth = 0;
  for (let width = 1; width < count; width *= 2) {
    depth += 1;
  }
  return depth;
}

/**
 * Decode one 32-byte hash: exactly 64 hex digits (an optional `0x` prefix),
 * or a 32-byte Buffer/Uint8Array (the gRPC client returns bytes).
 *
 * `Buffer.from(s, 'hex')` stops at the first character that is not hex and
 * drops an odd trailing digit, so it would hash a truncated sibling instead of
 * refusing it. This refuses.
 *
 * @param {unknown} value
 * @returns {Buffer|null}
 */
export function decodeHash32(value) {
  if (typeof value === 'string') {
    if (!HASH32_HEX.test(value)) return null;
    return Buffer.from(value.startsWith('0x') ? value.slice(2) : value, 'hex');
  }
  if (value instanceof Uint8Array && value.length === 32) {
    return Buffer.from(value);
  }
  return null;
}

/**
 * Decide which tree size a path is held to.
 *
 * - A caller-supplied size wins: `leafCountSource` is 'caller' and
 *   `pathBound` is true. If the proof also states a size, the two must agree.
 * - Otherwise a size the proof (or bundle) states is still applied, as a
 *   sanity bound: `leafCountSource` is `statedSource` and `pathBound` false.
 * - With neither, there is no size: `leafCount` and `leafCountSource` are null.
 *
 * Any size that is present must be usable (1 to 2^32 - 1), or the result is
 * not ok.
 *
 * @param {{ callerLeafCount?: unknown, statedLeafCount?: unknown, statedSource?: 'proof'|'bundle' }} sources
 * @returns {{ ok: boolean, reason?: string, leafCount: number|null, depth: number|null, leafCountSource: 'caller'|'proof'|'bundle'|null, pathBound: boolean }}
 */
export function resolveLeafCount({ callerLeafCount, statedLeafCount, statedSource = 'proof' }) {
  const hasCaller = (callerLeafCount ?? null) !== null;
  const hasStated = (statedLeafCount ?? null) !== null;
  const leafCountSource = hasCaller ? 'caller' : hasStated ? statedSource : null;
  const pathBound = leafCountSource === 'caller';
  const refuse = (reason) => ({
    ok: false,
    reason,
    leafCount: null,
    depth: null,
    leafCountSource,
    pathBound,
  });

  if (hasCaller && merklePathDepth(callerLeafCount) === null) {
    return refuse('the caller-supplied leaf count is not a usable tree size');
  }
  if (hasStated && merklePathDepth(statedLeafCount) === null) {
    return refuse(`the ${statedSource}'s leaf count is not a usable tree size`);
  }
  if (
    hasCaller &&
    hasStated &&
    parseTreeInteger(callerLeafCount) !== parseTreeInteger(statedLeafCount)
  ) {
    return refuse(`the ${statedSource}'s leaf count disagrees with the caller-supplied one`);
  }
  if (leafCountSource === null) {
    return { ok: true, leafCount: null, depth: null, leafCountSource, pathBound };
  }
  const chosen = hasCaller ? callerLeafCount : statedLeafCount;
  return {
    ok: true,
    leafCount: parseTreeInteger(chosen),
    depth: merklePathDepth(chosen),
    leafCountSource,
    pathBound,
  };
}

/**
 * Check an index-addressed path (a leaf index plus a list of sibling hashes)
 * against the canonical shape, and decode it.
 *
 * Always: at most MAX_MERKLE_DEPTH siblings, each exactly 32 bytes, and a
 * nonnegative integer index the path can address (below 2^siblings), so no
 * bit of it goes unread. With a tree size (see resolveLeafCount): exactly
 * `merklePathDepth(leafCount)` siblings and an index inside the tree.
 * `requireLeafCount` refuses a path with no tree size at all.
 *
 * @param {{ leafIndex: unknown, siblings: unknown, callerLeafCount?: unknown, statedLeafCount?: unknown, statedSource?: 'proof'|'bundle', requireLeafCount?: boolean }} shape
 * @returns {{ ok: boolean, reason?: string, pathBound: boolean, leafCountSource: 'caller'|'proof'|'bundle'|null, leafCount: number|null, leafIndex: number|null, siblings: Buffer[]|null }}
 */
export function checkIndexedPath({
  leafIndex,
  siblings,
  callerLeafCount,
  statedLeafCount,
  statedSource = 'proof',
  requireLeafCount = false,
}) {
  const count = resolveLeafCount({ callerLeafCount, statedLeafCount, statedSource });
  const verdict = {
    pathBound: count.pathBound,
    leafCountSource: count.leafCountSource,
    leafCount: count.leafCount,
  };
  const refuse = (reason) => ({ ok: false, reason, ...verdict, leafIndex: null, siblings: null });

  if (!count.ok) return refuse(count.reason);
  if (requireLeafCount && count.leafCount === null) {
    return refuse('no usable leaf count');
  }
  if (!Array.isArray(siblings) || siblings.length > MAX_MERKLE_DEPTH) {
    return refuse(`the path is not a list of at most ${MAX_MERKLE_DEPTH} sibling hashes`);
  }
  const index = parseTreeInteger(leafIndex);
  if (index === null || index >= 2 ** siblings.length) {
    return refuse('the leaf index is not a nonnegative integer the path can address');
  }
  if (count.leafCount !== null) {
    if (siblings.length !== count.depth) {
      return refuse(
        `expected ${count.depth} sibling hashes for ${count.leafCount} leaves, got ${siblings.length}`,
      );
    }
    if (index >= count.leafCount) {
      return refuse('the leaf index is outside the tree');
    }
  }
  const decoded = [];
  for (const sibling of siblings) {
    const buf = decodeHash32(sibling);
    if (buf === null) return refuse('a sibling is not a 32-byte hash');
    decoded.push(buf);
  }
  return { ok: true, ...verdict, leafIndex: index, siblings: decoded };
}

/**
 * Fold an index-addressed path up from `leaf`: at each level the index's low
 * bit says whether the running hash is the right child. The index is halved,
 * not shifted, because `>>` works on 32 bits and wraps past level 31.
 *
 * Takes the decoded output of checkIndexedPath.
 *
 * @param {Buffer} leaf
 * @param {number} leafIndex
 * @param {Buffer[]} siblings
 * @param {(left: Buffer, right: Buffer) => Buffer} nodeHash
 * @returns {Buffer} the root the path leads to
 */
export function foldIndexedPath(leaf, leafIndex, siblings, nodeHash) {
  let hash = leaf;
  let index = leafIndex;
  for (const sibling of siblings) {
    hash = index % 2 === 1 ? nodeHash(sibling, hash) : nodeHash(hash, sibling);
    index = Math.floor(index / 2);
  }
  return hash;
}
