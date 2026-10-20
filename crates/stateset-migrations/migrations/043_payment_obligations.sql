-- Payment obligations: scheduled amounts owed to suppliers, generated from
-- purchase-order terms, linkable to AP bills, with a dashboard rollup.
--
-- Repository: crates/stateset-db/src/sqlite/payment_obligations.rs
-- REST:       crates/stateset-http/src/routes/payment_obligations.rs
--
-- Money stored as TEXT; due_date as ISO date TEXT; timestamps RFC3339 TEXT.

CREATE TABLE IF NOT EXISTS payment_obligations (
    id TEXT PRIMARY KEY,
    number TEXT NOT NULL,
    supplier_id TEXT NOT NULL,
    purchase_order_id TEXT,
    amount TEXT NOT NULL,
    amount_paid TEXT NOT NULL DEFAULT '0',
    currency TEXT NOT NULL DEFAULT 'USD',
    due_date TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    linked_bill_ids TEXT NOT NULL DEFAULT '[]',
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_payment_obligations_supplier ON payment_obligations(supplier_id);
CREATE INDEX IF NOT EXISTS idx_payment_obligations_status ON payment_obligations(status);
CREATE INDEX IF NOT EXISTS idx_payment_obligations_due ON payment_obligations(due_date);
