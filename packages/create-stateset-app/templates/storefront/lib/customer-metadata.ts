/**
 * Customer metadata is arbitrary JSON: `@stateset/embedded` types it as
 * `Record<string, unknown>`, so a value read out of it is `unknown` until it
 * has been checked. These helpers do the checking in one place.
 *
 * That matters most for `walletAddress`, which several routes compare to
 * decide whether the caller may see or change someone's orders and
 * subscriptions. Reading it as a string without checking would let a
 * customer whose metadata holds a non-string `walletAddress` (a number, an
 * object, anything a write path let through) take a different branch than
 * intended.
 */

type Metadata = Record<string, unknown> | undefined | null;

/** The value at `key`, but only when it really is a string. */
export function metadataString(metadata: Metadata, key: string): string | undefined {
  const value = metadata?.[key];
  return typeof value === 'string' ? value : undefined;
}

/**
 * Whether this metadata records `wallet` as its wallet address, compared
 * case-insensitively. Absent, or present but not a string, is never a match.
 */
export function walletMatches(metadata: Metadata, wallet: string): boolean {
  const stored = metadataString(metadata, 'walletAddress');
  return stored !== undefined && stored.toLowerCase() === wallet.toLowerCase();
}
