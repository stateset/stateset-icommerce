-- Tenant-bound A2A disputes with exact money and immutable evidence.
ALTER TABLE a2a_escrows
  ADD COLUMN IF NOT EXISTS tenant_id TEXT NOT NULL DEFAULT 'legacy',
  ADD COLUMN IF NOT EXISTS store_id TEXT NOT NULL DEFAULT 'legacy';

CREATE INDEX IF NOT EXISTS idx_a2a_escrows_scope
  ON a2a_escrows(tenant_id, store_id, id);

CREATE TABLE IF NOT EXISTS a2a_disputes (
  id TEXT PRIMARY KEY,
  tenant_id TEXT NOT NULL,
  store_id TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'filed',
  escrow_id TEXT NOT NULL REFERENCES a2a_escrows(id),
  quote_id TEXT,
  claimant_address TEXT NOT NULL,
  respondent_address TEXT NOT NULL,
  reason TEXT NOT NULL,
  category TEXT NOT NULL,
  amount_decimal NUMERIC NOT NULL,
  asset TEXT NOT NULL,
  resolution_type TEXT,
  buyer_amount_decimal NUMERIC,
  seller_amount_decimal NUMERIC,
  resolution_note TEXT,
  resolved_by TEXT,
  evidence_deadline TIMESTAMPTZ NOT NULL,
  review_deadline TIMESTAMPTZ NOT NULL,
  metadata JSONB,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  resolved_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_a2a_disputes_scope
  ON a2a_disputes(tenant_id, store_id, id);
CREATE INDEX IF NOT EXISTS idx_a2a_disputes_escrow
  ON a2a_disputes(escrow_id);
CREATE INDEX IF NOT EXISTS idx_a2a_disputes_status
  ON a2a_disputes(status);
CREATE UNIQUE INDEX IF NOT EXISTS idx_a2a_disputes_one_open_per_escrow
  ON a2a_disputes(escrow_id)
  WHERE status IN ('filed', 'evidence_period', 'under_review', 'escalated');

CREATE TABLE IF NOT EXISTS a2a_dispute_evidence (
  id TEXT PRIMARY KEY,
  tenant_id TEXT NOT NULL,
  store_id TEXT NOT NULL,
  dispute_id TEXT NOT NULL REFERENCES a2a_disputes(id),
  submitted_by TEXT NOT NULL,
  evidence_type TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT,
  content TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_a2a_dispute_evidence_scope
  ON a2a_dispute_evidence(tenant_id, store_id, dispute_id, created_at);
