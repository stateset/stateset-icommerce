-- Lot/Batch Tracking Module
-- Lots, transactions, certificates, and location tracking

-- Lots table
CREATE TABLE IF NOT EXISTS lots (
    id TEXT PRIMARY KEY,
    lot_number TEXT NOT NULL UNIQUE,
    sku TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    quantity_produced TEXT NOT NULL DEFAULT '0',
    quantity_remaining TEXT NOT NULL DEFAULT '0',
    quantity_reserved TEXT NOT NULL DEFAULT '0',
    quantity_quarantined TEXT NOT NULL DEFAULT '0',
    production_date TEXT NOT NULL DEFAULT (datetime('now')),
    expiration_date TEXT,
    best_before_date TEXT,
    supplier_lot TEXT,
    supplier_id TEXT,
    work_order_id TEXT,
    purchase_order_id TEXT,
    cost_per_unit TEXT,
    attributes TEXT NOT NULL DEFAULT '{}',
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_lots_number ON lots(lot_number);
CREATE INDEX IF NOT EXISTS idx_lots_sku ON lots(sku);
CREATE INDEX IF NOT EXISTS idx_lots_status ON lots(status);
CREATE INDEX IF NOT EXISTS idx_lots_supplier ON lots(supplier_id);
CREATE INDEX IF NOT EXISTS idx_lots_work_order ON lots(work_order_id);
CREATE INDEX IF NOT EXISTS idx_lots_purchase_order ON lots(purchase_order_id);
CREATE INDEX IF NOT EXISTS idx_lots_expiration ON lots(expiration_date);
CREATE INDEX IF NOT EXISTS idx_lots_production ON lots(production_date);

-- Lot transactions
CREATE TABLE IF NOT EXISTS lot_transactions (
    id TEXT PRIMARY KEY,
    lot_id TEXT NOT NULL REFERENCES lots(id),
    transaction_type TEXT NOT NULL,
    quantity TEXT NOT NULL,
    reference_type TEXT NOT NULL,
    reference_id TEXT NOT NULL,
    from_location_id INTEGER,
    to_location_id INTEGER,
    reason TEXT,
    performed_by TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_lot_tx_lot ON lot_transactions(lot_id);
CREATE INDEX IF NOT EXISTS idx_lot_tx_type ON lot_transactions(transaction_type);
CREATE INDEX IF NOT EXISTS idx_lot_tx_reference ON lot_transactions(reference_type, reference_id);
CREATE INDEX IF NOT EXISTS idx_lot_tx_date ON lot_transactions(created_at);

-- Lot certificates
CREATE TABLE IF NOT EXISTS lot_certificates (
    id TEXT PRIMARY KEY,
    lot_id TEXT NOT NULL REFERENCES lots(id) ON DELETE CASCADE,
    certificate_type TEXT NOT NULL DEFAULT 'coa',
    certificate_number TEXT,
    document_url TEXT,
    issued_by TEXT,
    issued_at TEXT,
    expires_at TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_lot_certs_lot ON lot_certificates(lot_id);
CREATE INDEX IF NOT EXISTS idx_lot_certs_type ON lot_certificates(certificate_type);
CREATE INDEX IF NOT EXISTS idx_lot_certs_expires ON lot_certificates(expires_at);

-- Lot locations (quantity of lot at each location)
CREATE TABLE IF NOT EXISTS lot_locations (
    lot_id TEXT NOT NULL REFERENCES lots(id) ON DELETE CASCADE,
    location_id INTEGER NOT NULL,
    quantity TEXT NOT NULL DEFAULT '0',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (lot_id, location_id)
);

CREATE INDEX IF NOT EXISTS idx_lot_locations_lot ON lot_locations(lot_id);
CREATE INDEX IF NOT EXISTS idx_lot_locations_location ON lot_locations(location_id);

-- Lot reservations
CREATE TABLE IF NOT EXISTS lot_reservations (
    id TEXT PRIMARY KEY,
    lot_id TEXT NOT NULL REFERENCES lots(id),
    quantity TEXT NOT NULL,
    reference_type TEXT NOT NULL,
    reference_id TEXT NOT NULL,
    reserved_by TEXT,
    reserved_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT,
    confirmed_at TEXT,
    released_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_lot_reservations_lot ON lot_reservations(lot_id);
CREATE INDEX IF NOT EXISTS idx_lot_reservations_reference ON lot_reservations(reference_type, reference_id);
CREATE INDEX IF NOT EXISTS idx_lot_reservations_active ON lot_reservations(released_at, confirmed_at);
