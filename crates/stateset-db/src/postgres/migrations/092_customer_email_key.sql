-- Case-insensitive e-mail uniqueness for live customer accounts (PostgreSQL).
--
-- Mirrors SQLite migration 085. A keyed column is used instead of a partial
-- unique index on LOWER(email) so that legacy case-duplicates never make the
-- migration fail: only unambiguous live addresses are backfilled, deleted
-- accounts stay NULL (their address is released for re-registration and the
-- repository tombstones the raw `email` to `deleted+<id>@invalid`).
ALTER TABLE customers ADD COLUMN IF NOT EXISTS email_key TEXT;

UPDATE customers
SET email_key = LOWER(TRIM(email))
WHERE status <> 'deleted'
  AND email_key IS NULL
  AND (
      SELECT COUNT(*) FROM customers dup
      WHERE LOWER(TRIM(dup.email)) = LOWER(TRIM(customers.email))
        AND dup.status <> 'deleted'
  ) = 1;

CREATE UNIQUE INDEX IF NOT EXISTS idx_customers_email_key
    ON customers(email_key);
