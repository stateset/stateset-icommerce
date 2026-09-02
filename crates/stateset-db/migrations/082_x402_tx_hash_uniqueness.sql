-- Database-enforced uniqueness for x402 settlement transaction hashes.
--
-- Nothing stopped two payment intents from being settled with the same
-- on-chain `tx_hash`: there was no index on the column at all. The
-- repository's `mark_settled` now checks for a prior settlement inside its
-- write transaction (for a meaningful "already settled intent X" error);
-- this index is the backstop that holds even for writers that bypass it.
--
-- Design notes (why a keyed column instead of a unique index on tx_hash):
--   * `tx_hash` was caller-supplied and unguarded, so legacy databases may
--     already contain duplicates; a plain unique index would make this
--     migration fail on real data.
--   * The backfill keys ONLY hashes that settle exactly one row today and
--     leaves duplicated hashes NULL, so the migration can never fail. New
--     settlements are always keyed by the application (`tx_hash_key = tx_hash`).
--   * NULLs are distinct in a unique index, so unsettled intents never collide.
ALTER TABLE x402_payment_intents ADD COLUMN tx_hash_key TEXT;

UPDATE x402_payment_intents
SET tx_hash_key = tx_hash
WHERE tx_hash IS NOT NULL
  AND (
      SELECT COUNT(*) FROM x402_payment_intents dup
      WHERE dup.tx_hash = x402_payment_intents.tx_hash
  ) = 1;

CREATE UNIQUE INDEX IF NOT EXISTS ux_x402_intents_tx_hash_key
    ON x402_payment_intents(tx_hash_key);
