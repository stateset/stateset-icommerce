-- Fixed asset register + revenue recognition (ASC 606).
--
-- Repositories: crates/stateset-db/src/sqlite/fixed_assets.rs
--               crates/stateset-db/src/sqlite/revenue_recognition.rs
-- REST:         crates/stateset-http/src/routes/fixed_assets.rs
--               crates/stateset-http/src/routes/revenue_recognition.rs
--
-- Money/decimals stored as TEXT; dates as ISO 'YYYY-MM-DD' TEXT;
-- timestamps RFC3339 TEXT; tagged enums (depreciation/recognition methods) as JSON TEXT.

CREATE TABLE IF NOT EXISTS fixed_assets (
    id TEXT PRIMARY KEY,
    asset_number TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    category TEXT NOT NULL,
    acquisition_date TEXT NOT NULL,
    acquisition_cost TEXT NOT NULL,
    salvage_value TEXT NOT NULL DEFAULT '0',
    useful_life_months INTEGER NOT NULL,
    depreciation_method TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    in_service_date TEXT,
    location_id TEXT,
    asset_account_id TEXT,
    accumulated_depreciation_account_id TEXT,
    depreciation_expense_account_id TEXT,
    accumulated_depreciation TEXT NOT NULL DEFAULT '0',
    currency TEXT NOT NULL DEFAULT 'USD',
    disposal_date TEXT,
    disposal_proceeds TEXT,
    disposal_book_value TEXT,
    disposal_gain_loss TEXT,
    disposal_notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_fixed_assets_status ON fixed_assets(status);
CREATE INDEX IF NOT EXISTS idx_fixed_assets_category ON fixed_assets(category);

CREATE TABLE IF NOT EXISTS fixed_asset_depreciation_entries (
    asset_id TEXT NOT NULL REFERENCES fixed_assets(id) ON DELETE CASCADE,
    period INTEGER NOT NULL,
    amount TEXT NOT NULL,
    accumulated TEXT NOT NULL,
    book_value TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'scheduled',
    PRIMARY KEY (asset_id, period)
);

CREATE TABLE IF NOT EXISTS revenue_contracts (
    id TEXT PRIMARY KEY,
    contract_number TEXT NOT NULL,
    customer_id TEXT NOT NULL,
    order_id TEXT,
    invoice_id TEXT,
    transaction_price TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    status TEXT NOT NULL DEFAULT 'draft',
    effective_date TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_revenue_contracts_customer ON revenue_contracts(customer_id);
CREATE INDEX IF NOT EXISTS idx_revenue_contracts_status ON revenue_contracts(status);

CREATE TABLE IF NOT EXISTS performance_obligations (
    id TEXT PRIMARY KEY,
    contract_id TEXT NOT NULL REFERENCES revenue_contracts(id) ON DELETE CASCADE,
    description TEXT NOT NULL,
    standalone_selling_price TEXT,
    allocated_amount TEXT NOT NULL,
    recognition_method TEXT NOT NULL,
    recognized_amount TEXT NOT NULL DEFAULT '0',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_performance_obligations_contract ON performance_obligations(contract_id);

CREATE TABLE IF NOT EXISTS revenue_schedule_entries (
    obligation_id TEXT NOT NULL REFERENCES performance_obligations(id) ON DELETE CASCADE,
    period INTEGER NOT NULL,
    period_start TEXT NOT NULL,
    amount TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'deferred',
    PRIMARY KEY (obligation_id, period)
);
