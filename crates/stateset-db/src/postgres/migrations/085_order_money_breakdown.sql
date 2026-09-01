-- Order-level money breakdown (see the SQLite twin,
-- 078_order_money_breakdown.sql, for the full rationale): orders recorded only
-- a total, so checkout could not carry the cart's tax, shipping and discount
-- and a legitimate capture was rejected as exceeding the order total.
--
-- Existing rows default to zero, preserving today's arithmetic exactly.
ALTER TABLE orders ADD COLUMN IF NOT EXISTS tax_amount NUMERIC(12,2) NOT NULL DEFAULT 0;
ALTER TABLE orders ADD COLUMN IF NOT EXISTS shipping_amount NUMERIC(12,2) NOT NULL DEFAULT 0;
ALTER TABLE orders ADD COLUMN IF NOT EXISTS discount_amount NUMERIC(12,2) NOT NULL DEFAULT 0;
