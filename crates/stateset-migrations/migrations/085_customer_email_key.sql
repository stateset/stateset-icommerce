-- Case-insensitive e-mail uniqueness for live customer accounts.
--
-- `customers.email` is UNIQUE but case-sensitive, so `Alice@Example.com` and
-- `alice@example.com` were two accounts, `get_by_email` missed the second
-- spelling and `find_or_create` duplicated customers. The application now
-- normalises (trim + lower-case) at every write and lookup; this keyed column
-- is the database backstop that holds for writers that bypass the repository.
--
-- Design notes (why a keyed column instead of a unique index on LOWER(email)):
--   * Legacy databases may already hold case-duplicates; a plain unique
--     expression index would make this migration fail on real data. The
--     backfill keys ONLY addresses that are unambiguous today (exactly one
--     live row per lower-cased address) and leaves duplicates NULL, so the
--     migration can never fail. Ambiguous legacy rows are keyed the next
--     time the repository updates them (the second one to be touched gets
--     `EmailAlreadyExists`).
--   * Deleted accounts are NOT keyed (NULL), so a deleted customer's address
--     is free for re-registration; the repository also replaces the raw
--     `email` of a deleted row with a `deleted+<id>@invalid` tombstone
--     because the legacy raw UNIQUE(email) constraint cannot be dropped
--     without a table rebuild on SQLite.
ALTER TABLE customers ADD COLUMN email_key TEXT;

UPDATE customers
SET email_key = LOWER(TRIM(email))
WHERE status != 'deleted'
  AND (
      SELECT COUNT(*) FROM customers dup
      WHERE LOWER(TRIM(dup.email)) = LOWER(TRIM(customers.email))
        AND dup.status != 'deleted'
  ) = 1;

CREATE UNIQUE INDEX IF NOT EXISTS idx_customers_email_key
    ON customers(email_key);
