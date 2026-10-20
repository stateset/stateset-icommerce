-- Per-line shipped quantities (partial shipments) — SQLite
--
-- `order_items.shipped_quantity` tracks how many units of each line have
-- physically shipped. Orders move to `partially_shipped` while
-- SUM(shipped_quantity) < SUM(quantity) and to `shipped` once equal.
-- Returns validate against this column once an order has shipped.
--
-- Backfill: orders that already reached a shipped state before this column
-- existed shipped every unit, so their lines are marked fully shipped.

ALTER TABLE order_items ADD COLUMN shipped_quantity INTEGER NOT NULL DEFAULT 0;

UPDATE order_items
SET shipped_quantity = quantity
WHERE order_id IN (
    SELECT id FROM orders WHERE status IN ('shipped', 'delivered', 'completed')
);
