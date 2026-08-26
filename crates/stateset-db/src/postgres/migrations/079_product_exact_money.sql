-- Product prices are agent-facing exact decimals. The legacy NUMERIC(19,4)
-- ceiling rejected valid rust_decimal values and diverged from SQLite TEXT
-- storage, so preserve the full decimal contract without coercion or overflow.
ALTER TABLE product_variants
  ALTER COLUMN price TYPE NUMERIC USING price::NUMERIC,
  ALTER COLUMN compare_at_price TYPE NUMERIC USING compare_at_price::NUMERIC,
  ALTER COLUMN cost TYPE NUMERIC USING cost::NUMERIC;
