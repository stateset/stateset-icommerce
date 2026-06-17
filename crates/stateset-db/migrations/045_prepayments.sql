-- Prepayments: cash paid to suppliers in advance, drawn down by applying to
-- AP bills or payment obligations; unused balance refundable.
--
-- Repository: crates/stateset-db/src/sqlite/prepayments.rs
-- REST:       crates/stateset-http/src/routes/prepayments.rs
--
-- Money stored as TEXT; booleans as INTEGER 0/1; timestamps RFC3339 TEXT.

CREATE TABLE IF NOT EXISTS prepayments (
    id TEXT PRIMARY KEY,
    number TEXT NOT NULL,
    supplier_id TEXT NOT NULL,
    amount TEXT NOT NULL,
    remaining TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    status TEXT NOT NULL DEFAULT 'open',
    method TEXT,
    reference TEXT,
    memo TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_prepayments_supplier ON prepayments(supplier_id);
CREATE INDEX IF NOT EXISTS idx_prepayments_status ON prepayments(status);

CREATE TABLE IF NOT EXISTS prepayment_applications (
    id TEXT PRIMARY KEY,
    prepayment_id TEXT NOT NULL REFERENCES prepayments(id) ON DELETE CASCADE,
    target_type TEXT NOT NULL DEFAULT 'bill',
    target_id TEXT NOT NULL,
    amount TEXT NOT NULL,
    reversed INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_prepayment_apps_prepayment ON prepayment_applications(prepayment_id);
