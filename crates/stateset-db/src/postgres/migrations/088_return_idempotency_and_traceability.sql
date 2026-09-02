-- Returns hardening: serial/lot traceability on return lines (see the SQLite
-- twin, 081_return_idempotency_and_traceability.sql).
--
-- Which lot / serial numbers were physically received with the line,
-- recorded at disposition. `serial_ids` is a JSON array of serial UUIDs.
-- (`idx_returns_idempotency_key`, migration 030, already makes the return
-- idempotency key unique; the application now resolves the unique violation
-- into a replay of the original return.)
ALTER TABLE return_items ADD COLUMN IF NOT EXISTS lot_id UUID REFERENCES lots(id);
ALTER TABLE return_items ADD COLUMN IF NOT EXISTS serial_ids JSONB;
CREATE INDEX IF NOT EXISTS idx_return_items_lot ON return_items(lot_id);
