-- Add versioning columns for optimistic concurrency control
-- This enables safe concurrent updates by tracking entity versions

-- Add version column to orders
ALTER TABLE orders ADD COLUMN version INTEGER NOT NULL DEFAULT 1;

-- Add version column to customers (for consistency)
ALTER TABLE customers ADD COLUMN version INTEGER NOT NULL DEFAULT 1;

-- Add version column to products
ALTER TABLE products ADD COLUMN version INTEGER NOT NULL DEFAULT 1;

-- Add version column to product_variants
ALTER TABLE product_variants ADD COLUMN version INTEGER NOT NULL DEFAULT 1;

-- Add version column to returns
ALTER TABLE returns ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
