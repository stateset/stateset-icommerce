-- Fixed asset register + revenue recognition (ASC 606).
--
-- Repositories: crates/stateset-db/src/postgres/fixed_assets.rs
--               crates/stateset-db/src/postgres/revenue_recognition.rs

CREATE TABLE IF NOT EXISTS fixed_assets (
    id UUID PRIMARY KEY,
    asset_number TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    category TEXT NOT NULL,
    acquisition_date DATE NOT NULL,
    acquisition_cost NUMERIC(20, 6) NOT NULL,
    salvage_value NUMERIC(20, 6) NOT NULL DEFAULT 0,
    useful_life_months INTEGER NOT NULL,
    depreciation_method TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    in_service_date DATE,
    location_id UUID,
    asset_account_id UUID,
    accumulated_depreciation_account_id UUID,
    depreciation_expense_account_id UUID,
    accumulated_depreciation NUMERIC(20, 6) NOT NULL DEFAULT 0,
    currency TEXT NOT NULL DEFAULT 'USD',
    disposal_date DATE,
    disposal_proceeds NUMERIC(20, 6),
    disposal_book_value NUMERIC(20, 6),
    disposal_gain_loss NUMERIC(20, 6),
    disposal_notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_fixed_assets_status ON fixed_assets(status);
CREATE INDEX IF NOT EXISTS idx_fixed_assets_category ON fixed_assets(category);

CREATE TABLE IF NOT EXISTS fixed_asset_depreciation_entries (
    asset_id UUID NOT NULL REFERENCES fixed_assets(id) ON DELETE CASCADE,
    period INTEGER NOT NULL,
    amount NUMERIC(20, 6) NOT NULL,
    accumulated NUMERIC(20, 6) NOT NULL,
    book_value NUMERIC(20, 6) NOT NULL,
    status TEXT NOT NULL DEFAULT 'scheduled',
    PRIMARY KEY (asset_id, period)
);

CREATE TABLE IF NOT EXISTS revenue_contracts (
    id UUID PRIMARY KEY,
    contract_number TEXT NOT NULL,
    customer_id UUID NOT NULL,
    order_id UUID,
    invoice_id UUID,
    transaction_price NUMERIC(20, 6) NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    status TEXT NOT NULL DEFAULT 'draft',
    effective_date DATE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_revenue_contracts_customer ON revenue_contracts(customer_id);
CREATE INDEX IF NOT EXISTS idx_revenue_contracts_status ON revenue_contracts(status);

CREATE TABLE IF NOT EXISTS performance_obligations (
    id UUID PRIMARY KEY,
    contract_id UUID NOT NULL REFERENCES revenue_contracts(id) ON DELETE CASCADE,
    description TEXT NOT NULL,
    standalone_selling_price NUMERIC(20, 6),
    allocated_amount NUMERIC(20, 6) NOT NULL,
    recognition_method TEXT NOT NULL,
    recognized_amount NUMERIC(20, 6) NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_performance_obligations_contract ON performance_obligations(contract_id);

CREATE TABLE IF NOT EXISTS revenue_schedule_entries (
    obligation_id UUID NOT NULL REFERENCES performance_obligations(id) ON DELETE CASCADE,
    period INTEGER NOT NULL,
    period_start DATE NOT NULL,
    amount NUMERIC(20, 6) NOT NULL,
    status TEXT NOT NULL DEFAULT 'deferred',
    PRIMARY KEY (obligation_id, period)
);
