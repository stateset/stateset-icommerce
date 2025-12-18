-- Add versioning columns to core entities for optimistic locking
-- Version is incremented on each update to prevent concurrent modification conflicts

ALTER TABLE orders ADD COLUMN IF NOT EXISTS version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE returns ADD COLUMN IF NOT EXISTS version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE shipments ADD COLUMN IF NOT EXISTS version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE payments ADD COLUMN IF NOT EXISTS version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE work_orders ADD COLUMN IF NOT EXISTS version INTEGER NOT NULL DEFAULT 1;

-- Add indexes for common queries that include version checks (optional, for performance)
CREATE INDEX IF NOT EXISTS idx_orders_id_version ON orders(id, version);
CREATE INDEX IF NOT EXISTS idx_returns_id_version ON returns(id, version);
CREATE INDEX IF NOT EXISTS idx_shipments_id_version ON shipments(id, version);
CREATE INDEX IF NOT EXISTS idx_payments_id_version ON payments(id, version);
CREATE INDEX IF NOT EXISTS idx_work_orders_id_version ON work_orders(id, version);
