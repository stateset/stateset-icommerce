-- Fraud detection migration for PostgreSQL
-- Fraud assessments and configurable fraud rules

CREATE TABLE IF NOT EXISTS fraud_assessments (
    order_id UUID PRIMARY KEY,
    risk_score DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    signals JSONB NOT NULL DEFAULT '[]'::jsonb,
    decision VARCHAR(50) NOT NULL DEFAULT 'accept',
    reviewed_by VARCHAR(255),
    review_notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS fraud_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    signal_type VARCHAR(50) NOT NULL,
    threshold DOUBLE PRECISION NOT NULL DEFAULT 0.5,
    action VARCHAR(50) NOT NULL DEFAULT 'review',
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_fraud_assessments_decision ON fraud_assessments(decision);
CREATE INDEX IF NOT EXISTS idx_fraud_assessments_risk_score ON fraud_assessments(risk_score);
CREATE INDEX IF NOT EXISTS idx_fraud_assessments_created_at ON fraud_assessments(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_fraud_rules_signal_type ON fraud_rules(signal_type);
CREATE INDEX IF NOT EXISTS idx_fraud_rules_enabled ON fraud_rules(enabled);
CREATE INDEX IF NOT EXISTS idx_fraud_rules_created_at ON fraud_rules(created_at DESC);

-- Check constraints
ALTER TABLE fraud_assessments ADD CONSTRAINT fraud_assessments_risk_score_range CHECK (risk_score >= 0.0 AND risk_score <= 1.0);
ALTER TABLE fraud_rules ADD CONSTRAINT fraud_rules_threshold_range CHECK (threshold >= 0.0 AND threshold <= 1.0);
