-- Database-enforced uniqueness for subscription billing cycles (see the SQLite
-- twin, 077_billing_cycle_uniqueness.sql, for the full design rationale).
--
-- One live billing cycle per (subscription_id, cycle_number), enforced by a
-- unique index on a nullable key column: `cycle_number` is caller-supplied so
-- legacy duplicates from the pre-guard era can exist, and the backfill keys
-- only pairs with exactly one live row, so this migration can never fail on
-- real data. Voiding a cycle clears the key and frees the slot.
ALTER TABLE billing_cycles ADD COLUMN IF NOT EXISTS cycle_key TEXT;

UPDATE billing_cycles bc
SET cycle_key = bc.subscription_id::text || ':' || bc.cycle_number::text
WHERE bc.status != 'voided'
  AND (
      SELECT COUNT(*) FROM billing_cycles dup
      WHERE dup.subscription_id = bc.subscription_id
        AND dup.cycle_number = bc.cycle_number
        AND dup.status != 'voided'
  ) = 1;

CREATE UNIQUE INDEX IF NOT EXISTS idx_billing_cycles_cycle_key
    ON billing_cycles(cycle_key);
