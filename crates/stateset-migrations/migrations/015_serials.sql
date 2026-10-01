-- Serial Number Management Module
-- Serial numbers, history, and reservations

-- Serial numbers table
CREATE TABLE IF NOT EXISTS serial_numbers (
    id TEXT PRIMARY KEY,
    serial TEXT NOT NULL UNIQUE,
    sku TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'available',
    lot_id TEXT REFERENCES lots(id),
    lot_number TEXT,
    current_location_id INTEGER,
    current_owner_id TEXT,
    current_owner_type TEXT,
    warranty_id TEXT,
    manufactured_at TEXT,
    received_at TEXT,
    sold_at TEXT,
    activated_at TEXT,
    last_service_at TEXT,
    notes TEXT,
    attributes TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_serials_serial ON serial_numbers(serial);
CREATE INDEX IF NOT EXISTS idx_serials_sku ON serial_numbers(sku);
CREATE INDEX IF NOT EXISTS idx_serials_status ON serial_numbers(status);
CREATE INDEX IF NOT EXISTS idx_serials_lot ON serial_numbers(lot_id);
CREATE INDEX IF NOT EXISTS idx_serials_lot_number ON serial_numbers(lot_number);
CREATE INDEX IF NOT EXISTS idx_serials_location ON serial_numbers(current_location_id);
CREATE INDEX IF NOT EXISTS idx_serials_owner ON serial_numbers(current_owner_id, current_owner_type);
CREATE INDEX IF NOT EXISTS idx_serials_warranty ON serial_numbers(warranty_id);
CREATE INDEX IF NOT EXISTS idx_serials_manufactured ON serial_numbers(manufactured_at);
CREATE INDEX IF NOT EXISTS idx_serials_sold ON serial_numbers(sold_at);

-- Serial history
CREATE TABLE IF NOT EXISTS serial_history (
    id TEXT PRIMARY KEY,
    serial_id TEXT NOT NULL REFERENCES serial_numbers(id),
    event_type TEXT NOT NULL,
    reference_type TEXT,
    reference_id TEXT,
    from_status TEXT NOT NULL,
    to_status TEXT NOT NULL,
    from_location_id INTEGER,
    to_location_id INTEGER,
    from_owner_id TEXT,
    to_owner_id TEXT,
    performed_by TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_serial_history_serial ON serial_history(serial_id);
CREATE INDEX IF NOT EXISTS idx_serial_history_event ON serial_history(event_type);
CREATE INDEX IF NOT EXISTS idx_serial_history_reference ON serial_history(reference_type, reference_id);
CREATE INDEX IF NOT EXISTS idx_serial_history_date ON serial_history(created_at);

-- Serial reservations
CREATE TABLE IF NOT EXISTS serial_reservations (
    id TEXT PRIMARY KEY,
    serial_id TEXT NOT NULL REFERENCES serial_numbers(id),
    reference_type TEXT NOT NULL,
    reference_id TEXT NOT NULL,
    reserved_by TEXT,
    reserved_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT,
    confirmed_at TEXT,
    released_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_serial_reservations_serial ON serial_reservations(serial_id);
CREATE INDEX IF NOT EXISTS idx_serial_reservations_reference ON serial_reservations(reference_type, reference_id);
CREATE INDEX IF NOT EXISTS idx_serial_reservations_active ON serial_reservations(released_at, confirmed_at);
