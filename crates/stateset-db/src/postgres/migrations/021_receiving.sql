-- Receiving/Goods Receipt schema (PostgreSQL)

CREATE TABLE IF NOT EXISTS receipts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    receipt_number TEXT UNIQUE NOT NULL,
    receipt_type TEXT NOT NULL DEFAULT 'purchase_order',
    status TEXT NOT NULL DEFAULT 'expected',
    reference_type TEXT,
    reference_id UUID,
    supplier_id UUID,
    warehouse_id INTEGER NOT NULL REFERENCES warehouses(id),
    carrier TEXT,
    tracking_number TEXT,
    expected_date TIMESTAMPTZ,
    received_date TIMESTAMPTZ,
    completed_date TIMESTAMPTZ,
    expected_quantity NUMERIC(12, 4) NOT NULL DEFAULT 0,
    received_quantity NUMERIC(12, 4) NOT NULL DEFAULT 0,
    pending_inspection_quantity NUMERIC(12, 4) NOT NULL DEFAULT 0,
    put_away_quantity NUMERIC(12, 4) NOT NULL DEFAULT 0,
    notes TEXT,
    created_by TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_receipts_number ON receipts(receipt_number);
CREATE INDEX IF NOT EXISTS idx_receipts_status ON receipts(status);
CREATE INDEX IF NOT EXISTS idx_receipts_warehouse ON receipts(warehouse_id);
CREATE INDEX IF NOT EXISTS idx_receipts_supplier ON receipts(supplier_id);
CREATE INDEX IF NOT EXISTS idx_receipts_reference ON receipts(reference_type, reference_id);
CREATE INDEX IF NOT EXISTS idx_receipts_expected_date ON receipts(expected_date);

CREATE TABLE IF NOT EXISTS receipt_items (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    receipt_id UUID NOT NULL REFERENCES receipts(id) ON DELETE CASCADE,
    line_number INTEGER NOT NULL,
    sku TEXT NOT NULL,
    description TEXT,
    po_line_id UUID,
    expected_quantity NUMERIC(12, 4) NOT NULL DEFAULT 0,
    received_quantity NUMERIC(12, 4) NOT NULL DEFAULT 0,
    rejected_quantity NUMERIC(12, 4) NOT NULL DEFAULT 0,
    unit_cost NUMERIC(12, 4),
    lot_number TEXT,
    serial_numbers TEXT,
    expiration_date TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'pending',
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_receipt_items_receipt ON receipt_items(receipt_id);
CREATE INDEX IF NOT EXISTS idx_receipt_items_sku ON receipt_items(sku);
CREATE INDEX IF NOT EXISTS idx_receipt_items_po_line ON receipt_items(po_line_id);
CREATE INDEX IF NOT EXISTS idx_receipt_items_status ON receipt_items(status);

CREATE TABLE IF NOT EXISTS put_aways (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    receipt_id UUID NOT NULL REFERENCES receipts(id) ON DELETE CASCADE,
    receipt_item_id UUID NOT NULL REFERENCES receipt_items(id),
    sku TEXT NOT NULL,
    from_location_id INTEGER REFERENCES locations(id),
    to_location_id INTEGER NOT NULL REFERENCES locations(id),
    quantity NUMERIC(12, 4) NOT NULL,
    lot_id UUID,
    status TEXT NOT NULL DEFAULT 'pending',
    assigned_to TEXT,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_put_aways_receipt ON put_aways(receipt_id);
CREATE INDEX IF NOT EXISTS idx_put_aways_status ON put_aways(status);
CREATE INDEX IF NOT EXISTS idx_put_aways_assigned ON put_aways(assigned_to);
CREATE INDEX IF NOT EXISTS idx_put_aways_to_location ON put_aways(to_location_id);
