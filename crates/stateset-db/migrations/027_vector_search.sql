-- Vector embeddings for semantic search
-- Stores embeddings as BLOBs with cosine similarity computed in application layer
-- Uses OpenAI text-embedding-3-small (1536 dimensions)

-- Product embeddings (searchable by name + description)
CREATE TABLE IF NOT EXISTS product_embeddings (
    product_id TEXT PRIMARY KEY,
    embedding BLOB NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Customer embeddings (searchable by name + email + notes)
CREATE TABLE IF NOT EXISTS customer_embeddings (
    customer_id TEXT PRIMARY KEY,
    embedding BLOB NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Order embeddings (searchable by notes + items)
CREATE TABLE IF NOT EXISTS order_embeddings (
    order_id TEXT PRIMARY KEY,
    embedding BLOB NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Inventory item embeddings (searchable by name + SKU + description)
CREATE TABLE IF NOT EXISTS inventory_embeddings (
    item_id INTEGER PRIMARY KEY,
    embedding BLOB NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Metadata table to track embedding status
CREATE TABLE IF NOT EXISTS embedding_metadata (
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    model TEXT NOT NULL DEFAULT 'text-embedding-3-small',
    text_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (entity_type, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_embedding_metadata_type ON embedding_metadata(entity_type);
CREATE INDEX IF NOT EXISTS idx_product_embeddings_created ON product_embeddings(created_at);
CREATE INDEX IF NOT EXISTS idx_customer_embeddings_created ON customer_embeddings(created_at);
CREATE INDEX IF NOT EXISTS idx_order_embeddings_created ON order_embeddings(created_at);
CREATE INDEX IF NOT EXISTS idx_inventory_embeddings_created ON inventory_embeddings(created_at);
