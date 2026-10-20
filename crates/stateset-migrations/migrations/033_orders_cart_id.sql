-- Orders <-> Carts linkage for checkout idempotency (SQLite)
--
-- Allows cart checkout to be safely retried after partial failures (e.g., order
-- created but cart update failed) by looking up the order via cart_id.

ALTER TABLE orders ADD COLUMN cart_id TEXT REFERENCES carts(id);

-- Enforce at most one order per cart. Multiple NULLs are allowed.
CREATE UNIQUE INDEX IF NOT EXISTS idx_orders_cart_id_unique ON orders(cart_id) WHERE cart_id IS NOT NULL;

