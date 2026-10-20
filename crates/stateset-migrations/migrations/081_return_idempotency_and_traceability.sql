-- Returns hardening: serial/lot traceability on return lines.
--
-- `return_items.lot_id` / `return_items.serial_ids` record which lot and
-- which serial numbers were physically received with the line. They are set
-- at disposition; the serials are transitioned (`returned` -> disposition
-- target) and the lot's on-hand restored in the same transaction as the
-- stock effect. `serial_ids` is a JSON array of serial UUIDs.
--
-- (Idempotency of `returns.idempotency_key` is already enforced at the
-- database by `idx_returns_idempotency_key`, migration 026; the application
-- now resolves the unique violation into a replay of the original return.)
ALTER TABLE return_items ADD COLUMN lot_id TEXT REFERENCES lots(id);
ALTER TABLE return_items ADD COLUMN serial_ids TEXT;
CREATE INDEX IF NOT EXISTS idx_return_items_lot ON return_items(lot_id);
