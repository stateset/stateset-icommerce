-- Fraud detection: fraud assessments and configurable fraud rules (SQLite).
--
-- Mirrors postgres/migrations/048_fraud.sql. These tables were previously missing
-- from the embedded SQLite migration set, so every fraud operation failed at runtime
-- with "no such table: fraud_assessments" on the default backend. The schema matches
-- the repository code in sqlite/fraud.rs (order_id is the assessment primary key;
-- signals are stored as JSON TEXT). The Postgres CHECK constraints on risk_score /
-- threshold are intentionally omitted here to match the repository's existing
-- behavior (signal scores are not clamped in code).

CREATE TABLE IF NOT EXISTS fraud_assessments (
    order_id TEXT PRIMARY KEY,
    risk_score REAL NOT NULL DEFAULT 0.0,
    signals TEXT NOT NULL DEFAULT '[]',
    decision TEXT NOT NULL DEFAULT 'accept',
    reviewed_by TEXT,
    review_notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS fraud_rules (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    signal_type TEXT NOT NULL,
    threshold REAL NOT NULL DEFAULT 0.5,
    action TEXT NOT NULL DEFAULT 'review',
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_fraud_assessments_decision ON fraud_assessments(decision);
CREATE INDEX IF NOT EXISTS idx_fraud_assessments_risk_score ON fraud_assessments(risk_score);
CREATE INDEX IF NOT EXISTS idx_fraud_assessments_created_at ON fraud_assessments(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_fraud_rules_signal_type ON fraud_rules(signal_type);
CREATE INDEX IF NOT EXISTS idx_fraud_rules_enabled ON fraud_rules(enabled);
CREATE INDEX IF NOT EXISTS idx_fraud_rules_created_at ON fraud_rules(created_at DESC);
