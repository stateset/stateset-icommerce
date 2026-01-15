-- Quality Control schema (PostgreSQL)

CREATE TABLE IF NOT EXISTS defect_codes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    category TEXT NOT NULL,
    severity TEXT NOT NULL DEFAULT 'minor',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_defect_codes_code ON defect_codes(code);
CREATE INDEX IF NOT EXISTS idx_defect_codes_category ON defect_codes(category);

CREATE TABLE IF NOT EXISTS inspections (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    inspection_number TEXT NOT NULL UNIQUE,
    inspection_type TEXT NOT NULL DEFAULT 'receiving',
    reference_type TEXT NOT NULL,
    reference_id UUID NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    inspector_id TEXT,
    scheduled_at TIMESTAMPTZ,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_inspections_number ON inspections(inspection_number);
CREATE INDEX IF NOT EXISTS idx_inspections_type ON inspections(inspection_type);
CREATE INDEX IF NOT EXISTS idx_inspections_status ON inspections(status);
CREATE INDEX IF NOT EXISTS idx_inspections_reference ON inspections(reference_type, reference_id);
CREATE INDEX IF NOT EXISTS idx_inspections_inspector ON inspections(inspector_id);
CREATE INDEX IF NOT EXISTS idx_inspections_scheduled ON inspections(scheduled_at);

CREATE TABLE IF NOT EXISTS inspection_items (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    inspection_id UUID NOT NULL REFERENCES inspections(id) ON DELETE CASCADE,
    sku TEXT NOT NULL,
    lot_number TEXT,
    serial_number TEXT,
    quantity_inspected NUMERIC(12, 4) NOT NULL DEFAULT 0,
    quantity_passed NUMERIC(12, 4) NOT NULL DEFAULT 0,
    quantity_failed NUMERIC(12, 4) NOT NULL DEFAULT 0,
    defect_codes JSONB NOT NULL DEFAULT '[]'::jsonb,
    measurements JSONB,
    result TEXT NOT NULL DEFAULT 'pending',
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_inspection_items_inspection ON inspection_items(inspection_id);
CREATE INDEX IF NOT EXISTS idx_inspection_items_sku ON inspection_items(sku);
CREATE INDEX IF NOT EXISTS idx_inspection_items_lot ON inspection_items(lot_number);
CREATE INDEX IF NOT EXISTS idx_inspection_items_result ON inspection_items(result);

CREATE TABLE IF NOT EXISTS non_conformances (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    ncr_number TEXT NOT NULL UNIQUE,
    inspection_id UUID REFERENCES inspections(id),
    source TEXT NOT NULL DEFAULT 'inspection',
    severity TEXT NOT NULL DEFAULT 'minor',
    status TEXT NOT NULL DEFAULT 'open',
    sku TEXT NOT NULL,
    lot_number TEXT,
    serial_number TEXT,
    quantity_affected NUMERIC(12, 4) NOT NULL DEFAULT 0,
    description TEXT NOT NULL,
    root_cause TEXT,
    corrective_action TEXT,
    preventive_action TEXT,
    disposition TEXT,
    disposition_quantity NUMERIC(12, 4),
    assigned_to TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    closed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_ncr_number ON non_conformances(ncr_number);
CREATE INDEX IF NOT EXISTS idx_ncr_inspection ON non_conformances(inspection_id);
CREATE INDEX IF NOT EXISTS idx_ncr_source ON non_conformances(source);
CREATE INDEX IF NOT EXISTS idx_ncr_severity ON non_conformances(severity);
CREATE INDEX IF NOT EXISTS idx_ncr_status ON non_conformances(status);
CREATE INDEX IF NOT EXISTS idx_ncr_sku ON non_conformances(sku);
CREATE INDEX IF NOT EXISTS idx_ncr_lot ON non_conformances(lot_number);
CREATE INDEX IF NOT EXISTS idx_ncr_assigned ON non_conformances(assigned_to);

CREATE TABLE IF NOT EXISTS quality_holds (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    sku TEXT NOT NULL,
    lot_number TEXT,
    serial_number TEXT,
    location_id INTEGER,
    quantity_held NUMERIC(12, 4) NOT NULL DEFAULT 0,
    reason TEXT NOT NULL,
    hold_type TEXT NOT NULL DEFAULT 'quality_inspection',
    ncr_id UUID REFERENCES non_conformances(id),
    inspection_id UUID REFERENCES inspections(id),
    placed_by TEXT NOT NULL,
    released_by TEXT,
    release_notes TEXT,
    placed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    released_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_holds_sku ON quality_holds(sku);
CREATE INDEX IF NOT EXISTS idx_holds_lot ON quality_holds(lot_number);
CREATE INDEX IF NOT EXISTS idx_holds_serial ON quality_holds(serial_number);
CREATE INDEX IF NOT EXISTS idx_holds_location ON quality_holds(location_id);
CREATE INDEX IF NOT EXISTS idx_holds_type ON quality_holds(hold_type);
CREATE INDEX IF NOT EXISTS idx_holds_ncr ON quality_holds(ncr_id);
CREATE INDEX IF NOT EXISTS idx_holds_active ON quality_holds(released_at);
