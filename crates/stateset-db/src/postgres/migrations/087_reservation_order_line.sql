-- Key inventory reservations to the order LINE that holds them (see the SQLite
-- twin, 080_reservation_order_line.sql, for the full design rationale).
--
-- Nullable so legacy rows and non-order references are untouched; the orders
-- module releases/confirms by line first and falls back to the SKU-based path
-- only for rows whose `order_item_id` IS NULL.
ALTER TABLE inventory_reservations ADD COLUMN IF NOT EXISTS order_item_id UUID;

CREATE INDEX IF NOT EXISTS idx_inventory_reservations_order_item
    ON inventory_reservations(order_item_id)
    WHERE order_item_id IS NOT NULL;
