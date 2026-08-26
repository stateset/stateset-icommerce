CREATE TABLE IF NOT EXISTS a2a_escrows (
  id TEXT PRIMARY KEY,
  status TEXT NOT NULL DEFAULT 'created',
  quote_id TEXT,
  payment_id TEXT,
  buyer_address TEXT NOT NULL,
  seller_address TEXT NOT NULL,
  amount BIGINT NOT NULL,
  amount_decimal NUMERIC NOT NULL,
  asset TEXT NOT NULL DEFAULT 'USDC',
  network TEXT NOT NULL DEFAULT 'set_chain',
  release_conditions JSONB NOT NULL DEFAULT '[]'::jsonb,
  funded_at TIMESTAMPTZ,
  released_at TIMESTAMPTZ,
  disputed_at TIMESTAMPTZ,
  dispute_id TEXT,
  expires_at TIMESTAMPTZ NOT NULL,
  auto_release_after TIMESTAMPTZ,
  metadata JSONB,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_a2a_escrows_buyer ON a2a_escrows(buyer_address);
CREATE INDEX IF NOT EXISTS idx_a2a_escrows_seller ON a2a_escrows(seller_address);
CREATE INDEX IF NOT EXISTS idx_a2a_escrows_status ON a2a_escrows(status);
CREATE INDEX IF NOT EXISTS idx_a2a_escrows_quote ON a2a_escrows(quote_id);
