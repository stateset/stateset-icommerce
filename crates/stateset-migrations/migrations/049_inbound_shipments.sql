-- Inbound shipments (advance ship notices): goods in transit from a supplier
-- toward a receiving warehouse, upstream of a receiving report.
--
-- Repository: crates/stateset-db/src/sqlite/inbound_shipments.rs
-- REST:       crates/stateset-http/src/routes/inbound_shipments.rs
--
-- Money/quantities stored as TEXT; timestamps RFC3339 TEXT.

CREATE TABLE IF NOT EXISTS inbound_shipments (
    id TEXT PRIMARY KEY,
    number TEXT NOT NULL,
    supplier_id TEXT NOT NULL,
    purchase_order_id TEXT,
    warehouse_id TEXT,
    carrier TEXT,
    tracking_number TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    expected_at TEXT,
    received_at TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_inbound_shipments_supplier ON inbound_shipments(supplier_id);
CREATE INDEX IF NOT EXISTS idx_inbound_shipments_status ON inbound_shipments(status);
CREATE INDEX IF NOT EXISTS idx_inbound_shipments_warehouse ON inbound_shipments(warehouse_id);

CREATE TABLE IF NOT EXISTS inbound_shipment_items (
    id TEXT PRIMARY KEY,
    inbound_shipment_id TEXT NOT NULL REFERENCES inbound_shipments(id) ON DELETE CASCADE,
    product_id TEXT NOT NULL,
    sku TEXT NOT NULL,
    quantity_expected TEXT NOT NULL,
    quantity_received TEXT NOT NULL DEFAULT '0'
);
CREATE INDEX IF NOT EXISTS idx_inbound_shipment_items_shipment ON inbound_shipment_items(inbound_shipment_id);
