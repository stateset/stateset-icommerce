-- Serial Number Management schema (PostgreSQL)

CREATE TABLE IF NOT EXISTS serial_numbers (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    serial TEXT NOT NULL UNIQUE,
    sku TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'available',
    lot_id UUID REFERENCES lots(id),
    lot_number TEXT,
    current_location_id INTEGER,
    current_owner_id UUID,
    current_owner_type TEXT,
    warranty_id UUID,
    manufactured_at TIMESTAMPTZ,
    received_at TIMESTAMPTZ,
    sold_at TIMESTAMPTZ,
    activated_at TIMESTAMPTZ,
    last_service_at TIMESTAMPTZ,
    notes TEXT,
    attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
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

CREATE TABLE IF NOT EXISTS serial_history (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    serial_id UUID NOT NULL REFERENCES serial_numbers(id),
    event_type TEXT NOT NULL,
    reference_type TEXT,
    reference_id UUID,
    from_status TEXT NOT NULL,
    to_status TEXT NOT NULL,
    from_location_id INTEGER,
    to_location_id INTEGER,
    from_owner_id UUID,
    to_owner_id UUID,
    performed_by TEXT,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_serial_history_serial ON serial_history(serial_id);
CREATE INDEX IF NOT EXISTS idx_serial_history_event ON serial_history(event_type);
CREATE INDEX IF NOT EXISTS idx_serial_history_reference ON serial_history(reference_type, reference_id);
CREATE INDEX IF NOT EXISTS idx_serial_history_date ON serial_history(created_at);

CREATE TABLE IF NOT EXISTS serial_reservations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    serial_id UUID NOT NULL REFERENCES serial_numbers(id),
    reference_type TEXT NOT NULL,
    reference_id UUID NOT NULL,
    reserved_by TEXT,
    reserved_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    confirmed_at TIMESTAMPTZ,
    released_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_serial_reservations_serial ON serial_reservations(serial_id);
CREATE INDEX IF NOT EXISTS idx_serial_reservations_reference ON serial_reservations(reference_type, reference_id);
CREATE INDEX IF NOT EXISTS idx_serial_reservations_active ON serial_reservations(released_at, confirmed_at);
