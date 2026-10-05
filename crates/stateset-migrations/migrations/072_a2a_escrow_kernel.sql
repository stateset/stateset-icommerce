-- Shared embedded storage for governed A2A escrow release. The existing
-- a2a_quotes table from migration 029 remains authoritative for quote state.
CREATE TABLE IF NOT EXISTS a2a_escrows (
  id TEXT PRIMARY KEY,
  status TEXT NOT NULL DEFAULT 'created',
  quote_id TEXT,
  payment_id TEXT,
  buyer_address TEXT NOT NULL,
  seller_address TEXT NOT NULL,
  amount INTEGER NOT NULL,
  amount_decimal TEXT NOT NULL,
  asset TEXT NOT NULL DEFAULT 'USDC',
  network TEXT NOT NULL DEFAULT 'set_chain',
  release_conditions TEXT NOT NULL DEFAULT '[]',
  funded_at TEXT,
  released_at TEXT,
  disputed_at TEXT,
  dispute_id TEXT,
  expires_at TEXT NOT NULL,
  auto_release_after TEXT,
  metadata TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_a2a_escrows_buyer ON a2a_escrows(buyer_address);
CREATE INDEX IF NOT EXISTS idx_a2a_escrows_seller ON a2a_escrows(seller_address);
CREATE INDEX IF NOT EXISTS idx_a2a_escrows_status ON a2a_escrows(status);
CREATE INDEX IF NOT EXISTS idx_a2a_escrows_quote ON a2a_escrows(quote_id);
