-- Vendor credits: amounts a supplier owes back to the buyer, applicable
-- against AP bills or payment obligations (AP-side mirror of credit memos).
--
-- Repository: crates/stateset-db/src/sqlite/vendor_credits.rs
-- REST:       crates/stateset-http/src/routes/vendor_credits.rs
--
-- Money stored as TEXT; booleans as INTEGER 0/1; timestamps RFC3339 TEXT.

CREATE TABLE IF NOT EXISTS vendor_credits (
    id TEXT PRIMARY KEY,
    number TEXT NOT NULL,
    supplier_id TEXT NOT NULL,
    vendor_return_id TEXT,
    amount TEXT NOT NULL,
    remaining TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    status TEXT NOT NULL DEFAULT 'open',
    memo TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_vendor_credits_supplier ON vendor_credits(supplier_id);
CREATE INDEX IF NOT EXISTS idx_vendor_credits_status ON vendor_credits(status);

CREATE TABLE IF NOT EXISTS vendor_credit_applications (
    id TEXT PRIMARY KEY,
    vendor_credit_id TEXT NOT NULL REFERENCES vendor_credits(id) ON DELETE CASCADE,
    target_type TEXT NOT NULL DEFAULT 'bill',
    target_id TEXT NOT NULL,
    amount TEXT NOT NULL,
    reversed INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_vendor_credit_apps_credit ON vendor_credit_applications(vendor_credit_id);
