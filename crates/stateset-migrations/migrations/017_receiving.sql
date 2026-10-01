-- Receiving/Goods Receipt tables

-- Receipts (ASN/Goods Receipt documents)
CREATE TABLE IF NOT EXISTS receipts (
    id TEXT PRIMARY KEY,
    receipt_number TEXT UNIQUE NOT NULL,
    receipt_type TEXT NOT NULL DEFAULT 'purchase_order',
    status TEXT NOT NULL DEFAULT 'expected',
    reference_type TEXT,
    reference_id TEXT,
    supplier_id TEXT,
    warehouse_id INTEGER NOT NULL,
    carrier TEXT,
    tracking_number TEXT,
    expected_date TEXT,
    received_date TEXT,
    completed_date TEXT,
    expected_quantity TEXT NOT NULL DEFAULT '0',
    received_quantity TEXT NOT NULL DEFAULT '0',
    pending_inspection_quantity TEXT NOT NULL DEFAULT '0',
    put_away_quantity TEXT NOT NULL DEFAULT '0',
    notes TEXT,
    created_by TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (warehouse_id) REFERENCES warehouses(id)
);

CREATE INDEX IF NOT EXISTS idx_receipts_number ON receipts(receipt_number);
CREATE INDEX IF NOT EXISTS idx_receipts_status ON receipts(status);
CREATE INDEX IF NOT EXISTS idx_receipts_warehouse ON receipts(warehouse_id);
CREATE INDEX IF NOT EXISTS idx_receipts_supplier ON receipts(supplier_id);
CREATE INDEX IF NOT EXISTS idx_receipts_reference ON receipts(reference_type, reference_id);
CREATE INDEX IF NOT EXISTS idx_receipts_expected_date ON receipts(expected_date);

-- Receipt line items
CREATE TABLE IF NOT EXISTS receipt_items (
    id TEXT PRIMARY KEY,
    receipt_id TEXT NOT NULL,
    line_number INTEGER NOT NULL,
    sku TEXT NOT NULL,
    description TEXT,
    po_line_id TEXT,
    expected_quantity TEXT NOT NULL DEFAULT '0',
    received_quantity TEXT NOT NULL DEFAULT '0',
    rejected_quantity TEXT NOT NULL DEFAULT '0',
    unit_cost TEXT,
    lot_number TEXT,
    serial_numbers TEXT,
    expiration_date TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (receipt_id) REFERENCES receipts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_receipt_items_receipt ON receipt_items(receipt_id);
CREATE INDEX IF NOT EXISTS idx_receipt_items_sku ON receipt_items(sku);
CREATE INDEX IF NOT EXISTS idx_receipt_items_po_line ON receipt_items(po_line_id);
CREATE INDEX IF NOT EXISTS idx_receipt_items_status ON receipt_items(status);

-- Put-away tasks
CREATE TABLE IF NOT EXISTS put_aways (
    id TEXT PRIMARY KEY,
    receipt_id TEXT NOT NULL,
    receipt_item_id TEXT NOT NULL,
    sku TEXT NOT NULL,
    from_location_id INTEGER,
    to_location_id INTEGER NOT NULL,
    quantity TEXT NOT NULL,
    lot_id TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    assigned_to TEXT,
    started_at TEXT,
    completed_at TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (receipt_id) REFERENCES receipts(id) ON DELETE CASCADE,
    FOREIGN KEY (receipt_item_id) REFERENCES receipt_items(id),
    FOREIGN KEY (from_location_id) REFERENCES locations(id),
    FOREIGN KEY (to_location_id) REFERENCES locations(id)
);

CREATE INDEX IF NOT EXISTS idx_put_aways_receipt ON put_aways(receipt_id);
CREATE INDEX IF NOT EXISTS idx_put_aways_status ON put_aways(status);
CREATE INDEX IF NOT EXISTS idx_put_aways_assigned ON put_aways(assigned_to);
CREATE INDEX IF NOT EXISTS idx_put_aways_to_location ON put_aways(to_location_id);

-- Auto-update timestamps
CREATE TRIGGER IF NOT EXISTS receipts_updated_at
AFTER UPDATE ON receipts
BEGIN
    UPDATE receipts SET updated_at = datetime('now') WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS receipt_items_updated_at
AFTER UPDATE ON receipt_items
BEGIN
    UPDATE receipt_items SET updated_at = datetime('now') WHERE id = NEW.id;
END;
