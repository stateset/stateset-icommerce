/**
 * Canonical shape of a Merkle inclusion path.
 *
 * The tree is padded to a power of two, so a path has exactly one sibling per
 * level. Accepting any other length lets a leaf hash stand in for a root, or
 * an interior node for a leaf. This is the rule
 * `verify_command_inclusion_proof` enforces in
 * crates/stateset-sync/src/attestation.rs; the JavaScript verifiers share it
 * from here.
 *
 * @module sync/merkle-shape
 */

/** Deepest possible path: the Rust verifier counts leaves in a u32. */
export const MAX_MERKLE_DEPTH = 32;

const MAX_LEAF_COUNT = 0xffffffff;

/**
 * Number of siblings in an inclusion path of a tree with `leafCount` leaves:
 * `ceil(log2(leafCount))`, so 0 for a single leaf.
 *
 * Computed by doubling rather than shifting, because `<<` and `>>` work on
 * 32-bit integers and wrap.
 *
 * @param {unknown} leafCount
 * @returns {number|null} the depth, or null when `leafCount` is not a usable tree size
 */
export function merklePathDepth(leafCount) {
  if (!Number.isSafeInteger(leafCount) || leafCount < 1 || leafCount > MAX_LEAF_COUNT) {
    return null;
  }
  let depth = 0;
  for (let width = 1; width < leafCount; width *= 2) {
    depth += 1;
  }
  return depth;
}
