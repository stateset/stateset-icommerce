/** Canonical JSON for signed commerce payloads.
 *
 * One implementation, shared by every producer and verifier of a signature or
 * digest over commerce data (the purchase runtime, the marketplace sequencer
 * bridge). Two canonicalizers that disagree on a single value are a forgery
 * surface: a message that canonicalizes one way when signed and another way
 * when verified either fails to verify or — worse — lets two distinct objects
 * share one signature.
 *
 * Rules:
 *   - object keys are emitted in ascending code-unit order,
 *   - arrays keep their order,
 *   - every scalar is `JSON.stringify`d exactly once, and
 *   - a value JSON cannot represent (`undefined`, a function, a symbol) is a
 *     hard error. `JSON.stringify(undefined)` returns `undefined`, which
 *     string interpolation would turn into the literal text `undefined`; that
 *     makes `{ winner: undefined }` and `{ winner: 'undefined' }` sign the
 *     same bytes. Reject instead.
 *
 * Placed at the package root (not under `lib/`) so the published
 * `@stateset/embedded` `files` glob (`*.mjs`) ships it.
 */
export function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (value !== null && typeof value === 'object') {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`)
      .join(',')}}`;
  }
  const encoded = JSON.stringify(value);
  if (encoded === undefined) {
    throw new Error(
      'commerce data must be JSON serializable: undefined, functions and symbols have no canonical form',
    );
  }
  return encoded;
}
