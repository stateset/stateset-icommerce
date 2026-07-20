-- 051_loyalty_tiers: persist loyalty program tiers (Postgres parity with
-- SQLite migration 057). Stored as a JSON array in a TEXT column so both
-- backends share identical serialize/deserialize logic.

ALTER TABLE loyalty_programs ADD COLUMN IF NOT EXISTS tiers TEXT;
