-- Database-enforced uniqueness for x402 settlement transaction hashes (see
-- the SQLite twin, 082_x402_tx_hash_uniqueness.sql, for the full rationale).
--
-- One settled intent per on-chain `tx_hash`, enforced by a unique index on a
-- nullable key column: `tx_hash` was unguarded so legacy duplicates can exist,
-- and the backfill keys only hashes that settle exactly one row, so this
-- migration can never fail on real data. `mark_settled` writes
-- `tx_hash_key = tx_hash` for every new settlement.
ALTER TABLE x402_payment_intents ADD COLUMN IF NOT EXISTS tx_hash_key TEXT;

UPDATE x402_payment_intents pi
SET tx_hash_key = pi.tx_hash
WHERE pi.tx_hash IS NOT NULL
  AND pi.tx_hash_key IS NULL
  AND (
      SELECT COUNT(*) FROM x402_payment_intents dup
      WHERE dup.tx_hash = pi.tx_hash
  ) = 1;

CREATE UNIQUE INDEX IF NOT EXISTS ux_x402_intents_tx_hash_key
    ON x402_payment_intents(tx_hash_key);
