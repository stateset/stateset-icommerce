-- General Ledger tables

-- Chart of Accounts
CREATE TABLE IF NOT EXISTS gl_accounts (
    id TEXT PRIMARY KEY,
    account_number TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    account_type TEXT NOT NULL,  -- asset, liability, equity, revenue, expense
    account_sub_type TEXT,
    parent_account_id TEXT,
    is_header INTEGER NOT NULL DEFAULT 0,
    is_posting INTEGER NOT NULL DEFAULT 1,
    normal_balance TEXT NOT NULL DEFAULT 'debit',  -- debit or credit
    currency TEXT NOT NULL DEFAULT 'USD',
    status TEXT NOT NULL DEFAULT 'active',
    current_balance TEXT NOT NULL DEFAULT '0',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (parent_account_id) REFERENCES gl_accounts(id)
);

CREATE INDEX IF NOT EXISTS idx_gl_accounts_number ON gl_accounts(account_number);
CREATE INDEX IF NOT EXISTS idx_gl_accounts_type ON gl_accounts(account_type);
CREATE INDEX IF NOT EXISTS idx_gl_accounts_sub_type ON gl_accounts(account_sub_type);
CREATE INDEX IF NOT EXISTS idx_gl_accounts_parent ON gl_accounts(parent_account_id);
CREATE INDEX IF NOT EXISTS idx_gl_accounts_status ON gl_accounts(status);

-- GL Periods
CREATE TABLE IF NOT EXISTS gl_periods (
    id TEXT PRIMARY KEY,
    period_name TEXT NOT NULL,
    fiscal_year INTEGER NOT NULL,
    period_number INTEGER NOT NULL,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'future',  -- future, open, closed, locked
    closed_at TEXT,
    closed_by TEXT,
    locked_at TEXT,
    locked_by TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(fiscal_year, period_number)
);

CREATE INDEX IF NOT EXISTS idx_gl_periods_year ON gl_periods(fiscal_year);
CREATE INDEX IF NOT EXISTS idx_gl_periods_status ON gl_periods(status);
CREATE INDEX IF NOT EXISTS idx_gl_periods_dates ON gl_periods(start_date, end_date);

-- Journal Entries (header)
CREATE TABLE IF NOT EXISTS gl_journal_entries (
    id TEXT PRIMARY KEY,
    entry_number TEXT UNIQUE NOT NULL,
    entry_date TEXT NOT NULL,
    period_id TEXT NOT NULL,
    entry_type TEXT NOT NULL DEFAULT 'standard',
    source TEXT NOT NULL DEFAULT 'manual',
    source_document_type TEXT,
    source_document_id TEXT,
    description TEXT NOT NULL,
    total_debits TEXT NOT NULL DEFAULT '0',
    total_credits TEXT NOT NULL DEFAULT '0',
    is_balanced INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'draft',
    posted_at TEXT,
    posted_by TEXT,
    reversed_entry_id TEXT,
    reversing_entry_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (period_id) REFERENCES gl_periods(id),
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

-- Journal Entry Lines (detail)
CREATE TABLE IF NOT EXISTS gl_journal_entry_lines (
    id TEXT PRIMARY KEY,
    journal_entry_id TEXT NOT NULL,
    line_number INTEGER NOT NULL,
    account_id TEXT NOT NULL,
    account_number TEXT,
    account_name TEXT,
    description TEXT,
    debit_amount TEXT NOT NULL DEFAULT '0',
    credit_amount TEXT NOT NULL DEFAULT '0',
    currency TEXT NOT NULL DEFAULT 'USD',
    reference_type TEXT,
    reference_id TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (journal_entry_id) REFERENCES gl_journal_entries(id) ON DELETE CASCADE,
    FOREIGN KEY (account_id) REFERENCES gl_accounts(id)
);

CREATE INDEX IF NOT EXISTS idx_gl_jel_entry ON gl_journal_entry_lines(journal_entry_id);
CREATE INDEX IF NOT EXISTS idx_gl_jel_account ON gl_journal_entry_lines(account_id);
CREATE INDEX IF NOT EXISTS idx_gl_jel_reference ON gl_journal_entry_lines(reference_type, reference_id);

-- Auto-posting configuration
CREATE TABLE IF NOT EXISTS gl_auto_posting_config (
    id TEXT PRIMARY KEY,
    config_name TEXT NOT NULL,
    cash_account_id TEXT NOT NULL,
    accounts_receivable_account_id TEXT NOT NULL,
    inventory_account_id TEXT NOT NULL,
    accounts_payable_account_id TEXT NOT NULL,
    unearned_revenue_account_id TEXT,
    sales_revenue_account_id TEXT NOT NULL,
    shipping_revenue_account_id TEXT,
    cogs_account_id TEXT NOT NULL,
    bad_debt_expense_account_id TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (cash_account_id) REFERENCES gl_accounts(id),
    FOREIGN KEY (accounts_receivable_account_id) REFERENCES gl_accounts(id),
    FOREIGN KEY (inventory_account_id) REFERENCES gl_accounts(id),
    FOREIGN KEY (accounts_payable_account_id) REFERENCES gl_accounts(id),
    FOREIGN KEY (sales_revenue_account_id) REFERENCES gl_accounts(id),
    FOREIGN KEY (cogs_account_id) REFERENCES gl_accounts(id)
);

-- Account balance history (for period-end snapshots)
CREATE TABLE IF NOT EXISTS gl_account_balances (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    period_id TEXT NOT NULL,
    opening_balance TEXT NOT NULL DEFAULT '0',
    total_debits TEXT NOT NULL DEFAULT '0',
    total_credits TEXT NOT NULL DEFAULT '0',
    closing_balance TEXT NOT NULL DEFAULT '0',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(account_id, period_id),
    FOREIGN KEY (account_id) REFERENCES gl_accounts(id),
    FOREIGN KEY (period_id) REFERENCES gl_periods(id)
);

CREATE INDEX IF NOT EXISTS idx_gl_balances_account ON gl_account_balances(account_id);
CREATE INDEX IF NOT EXISTS idx_gl_balances_period ON gl_account_balances(period_id);

-- Triggers for timestamp updates
CREATE TRIGGER IF NOT EXISTS gl_accounts_updated_at
AFTER UPDATE ON gl_accounts
BEGIN
    UPDATE gl_accounts SET updated_at = datetime('now') WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS gl_periods_updated_at
AFTER UPDATE ON gl_periods
BEGIN
    UPDATE gl_periods SET updated_at = datetime('now') WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS gl_journal_entries_updated_at
AFTER UPDATE ON gl_journal_entries
BEGIN
    UPDATE gl_journal_entries SET updated_at = datetime('now') WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS gl_auto_posting_config_updated_at
AFTER UPDATE ON gl_auto_posting_config
BEGIN
    UPDATE gl_auto_posting_config SET updated_at = datetime('now') WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS gl_account_balances_updated_at
AFTER UPDATE ON gl_account_balances
BEGIN
    UPDATE gl_account_balances SET updated_at = datetime('now') WHERE id = NEW.id;
END;

-- View for Trial Balance
CREATE VIEW IF NOT EXISTS v_trial_balance AS
SELECT
    a.id as account_id,
    a.account_number,
    a.name as account_name,
    a.account_type,
    a.normal_balance,
    CASE
        WHEN a.normal_balance = 'debit' THEN CAST(a.current_balance AS REAL)
        ELSE 0
    END as debit_balance,
    CASE
        WHEN a.normal_balance = 'credit' THEN CAST(a.current_balance AS REAL)
        ELSE 0
    END as credit_balance
FROM gl_accounts a
WHERE a.is_posting = 1 AND a.status = 'active'
ORDER BY a.account_number;
