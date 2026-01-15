-- General Ledger schema (PostgreSQL)

CREATE TABLE IF NOT EXISTS gl_accounts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    account_number TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    account_type TEXT NOT NULL,
    account_sub_type TEXT,
    parent_account_id UUID,
    is_header BOOLEAN NOT NULL DEFAULT FALSE,
    is_posting BOOLEAN NOT NULL DEFAULT TRUE,
    normal_balance TEXT NOT NULL DEFAULT 'debit',
    currency TEXT NOT NULL DEFAULT 'USD',
    status TEXT NOT NULL DEFAULT 'active',
    current_balance NUMERIC(14, 4) NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (parent_account_id) REFERENCES gl_accounts(id)
);

CREATE INDEX IF NOT EXISTS idx_gl_accounts_number ON gl_accounts(account_number);
CREATE INDEX IF NOT EXISTS idx_gl_accounts_type ON gl_accounts(account_type);
CREATE INDEX IF NOT EXISTS idx_gl_accounts_sub_type ON gl_accounts(account_sub_type);
CREATE INDEX IF NOT EXISTS idx_gl_accounts_parent ON gl_accounts(parent_account_id);
CREATE INDEX IF NOT EXISTS idx_gl_accounts_status ON gl_accounts(status);

CREATE TABLE IF NOT EXISTS gl_periods (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    period_name TEXT NOT NULL,
    fiscal_year INTEGER NOT NULL,
    period_number INTEGER NOT NULL,
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    status TEXT NOT NULL DEFAULT 'future',
    closed_at TIMESTAMPTZ,
    closed_by TEXT,
    locked_at TIMESTAMPTZ,
    locked_by TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (fiscal_year, period_number)
);

CREATE INDEX IF NOT EXISTS idx_gl_periods_year ON gl_periods(fiscal_year);
CREATE INDEX IF NOT EXISTS idx_gl_periods_status ON gl_periods(status);
CREATE INDEX IF NOT EXISTS idx_gl_periods_dates ON gl_periods(start_date, end_date);

CREATE TABLE IF NOT EXISTS gl_journal_entries (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entry_number TEXT UNIQUE NOT NULL,
    entry_date DATE NOT NULL,
    period_id UUID NOT NULL REFERENCES gl_periods(id),
    entry_type TEXT NOT NULL DEFAULT 'standard',
    source TEXT NOT NULL DEFAULT 'manual',
    source_document_type TEXT,
    source_document_id UUID,
    description TEXT NOT NULL,
    total_debits NUMERIC(14, 4) NOT NULL DEFAULT 0,
    total_credits NUMERIC(14, 4) NOT NULL DEFAULT 0,
    is_balanced BOOLEAN NOT NULL DEFAULT TRUE,
    status TEXT NOT NULL DEFAULT 'draft',
    posted_at TIMESTAMPTZ,
    posted_by TEXT,
    reversed_entry_id UUID,
    reversing_entry_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (reversed_entry_id) REFERENCES gl_journal_entries(id),
    FOREIGN KEY (reversing_entry_id) REFERENCES gl_journal_entries(id)
);

CREATE INDEX IF NOT EXISTS idx_gl_je_number ON gl_journal_entries(entry_number);
CREATE INDEX IF NOT EXISTS idx_gl_je_date ON gl_journal_entries(entry_date);
CREATE INDEX IF NOT EXISTS idx_gl_je_period ON gl_journal_entries(period_id);
CREATE INDEX IF NOT EXISTS idx_gl_je_status ON gl_journal_entries(status);
CREATE INDEX IF NOT EXISTS idx_gl_je_type ON gl_journal_entries(entry_type);
CREATE INDEX IF NOT EXISTS idx_gl_je_source ON gl_journal_entries(source);
CREATE INDEX IF NOT EXISTS idx_gl_je_source_doc ON gl_journal_entries(source_document_type, source_document_id);

CREATE TABLE IF NOT EXISTS gl_journal_entry_lines (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    journal_entry_id UUID NOT NULL REFERENCES gl_journal_entries(id) ON DELETE CASCADE,
    line_number INTEGER NOT NULL,
    account_id UUID NOT NULL REFERENCES gl_accounts(id),
    account_number TEXT,
    account_name TEXT,
    description TEXT,
    debit_amount NUMERIC(14, 4) NOT NULL DEFAULT 0,
    credit_amount NUMERIC(14, 4) NOT NULL DEFAULT 0,
    currency TEXT NOT NULL DEFAULT 'USD',
    reference_type TEXT,
    reference_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_gl_jel_entry ON gl_journal_entry_lines(journal_entry_id);
CREATE INDEX IF NOT EXISTS idx_gl_jel_account ON gl_journal_entry_lines(account_id);
CREATE INDEX IF NOT EXISTS idx_gl_jel_reference ON gl_journal_entry_lines(reference_type, reference_id);

CREATE TABLE IF NOT EXISTS gl_auto_posting_config (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    config_name TEXT NOT NULL,
    cash_account_id UUID NOT NULL REFERENCES gl_accounts(id),
    accounts_receivable_account_id UUID NOT NULL REFERENCES gl_accounts(id),
    inventory_account_id UUID NOT NULL REFERENCES gl_accounts(id),
    accounts_payable_account_id UUID NOT NULL REFERENCES gl_accounts(id),
    unearned_revenue_account_id UUID,
    sales_revenue_account_id UUID NOT NULL REFERENCES gl_accounts(id),
    shipping_revenue_account_id UUID,
    cogs_account_id UUID NOT NULL REFERENCES gl_accounts(id),
    bad_debt_expense_account_id UUID,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS gl_account_balances (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    account_id UUID NOT NULL REFERENCES gl_accounts(id),
    period_id UUID NOT NULL REFERENCES gl_periods(id),
    opening_balance NUMERIC(14, 4) NOT NULL DEFAULT 0,
    total_debits NUMERIC(14, 4) NOT NULL DEFAULT 0,
    total_credits NUMERIC(14, 4) NOT NULL DEFAULT 0,
    closing_balance NUMERIC(14, 4) NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (account_id, period_id)
);

CREATE INDEX IF NOT EXISTS idx_gl_balances_account ON gl_account_balances(account_id);
CREATE INDEX IF NOT EXISTS idx_gl_balances_period ON gl_account_balances(period_id);

CREATE OR REPLACE VIEW v_trial_balance AS
SELECT
    a.id AS account_id,
    a.account_number,
    a.name AS account_name,
    a.account_type,
    a.normal_balance,
    CASE WHEN a.normal_balance = 'debit' THEN a.current_balance ELSE 0 END AS debit_balance,
    CASE WHEN a.normal_balance = 'credit' THEN a.current_balance ELSE 0 END AS credit_balance
FROM gl_accounts a
WHERE a.is_posting = TRUE AND a.status = 'active'
ORDER BY a.account_number;
