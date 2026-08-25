-- Per-line shipped quantities (partial shipments) — PostgreSQL
--
-- Mirrors SQLite migration 067_order_item_shipped_quantity.sql.

ALTER TABLE order_items ADD COLUMN IF NOT EXISTS shipped_quantity INTEGER NOT NULL DEFAULT 0;

UPDATE order_items
SET shipped_quantity = quantity
WHERE order_id IN (
    SELECT id FROM orders WHERE status IN ('shipped', 'delivered', 'completed')
);
