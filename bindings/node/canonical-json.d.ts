/**
 * Canonical JSON for signed commerce payloads: object keys sorted, arrays in
 * order, every scalar `JSON.stringify`d once. A value JSON cannot represent
 * (`undefined`, a function, a symbol) throws rather than serializing to the
 * literal text `undefined`, which would let two distinct objects share one
 * signature.
 */
export function canonicalJson(value: unknown): string;
