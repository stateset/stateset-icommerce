-- Add versioning columns for catalog entities to support optimistic locking

ALTER TABLE customers ADD COLUMN IF NOT EXISTS version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE products ADD COLUMN IF NOT EXISTS version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE product_variants ADD COLUMN IF NOT EXISTS version INTEGER NOT NULL DEFAULT 1;

CREATE INDEX IF NOT EXISTS idx_customers_id_version ON customers(id, version);
CREATE INDEX IF NOT EXISTS idx_products_id_version ON products(id, version);
CREATE INDEX IF NOT EXISTS idx_product_variants_id_version ON product_variants(id, version);
