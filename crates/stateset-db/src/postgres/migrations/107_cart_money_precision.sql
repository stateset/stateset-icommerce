-- Cart money was stored as DECIMAL(12, 2) (prices as DECIMAL(12, 4)), so a
-- three-decimal currency (BHD, KWD, JOD, OMR, TND, LYD, IQD) could not hold
-- its own minor unit and large carts were capped below 10^10.
--
-- Use unconstrained NUMERIC: it stores each value exactly, at the scale it
-- was written with, so amounts (already rounded to the cart currency's minor
-- unit by the repository) read back as written, e.g. `20.00` USD and
-- `3.702` BHD, as SQLite's TEXT columns do. A fixed-scale type such as
-- NUMERIC(38, 18) would instead report every amount with 18 decimals.
-- Widening never rounds an existing value.
ALTER TABLE carts
    ALTER COLUMN subtotal TYPE NUMERIC,
    ALTER COLUMN tax_amount TYPE NUMERIC,
    ALTER COLUMN shipping_amount TYPE NUMERIC,
    ALTER COLUMN discount_amount TYPE NUMERIC,
    ALTER COLUMN grand_total TYPE NUMERIC;

ALTER TABLE cart_items
    ALTER COLUMN unit_price TYPE NUMERIC,
    ALTER COLUMN original_price TYPE NUMERIC,
    ALTER COLUMN discount_amount TYPE NUMERIC,
    ALTER COLUMN tax_amount TYPE NUMERIC,
    ALTER COLUMN total TYPE NUMERIC;
