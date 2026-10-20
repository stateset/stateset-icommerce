-- BM25 full-text search indexes for hybrid vector search
-- Uses SQLite FTS5 to enable lexical matching alongside embeddings

-- Products
CREATE VIRTUAL TABLE IF NOT EXISTS product_fts USING fts5(
    entity_id UNINDEXED,
    name,
    description,
    slug
);

CREATE TRIGGER IF NOT EXISTS product_fts_ai AFTER INSERT ON products BEGIN
    INSERT INTO product_fts(entity_id, name, description, slug)
    VALUES (new.id, new.name, new.description, new.slug);
END;

CREATE TRIGGER IF NOT EXISTS product_fts_ad AFTER DELETE ON products BEGIN
    DELETE FROM product_fts WHERE entity_id = old.id;
END;

CREATE TRIGGER IF NOT EXISTS product_fts_au AFTER UPDATE ON products BEGIN
    DELETE FROM product_fts WHERE entity_id = old.id;
    INSERT INTO product_fts(entity_id, name, description, slug)
    VALUES (new.id, new.name, new.description, new.slug);
END;

DELETE FROM product_fts;
INSERT INTO product_fts(entity_id, name, description, slug)
SELECT id, name, description, slug FROM products;

-- Customers
CREATE VIRTUAL TABLE IF NOT EXISTS customer_fts USING fts5(
    entity_id UNINDEXED,
    first_name,
    last_name,
    email
);

CREATE TRIGGER IF NOT EXISTS customer_fts_ai AFTER INSERT ON customers BEGIN
    INSERT INTO customer_fts(entity_id, first_name, last_name, email)
    VALUES (new.id, new.first_name, new.last_name, new.email);
END;

CREATE TRIGGER IF NOT EXISTS customer_fts_ad AFTER DELETE ON customers BEGIN
    DELETE FROM customer_fts WHERE entity_id = old.id;
END;

CREATE TRIGGER IF NOT EXISTS customer_fts_au AFTER UPDATE ON customers BEGIN
    DELETE FROM customer_fts WHERE entity_id = old.id;
    INSERT INTO customer_fts(entity_id, first_name, last_name, email)
    VALUES (new.id, new.first_name, new.last_name, new.email);
END;

DELETE FROM customer_fts;
INSERT INTO customer_fts(entity_id, first_name, last_name, email)
SELECT id, first_name, last_name, email FROM customers;

-- Orders
CREATE VIRTUAL TABLE IF NOT EXISTS order_fts USING fts5(
    entity_id UNINDEXED,
    order_number,
    status,
    notes
);

CREATE TRIGGER IF NOT EXISTS order_fts_ai AFTER INSERT ON orders BEGIN
    INSERT INTO order_fts(entity_id, order_number, status, notes)
    VALUES (new.id, new.order_number, new.status, COALESCE(new.notes, ''));
END;

CREATE TRIGGER IF NOT EXISTS order_fts_ad AFTER DELETE ON orders BEGIN
    DELETE FROM order_fts WHERE entity_id = old.id;
END;

CREATE TRIGGER IF NOT EXISTS order_fts_au AFTER UPDATE ON orders BEGIN
    DELETE FROM order_fts WHERE entity_id = old.id;
    INSERT INTO order_fts(entity_id, order_number, status, notes)
    VALUES (new.id, new.order_number, new.status, COALESCE(new.notes, ''));
END;

DELETE FROM order_fts;
INSERT INTO order_fts(entity_id, order_number, status, notes)
SELECT id, order_number, status, COALESCE(notes, '') FROM orders;

-- Inventory items
CREATE VIRTUAL TABLE IF NOT EXISTS inventory_fts USING fts5(
    entity_id UNINDEXED,
    sku,
    name,
    description
);

CREATE TRIGGER IF NOT EXISTS inventory_fts_ai AFTER INSERT ON inventory_items BEGIN
    INSERT INTO inventory_fts(entity_id, sku, name, description)
    VALUES (new.id, new.sku, new.name, COALESCE(new.description, ''));
END;

CREATE TRIGGER IF NOT EXISTS inventory_fts_ad AFTER DELETE ON inventory_items BEGIN
    DELETE FROM inventory_fts WHERE entity_id = old.id;
END;

CREATE TRIGGER IF NOT EXISTS inventory_fts_au AFTER UPDATE ON inventory_items BEGIN
    DELETE FROM inventory_fts WHERE entity_id = old.id;
    INSERT INTO inventory_fts(entity_id, sku, name, description)
    VALUES (new.id, new.sku, new.name, COALESCE(new.description, ''));
END;

DELETE FROM inventory_fts;
INSERT INTO inventory_fts(entity_id, sku, name, description)
SELECT id, sku, name, COALESCE(description, '') FROM inventory_items;
