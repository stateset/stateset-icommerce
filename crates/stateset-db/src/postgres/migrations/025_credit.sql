-- Credit Management schema (PostgreSQL)

CREATE TABLE IF NOT EXISTS credit_accounts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_id UUID UNIQUE NOT NULL,
    credit_limit NUMERIC(12, 4) NOT NULL DEFAULT 0,
    available_credit NUMERIC(12, 4) NOT NULL DEFAULT 0,
    current_balance NUMERIC(12, 4) NOT NULL DEFAULT 0,
    hold_amount NUMERIC(12, 4) NOT NULL DEFAULT 0,
    currency TEXT NOT NULL DEFAULT 'USD',
    status TEXT NOT NULL DEFAULT 'active',
    payment_terms TEXT,
    risk_rating TEXT,
    last_review_date DATE,
    next_review_date DATE,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_credit_accounts_customer ON credit_accounts(customer_id);
CREATE INDEX IF NOT EXISTS idx_credit_accounts_status ON credit_accounts(status);
CREATE INDEX IF NOT EXISTS idx_credit_accounts_risk ON credit_accounts(risk_rating);

CREATE TABLE IF NOT EXISTS credit_holds (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_id UUID NOT NULL REFERENCES credit_accounts(customer_id),
    order_id UUID,
    hold_type TEXT NOT NULL,
    hold_amount NUMERIC(12, 4) NOT NULL DEFAULT 0,
    reason TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    placed_by TEXT,
    placed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    released_by TEXT,
    released_at TIMESTAMPTZ,
    release_notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_credit_holds_customer ON credit_holds(customer_id);
CREATE INDEX IF NOT EXISTS idx_credit_holds_order ON credit_holds(order_id);
CREATE INDEX IF NOT EXISTS idx_credit_holds_status ON credit_holds(status);
CREATE INDEX IF NOT EXISTS idx_credit_holds_type ON credit_holds(hold_type);

CREATE TABLE IF NOT EXISTS credit_applications (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    application_number TEXT UNIQUE NOT NULL,
    customer_id UUID NOT NULL,
    requested_limit NUMERIC(12, 4) NOT NULL,
    approved_limit NUMERIC(12, 4),
    status TEXT NOT NULL DEFAULT 'pending',
    business_name TEXT,
    tax_id TEXT,
    years_in_business INTEGER,
    annual_revenue NUMERIC(12, 4),
    bank_reference TEXT,
    trade_references TEXT,
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reviewed_by TEXT,
    reviewed_at TIMESTAMPTZ,
    decision_notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_credit_apps_number ON credit_applications(application_number);
CREATE INDEX IF NOT EXISTS idx_credit_apps_customer ON credit_applications(customer_id);
CREATE INDEX IF NOT EXISTS idx_credit_apps_status ON credit_applications(status);

CREATE TABLE IF NOT EXISTS credit_transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_id UUID NOT NULL REFERENCES credit_accounts(customer_id),
    transaction_type TEXT NOT NULL,
    amount NUMERIC(12, 4) NOT NULL,
    running_balance NUMERIC(12, 4) NOT NULL,
    reference_type TEXT,
    reference_id UUID,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_credit_tx_customer ON credit_transactions(customer_id);
CREATE INDEX IF NOT EXISTS idx_credit_tx_type ON credit_transactions(transaction_type);
CREATE INDEX IF NOT EXISTS idx_credit_tx_date ON credit_transactions(created_at);
CREATE INDEX IF NOT EXISTS idx_credit_tx_ref ON credit_transactions(reference_id);

CREATE TABLE IF NOT EXISTS credit_reservations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_id UUID NOT NULL REFERENCES credit_accounts(customer_id),
    order_id UUID NOT NULL,
    amount NUMERIC(12, 4) NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    released_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_credit_res_customer ON credit_reservations(customer_id);
CREATE INDEX IF NOT EXISTS idx_credit_res_order ON credit_reservations(order_id);
CREATE INDEX IF NOT EXISTS idx_credit_res_status ON credit_reservations(status);
