-- Preserve exact decimal quantities end-to-end. Commerce agents may trade
-- fractional units and quantities larger than NUMERIC(19,4) can represent.
ALTER TABLE inventory_balances
    ALTER COLUMN quantity_on_hand TYPE NUMERIC,
    ALTER COLUMN quantity_allocated TYPE NUMERIC,
    ALTER COLUMN quantity_available TYPE NUMERIC,
    ALTER COLUMN reorder_point TYPE NUMERIC,
    ALTER COLUMN safety_stock TYPE NUMERIC;

ALTER TABLE inventory_transactions
    ALTER COLUMN quantity TYPE NUMERIC;

ALTER TABLE inventory_reservations
    ALTER COLUMN quantity TYPE NUMERIC;
