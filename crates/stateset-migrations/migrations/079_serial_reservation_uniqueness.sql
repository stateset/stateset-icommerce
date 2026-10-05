-- Database-enforced "one open reservation per serial".
--
-- `reserve` read the serial, counted its active reservations, then inserted
-- a new one and flipped the serial to `reserved`. On Postgres (READ COMMITTED,
-- no row lock) two orders could both see "available, 0 reservations" and both
-- end up holding the same physical unit. The application layer now locks the
-- serial row and writes the status conditionally; this index is the backstop
-- that holds even for writers that bypass the application layer.
--
-- Design notes (why a keyed column instead of a partial unique index on
-- (serial_id) WHERE released_at IS NULL) — the same shape as
-- 077_billing_cycle_uniqueness:
--   * Legacy databases can already hold several open rows for one serial
--     (the race above, or confirmed reservations that were never closed
--     because `mark_sold`/`mark_shipped` did not consume them). A partial
--     unique index would make this migration fail on real data.
--   * The backfill keys ONLY reservations whose serial is currently
--     `reserved` and that are the single open row for that serial; every
--     other open row stays NULL, so the migration can never fail. New rows
--     are always keyed by the application, and closing a reservation
--     (release, consume on ship/sell, expiry sweep) clears the key.
ALTER TABLE serial_reservations ADD COLUMN active_key TEXT;

UPDATE serial_reservations
SET active_key = serial_id
WHERE released_at IS NULL
  AND serial_id IN (SELECT id FROM serial_numbers WHERE status = 'reserved')
  AND (
      SELECT COUNT(*) FROM serial_reservations dup
      WHERE dup.serial_id = serial_reservations.serial_id
        AND dup.released_at IS NULL
  ) = 1;

CREATE UNIQUE INDEX IF NOT EXISTS idx_serial_reservations_active_key
    ON serial_reservations(active_key);
