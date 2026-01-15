-- Lot/Batch Tracking schema (PostgreSQL)

CREATE TABLE IF NOT EXISTS lots (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    lot_number TEXT NOT NULL UNIQUE,
    sku TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    quantity_produced NUMERIC(12, 4) NOT NULL DEFAULT 0,
    quantity_remaining NUMERIC(12, 4) NOT NULL DEFAULT 0,
    quantity_reserved NUMERIC(12, 4) NOT NULL DEFAULT 0,
    quantity_quarantined NUMERIC(12, 4) NOT NULL DEFAULT 0,
    production_date TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expiration_date TIMESTAMPTZ,
    best_before_date TIMESTAMPTZ,
    supplier_lot TEXT,
    supplier_id UUID,
    work_order_id UUID,
    purchase_order_id UUID,
    cost_per_unit NUMERIC(12, 4),
    attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_lots_number ON lots(lot_number);
CREATE INDEX IF NOT EXISTS idx_lots_sku ON lots(sku);
CREATE INDEX IF NOT EXISTS idx_lots_status ON lots(status);
CREATE INDEX IF NOT EXISTS idx_lots_supplier ON lots(supplier_id);
CREATE INDEX IF NOT EXISTS idx_lots_work_order ON lots(work_order_id);
CREATE INDEX IF NOT EXISTS idx_lots_purchase_order ON lots(purchase_order_id);
CREATE INDEX IF NOT EXISTS idx_lots_expiration ON lots(expiration_date);
CREATE INDEX IF NOT EXISTS idx_lots_production ON lots(production_date);

CREATE TABLE IF NOT EXISTS lot_transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    lot_id UUID NOT NULL REFERENCES lots(id),
    transaction_type TEXT NOT NULL,
    quantity NUMERIC(12, 4) NOT NULL,
    reference_type TEXT NOT NULL,
    reference_id UUID NOT NULL,
    from_location_id INTEGER,
    to_location_id INTEGER,
    reason TEXT,
    performed_by TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_lot_tx_lot ON lot_transactions(lot_id);
CREATE INDEX IF NOT EXISTS idx_lot_tx_type ON lot_transactions(transaction_type);
CREATE INDEX IF NOT EXISTS idx_lot_tx_reference ON lot_transactions(reference_type, reference_id);
CREATE INDEX IF NOT EXISTS idx_lot_tx_date ON lot_transactions(created_at);

CREATE TABLE IF NOT EXISTS lot_certificates (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    lot_id UUID NOT NULL REFERENCES lots(id) ON DELETE CASCADE,
    certificate_type TEXT NOT NULL DEFAULT 'coa',
    certificate_number TEXT,
    document_url TEXT,
    issued_by TEXT,
    issued_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_lot_certs_lot ON lot_certificates(lot_id);
CREATE INDEX IF NOT EXISTS idx_lot_certs_type ON lot_certificates(certificate_type);
CREATE INDEX IF NOT EXISTS idx_lot_certs_expires ON lot_certificates(expires_at);

CREATE TABLE IF NOT EXISTS lot_locations (
    lot_id UUID NOT NULL REFERENCES lots(id) ON DELETE CASCADE,
    location_id INTEGER NOT NULL,
    quantity NUMERIC(12, 4) NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (lot_id, location_id)
);

CREATE INDEX IF NOT EXISTS idx_lot_locations_lot ON lot_locations(lot_id);
CREATE INDEX IF NOT EXISTS idx_lot_locations_location ON lot_locations(location_id);

CREATE TABLE IF NOT EXISTS lot_reservations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    lot_id UUID NOT NULL REFERENCES lots(id),
    quantity NUMERIC(12, 4) NOT NULL,
    reference_type TEXT NOT NULL,
    reference_id UUID NOT NULL,
    reserved_by TEXT,
    reserved_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    confirmed_at TIMESTAMPTZ,
    released_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_lot_reservations_lot ON lot_reservations(lot_id);
CREATE INDEX IF NOT EXISTS idx_lot_reservations_reference ON lot_reservations(reference_type, reference_id);
CREATE INDEX IF NOT EXISTS idx_lot_reservations_active ON lot_reservations(released_at, confirmed_at);
