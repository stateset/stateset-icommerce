-- Credit Management tables

-- Credit accounts
CREATE TABLE IF NOT EXISTS credit_accounts (
    id TEXT PRIMARY KEY,
    customer_id TEXT UNIQUE NOT NULL,
    credit_limit TEXT NOT NULL DEFAULT '0',
    available_credit TEXT NOT NULL DEFAULT '0',
    current_balance TEXT NOT NULL DEFAULT '0',
    hold_amount TEXT NOT NULL DEFAULT '0',
    currency TEXT NOT NULL DEFAULT 'USD',
    status TEXT NOT NULL DEFAULT 'active',
    payment_terms TEXT,
    risk_rating TEXT,
    last_review_date TEXT,
    next_review_date TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_credit_accounts_customer ON credit_accounts(customer_id);
CREATE INDEX IF NOT EXISTS idx_credit_accounts_status ON credit_accounts(status);
CREATE INDEX IF NOT EXISTS idx_credit_accounts_risk ON credit_accounts(risk_rating);

-- Credit holds
CREATE TABLE IF NOT EXISTS credit_holds (
    id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL,
    order_id TEXT,
    hold_type TEXT NOT NULL,
    hold_amount TEXT NOT NULL DEFAULT '0',
    reason TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    placed_by TEXT,
    placed_at TEXT NOT NULL,
    released_by TEXT,
    released_at TEXT,
    release_notes TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (customer_id) REFERENCES credit_accounts(customer_id)
);

CREATE INDEX IF NOT EXISTS idx_credit_holds_customer ON credit_holds(customer_id);
CREATE INDEX IF NOT EXISTS idx_credit_holds_order ON credit_holds(order_id);
CREATE INDEX IF NOT EXISTS idx_credit_holds_status ON credit_holds(status);
CREATE INDEX IF NOT EXISTS idx_credit_holds_type ON credit_holds(hold_type);

-- Credit applications
CREATE TABLE IF NOT EXISTS credit_applications (
    id TEXT PRIMARY KEY,
    application_number TEXT UNIQUE NOT NULL,
    customer_id TEXT NOT NULL,
    requested_limit TEXT NOT NULL,
    approved_limit TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    business_name TEXT,
    tax_id TEXT,
    years_in_business INTEGER,
    annual_revenue TEXT,
    bank_reference TEXT,
    trade_references TEXT,
    submitted_at TEXT NOT NULL,
    reviewed_by TEXT,
    reviewed_at TEXT,
    decision_notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_credit_apps_number ON credit_applications(application_number);
CREATE INDEX IF NOT EXISTS idx_credit_apps_customer ON credit_applications(customer_id);
CREATE INDEX IF NOT EXISTS idx_credit_apps_status ON credit_applications(status);

-- Credit transactions
CREATE TABLE IF NOT EXISTS credit_transactions (
    id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL,
    transaction_type TEXT NOT NULL,
    amount TEXT NOT NULL,
    running_balance TEXT NOT NULL,
    reference_type TEXT,
    reference_id TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (customer_id) REFERENCES credit_accounts(customer_id)
);

CREATE INDEX IF NOT EXISTS idx_credit_tx_customer ON credit_transactions(customer_id);
CREATE INDEX IF NOT EXISTS idx_credit_tx_type ON credit_transactions(transaction_type);
CREATE INDEX IF NOT EXISTS idx_credit_tx_date ON credit_transactions(created_at);
CREATE INDEX IF NOT EXISTS idx_credit_tx_ref ON credit_transactions(reference_id);

-- Credit reservations (for order holds)
CREATE TABLE IF NOT EXISTS credit_reservations (
    id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL,
    order_id TEXT NOT NULL,
    amount TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    released_at TEXT,
    FOREIGN KEY (customer_id) REFERENCES credit_accounts(customer_id)
);

CREATE INDEX IF NOT EXISTS idx_credit_res_customer ON credit_reservations(customer_id);
CREATE INDEX IF NOT EXISTS idx_credit_res_order ON credit_reservations(order_id);
CREATE INDEX IF NOT EXISTS idx_credit_res_status ON credit_reservations(status);

-- Auto-update timestamps
CREATE TRIGGER IF NOT EXISTS credit_accounts_updated_at
AFTER UPDATE ON credit_accounts
BEGIN
    UPDATE credit_accounts SET updated_at = datetime('now') WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS credit_applications_updated_at
AFTER UPDATE ON credit_applications
BEGIN
    UPDATE credit_applications SET updated_at = datetime('now') WHERE id = NEW.id;
END;
