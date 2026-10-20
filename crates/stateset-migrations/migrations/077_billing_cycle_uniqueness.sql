-- Database-enforced uniqueness for subscription billing cycles.
--
-- A billing worker polls `next_billing_date`, bills, marks the cycle paid and
-- polls again. Without a uniqueness constraint on (subscription_id,
-- cycle_number) nothing stopped a second pass from creating a SECOND cycle for
-- the same period and charging the customer twice. The application layer now
-- advances `next_billing_date` inside the mark-paid transaction; this index is
-- the backstop that holds even for writers that bypass the application layer.
--
-- Design notes (why a keyed column instead of a plain unique index on
-- (subscription_id, cycle_number)):
--   * `cycle_number` is caller-supplied (`CreateBillingCycle`), so legacy
--     databases from the pre-guard era may already contain duplicates; a plain
--     unique index would make this migration fail on real data.
--   * The backfill keys ONLY (subscription, cycle_number) pairs that have
--     exactly one live row today and leaves duplicate rows NULL, so the
--     migration can never fail. New rows are always keyed by the application.
--   * A voided cycle must free its slot for a corrected re-create, so voiding
--     clears the key (SQLite/Postgres treat NULLs as distinct).
ALTER TABLE billing_cycles ADD COLUMN cycle_key TEXT;

-- Backfill: key every (subscription, cycle_number) pair that has exactly one
-- non-voided row today. String concatenation only — no math on TEXT money.
UPDATE billing_cycles
SET cycle_key = subscription_id || ':' || cycle_number
WHERE status != 'voided'
  AND (
      SELECT COUNT(*) FROM billing_cycles dup
      WHERE dup.subscription_id = billing_cycles.subscription_id
        AND dup.cycle_number = billing_cycles.cycle_number
        AND dup.status != 'voided'
  ) = 1;

CREATE UNIQUE INDEX IF NOT EXISTS idx_billing_cycles_cycle_key
    ON billing_cycles(cycle_key);
