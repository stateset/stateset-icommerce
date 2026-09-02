-- Database-enforced "one open reservation per serial" (see the SQLite twin,
-- 079_serial_reservation_uniqueness.sql, for the full design rationale).
--
-- A nullable `active_key` column (= serial_id while the reservation is open,
-- NULL once released/consumed/swept) under a unique index. The backfill keys
-- only reservations whose serial is currently `reserved` and that are the
-- single open row for that serial, so legacy duplicates from the pre-lock era
-- can never make this migration fail.
ALTER TABLE serial_reservations ADD COLUMN IF NOT EXISTS active_key TEXT;

UPDATE serial_reservations sr
SET active_key = sr.serial_id::text
WHERE sr.released_at IS NULL
  AND sr.serial_id IN (SELECT id FROM serial_numbers WHERE status = 'reserved')
  AND (
      SELECT COUNT(*) FROM serial_reservations dup
      WHERE dup.serial_id = sr.serial_id
        AND dup.released_at IS NULL
  ) = 1;

CREATE UNIQUE INDEX IF NOT EXISTS idx_serial_reservations_active_key
    ON serial_reservations(active_key);
