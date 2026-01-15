-- Cost Accounting schema (PostgreSQL)

CREATE TABLE IF NOT EXISTS item_costs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    sku TEXT UNIQUE NOT NULL,
    cost_method TEXT NOT NULL DEFAULT 'average',
    standard_cost NUMERIC(12, 4) NOT NULL DEFAULT 0,
    average_cost NUMERIC(12, 4) NOT NULL DEFAULT 0,
    last_cost NUMERIC(12, 4) NOT NULL DEFAULT 0,
    material_cost NUMERIC(12, 4) NOT NULL DEFAULT 0,
    labor_cost NUMERIC(12, 4) NOT NULL DEFAULT 0,
    overhead_cost NUMERIC(12, 4) NOT NULL DEFAULT 0,
    currency TEXT NOT NULL DEFAULT 'USD',
    effective_date DATE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_item_costs_sku ON item_costs(sku);
CREATE INDEX IF NOT EXISTS idx_item_costs_method ON item_costs(cost_method);

CREATE TABLE IF NOT EXISTS cost_layers (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    sku TEXT NOT NULL,
    layer_date DATE NOT NULL,
    quantity NUMERIC(12, 4) NOT NULL,
    remaining_quantity NUMERIC(12, 4) NOT NULL,
    unit_cost NUMERIC(12, 4) NOT NULL,
    total_cost NUMERIC(12, 4) NOT NULL,
    source_type TEXT NOT NULL DEFAULT 'purchase',
    source_id UUID,
    lot_id UUID,
    location_id INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cost_layers_sku ON cost_layers(sku);
CREATE INDEX IF NOT EXISTS idx_cost_layers_date ON cost_layers(layer_date);
CREATE INDEX IF NOT EXISTS idx_cost_layers_remaining ON cost_layers(remaining_quantity);
CREATE INDEX IF NOT EXISTS idx_cost_layers_source ON cost_layers(source_id);

CREATE TABLE IF NOT EXISTS cost_transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    sku TEXT NOT NULL,
    transaction_type TEXT NOT NULL,
    quantity NUMERIC(12, 4) NOT NULL,
    unit_cost NUMERIC(12, 4) NOT NULL,
    total_cost NUMERIC(12, 4) NOT NULL,
    layer_id UUID REFERENCES cost_layers(id),
    reference_type TEXT,
    reference_id UUID,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cost_transactions_sku ON cost_transactions(sku);
CREATE INDEX IF NOT EXISTS idx_cost_transactions_type ON cost_transactions(transaction_type);
CREATE INDEX IF NOT EXISTS idx_cost_transactions_date ON cost_transactions(created_at);
CREATE INDEX IF NOT EXISTS idx_cost_transactions_layer ON cost_transactions(layer_id);
CREATE INDEX IF NOT EXISTS idx_cost_transactions_ref ON cost_transactions(reference_id);

CREATE TABLE IF NOT EXISTS cost_variances (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    sku TEXT NOT NULL,
    variance_type TEXT NOT NULL,
    variance_date DATE NOT NULL,
    standard_cost NUMERIC(12, 4) NOT NULL,
    actual_cost NUMERIC(12, 4) NOT NULL,
    variance_amount NUMERIC(12, 4) NOT NULL,
    variance_percent NUMERIC(12, 6) NOT NULL,
    quantity NUMERIC(12, 4) NOT NULL,
    total_variance NUMERIC(12, 4) NOT NULL,
    reference_type TEXT,
    reference_id UUID,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cost_variances_sku ON cost_variances(sku);
CREATE INDEX IF NOT EXISTS idx_cost_variances_type ON cost_variances(variance_type);
CREATE INDEX IF NOT EXISTS idx_cost_variances_date ON cost_variances(variance_date);

CREATE TABLE IF NOT EXISTS cost_adjustments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    adjustment_number TEXT UNIQUE NOT NULL,
    sku TEXT NOT NULL,
    adjustment_type TEXT NOT NULL,
    previous_cost NUMERIC(12, 4) NOT NULL,
    new_cost NUMERIC(12, 4) NOT NULL,
    adjustment_amount NUMERIC(12, 4) NOT NULL,
    reason TEXT NOT NULL,
    approved_by TEXT,
    approved_at TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'pending',
    created_by TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cost_adjustments_number ON cost_adjustments(adjustment_number);
CREATE INDEX IF NOT EXISTS idx_cost_adjustments_sku ON cost_adjustments(sku);
CREATE INDEX IF NOT EXISTS idx_cost_adjustments_status ON cost_adjustments(status);
CREATE INDEX IF NOT EXISTS idx_cost_adjustments_date ON cost_adjustments(created_at);

CREATE TABLE IF NOT EXISTS cost_rollups (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    sku TEXT NOT NULL,
    bom_id UUID,
    rollup_date DATE NOT NULL,
    material_cost NUMERIC(12, 4) NOT NULL DEFAULT 0,
    labor_cost NUMERIC(12, 4) NOT NULL DEFAULT 0,
    overhead_cost NUMERIC(12, 4) NOT NULL DEFAULT 0,
    total_cost NUMERIC(12, 4) NOT NULL DEFAULT 0,
    previous_cost NUMERIC(12, 4) NOT NULL DEFAULT 0,
    cost_change NUMERIC(12, 4) NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cost_rollups_sku ON cost_rollups(sku);
CREATE INDEX IF NOT EXISTS idx_cost_rollups_bom ON cost_rollups(bom_id);
CREATE INDEX IF NOT EXISTS idx_cost_rollups_date ON cost_rollups(rollup_date);
