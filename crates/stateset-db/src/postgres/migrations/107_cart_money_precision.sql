-- Cart money was stored as DECIMAL(12, 2) (prices as DECIMAL(12, 4)), so a
-- three-decimal currency (BHD, KWD, JOD, OMR, TND, LYD, IQD) could not hold
-- its own minor unit and large carts were capped below 10^10. Widen to the
-- NUMERIC(38, 18) every other money column uses; existing values are kept
-- exactly (widening never rounds).
ALTER TABLE carts
    ALTER COLUMN subtotal TYPE NUMERIC(38, 18),
    ALTER COLUMN tax_amount TYPE NUMERIC(38, 18),
    ALTER COLUMN shipping_amount TYPE NUMERIC(38, 18),
    ALTER COLUMN discount_amount TYPE NUMERIC(38, 18),
    ALTER COLUMN grand_total TYPE NUMERIC(38, 18);

ALTER TABLE cart_items
    ALTER COLUMN unit_price TYPE NUMERIC(38, 18),
    ALTER COLUMN original_price TYPE NUMERIC(38, 18),
    ALTER COLUMN discount_amount TYPE NUMERIC(38, 18),
    ALTER COLUMN tax_amount TYPE NUMERIC(38, 18),
    ALTER COLUMN total TYPE NUMERIC(38, 18);
