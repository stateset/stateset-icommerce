-- Cost Accounting tables

-- Item costs (standard cost master)
CREATE TABLE IF NOT EXISTS item_costs (
    id TEXT PRIMARY KEY,
    sku TEXT UNIQUE NOT NULL,
    cost_method TEXT NOT NULL DEFAULT 'average',
    standard_cost TEXT NOT NULL DEFAULT '0',
    average_cost TEXT NOT NULL DEFAULT '0',
    last_cost TEXT NOT NULL DEFAULT '0',
    material_cost TEXT NOT NULL DEFAULT '0',
    labor_cost TEXT NOT NULL DEFAULT '0',
    overhead_cost TEXT NOT NULL DEFAULT '0',
    currency TEXT NOT NULL DEFAULT 'USD',
    effective_date TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_item_costs_sku ON item_costs(sku);
CREATE INDEX IF NOT EXISTS idx_item_costs_method ON item_costs(cost_method);

-- Cost layers (for FIFO/LIFO costing)
CREATE TABLE IF NOT EXISTS cost_layers (
    id TEXT PRIMARY KEY,
    sku TEXT NOT NULL,
    layer_date TEXT NOT NULL,
    quantity TEXT NOT NULL,
    remaining_quantity TEXT NOT NULL,
    unit_cost TEXT NOT NULL,
    total_cost TEXT NOT NULL,
    source_type TEXT NOT NULL DEFAULT 'purchase',
    source_id TEXT,
    lot_id TEXT,
    location_id INTEGER,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cost_layers_sku ON cost_layers(sku);
CREATE INDEX IF NOT EXISTS idx_cost_layers_date ON cost_layers(layer_date);
CREATE INDEX IF NOT EXISTS idx_cost_layers_remaining ON cost_layers(remaining_quantity);
CREATE INDEX IF NOT EXISTS idx_cost_layers_source ON cost_layers(source_id);

-- Cost transactions
CREATE TABLE IF NOT EXISTS cost_transactions (
    id TEXT PRIMARY KEY,
    sku TEXT NOT NULL,
    transaction_type TEXT NOT NULL,
    quantity TEXT NOT NULL,
    unit_cost TEXT NOT NULL,
    total_cost TEXT NOT NULL,
    layer_id TEXT,
    reference_type TEXT,
    reference_id TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (layer_id) REFERENCES cost_layers(id)
);

CREATE INDEX IF NOT EXISTS idx_cost_transactions_sku ON cost_transactions(sku);
CREATE INDEX IF NOT EXISTS idx_cost_transactions_type ON cost_transactions(transaction_type);
CREATE INDEX IF NOT EXISTS idx_cost_transactions_date ON cost_transactions(created_at);
CREATE INDEX IF NOT EXISTS idx_cost_transactions_layer ON cost_transactions(layer_id);
CREATE INDEX IF NOT EXISTS idx_cost_transactions_ref ON cost_transactions(reference_id);

-- Cost variances
CREATE TABLE IF NOT EXISTS cost_variances (
    id TEXT PRIMARY KEY,
    sku TEXT NOT NULL,
    variance_type TEXT NOT NULL,
    variance_date TEXT NOT NULL,
    standard_cost TEXT NOT NULL,
    actual_cost TEXT NOT NULL,
    variance_amount TEXT NOT NULL,
    variance_percent TEXT NOT NULL,
    quantity TEXT NOT NULL,
    total_variance TEXT NOT NULL,
    reference_type TEXT,
    reference_id TEXT,
    notes TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cost_variances_sku ON cost_variances(sku);
CREATE INDEX IF NOT EXISTS idx_cost_variances_type ON cost_variances(variance_type);
CREATE INDEX IF NOT EXISTS idx_cost_variances_date ON cost_variances(variance_date);

-- Cost adjustments
CREATE TABLE IF NOT EXISTS cost_adjustments (
    id TEXT PRIMARY KEY,
    adjustment_number TEXT UNIQUE NOT NULL,
    sku TEXT NOT NULL,
    adjustment_type TEXT NOT NULL,
    previous_cost TEXT NOT NULL,
    new_cost TEXT NOT NULL,
    adjustment_amount TEXT NOT NULL,
    reason TEXT NOT NULL,
    approved_by TEXT,
    approved_at TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    created_by TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cost_adjustments_number ON cost_adjustments(adjustment_number);
CREATE INDEX IF NOT EXISTS idx_cost_adjustments_sku ON cost_adjustments(sku);
CREATE INDEX IF NOT EXISTS idx_cost_adjustments_status ON cost_adjustments(status);
CREATE INDEX IF NOT EXISTS idx_cost_adjustments_date ON cost_adjustments(created_at);

-- Cost rollups (for manufactured items)
CREATE TABLE IF NOT EXISTS cost_rollups (
    id TEXT PRIMARY KEY,
    sku TEXT NOT NULL,
    bom_id TEXT,
    rollup_date TEXT NOT NULL,
    material_cost TEXT NOT NULL DEFAULT '0',
    labor_cost TEXT NOT NULL DEFAULT '0',
    overhead_cost TEXT NOT NULL DEFAULT '0',
    total_cost TEXT NOT NULL DEFAULT '0',
    previous_cost TEXT NOT NULL DEFAULT '0',
    cost_change TEXT NOT NULL DEFAULT '0',
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cost_rollups_sku ON cost_rollups(sku);
CREATE INDEX IF NOT EXISTS idx_cost_rollups_bom ON cost_rollups(bom_id);
CREATE INDEX IF NOT EXISTS idx_cost_rollups_date ON cost_rollups(rollup_date);

-- Auto-update timestamps
CREATE TRIGGER IF NOT EXISTS item_costs_updated_at
AFTER UPDATE ON item_costs
BEGIN
    UPDATE item_costs SET updated_at = datetime('now') WHERE id = NEW.id;
END;
