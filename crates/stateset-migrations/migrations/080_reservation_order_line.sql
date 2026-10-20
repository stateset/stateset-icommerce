-- Key inventory reservations to the order LINE that holds them.
--
-- Order creation and `add_item` reserve stock one reservation per line, but
-- until now a reservation only knew its (reference_type, reference_id, sku).
-- `remove_item` therefore released "whole reservations for this SKU, oldest
-- first, until the removed line's quantity is covered". With two lines sharing
-- a SKU (A qty 5 reserved first, B qty 1) removing B released A's hold: the
-- stock for a line that was still on the order went back to available.
--
-- Design notes:
--   * `order_item_id` is nullable so legacy rows (and non-order references:
--     carts, kernel commands) are untouched. New order-line reservations are
--     always keyed by the application; a split (partial confirm) copies the
--     key onto the confirmed slice.
--   * Legacy rows keep working: the orders module releases/confirms by line
--     first and falls back to the historical SKU-based path only for rows
--     whose `order_item_id` IS NULL, so a keyed line can never be stolen.
--   * No FK: order lines are deleted by `remove_item`/`delete` after their
--     reservations are released, and a released reservation is an audit row.
ALTER TABLE inventory_reservations ADD COLUMN order_item_id TEXT;

CREATE INDEX IF NOT EXISTS idx_inventory_reservations_order_item
    ON inventory_reservations(order_item_id)
    WHERE order_item_id IS NOT NULL;
