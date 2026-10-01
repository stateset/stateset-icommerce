-- Give EVERY live customer a resolvable `email_key` (legacy-safe follow-up to 085).
--
-- Migration 085 keyed only addresses that were unambiguous at the time: rows
-- whose `LOWER(TRIM(email))` collided with another live row were left
-- `email_key = NULL`. That orphaned them:
--   * `get_by_email` matches on `email_key`, so a NULL-keyed account could
--     never be found by its own address;
--   * `find_or_create` therefore fell through to an INSERT, which hit the
--     legacy raw `UNIQUE(email)` constraint from 001 and returned
--     `EmailAlreadyExists` forever. The account was neither reachable nor
--     re-registerable without a manual fix.
--
-- Backfill rule (deterministic and reversible)
-- -------------------------------------------
-- Within each group of live rows sharing `LOWER(TRIM(email))`:
--   * the OLDEST row (ORDER BY created_at, id) keeps the canonical key
--     `LOWER(TRIM(email))` — first registration wins the address, so
--     `get_by_email` / `find_or_create` resolve to it;
--   * every newer row is keyed `LOWER(TRIM(email)) || ' ' || id`.
-- Whitespace can never appear in an address accepted by `validate_email`, so a
-- suffixed key can never collide with a real normalised address. The rule is
-- reversible: strip everything from the space onwards to recover the address
-- the row was registered with (its raw `email` column is left untouched).
--
-- Consequences, all of them defined:
--   * every live customer now has a key, so `create` / `update` always fail
--     with the typed `EmailAlreadyExists` instead of a raw-constraint error;
--   * a suffixed duplicate stays retrievable by id and by the `list(email=…)`
--     substring filter, and keeps its own address once the canonical holder
--     changes or deletes theirs (a later update re-keys it normally);
--   * deleted accounts remain NULL-keyed on purpose (their address is free).
--
-- The expression index makes the grouping (and 085's correlated backfill on
-- databases that have not run it yet) linear instead of a table scan per row.

CREATE INDEX IF NOT EXISTS idx_customers_email_lower
    ON customers(LOWER(TRIM(email)));

-- 1. Suffix every non-oldest member of a colliding group.
WITH ranked AS (
    SELECT id,
           ROW_NUMBER() OVER (
               PARTITION BY LOWER(TRIM(email))
               ORDER BY created_at ASC, id ASC
           ) AS rank_in_group
    FROM customers
    WHERE status != 'deleted' AND email_key IS NULL
)
UPDATE customers
SET email_key = LOWER(TRIM(email)) || ' ' || id
WHERE id IN (SELECT id FROM ranked WHERE rank_in_group > 1);

-- 2. Give the surviving oldest row the canonical key, unless some other row
--    already holds it (hand-edited databases); the unique index makes this
--    lookup a probe rather than a scan.
UPDATE customers
SET email_key = LOWER(TRIM(email))
WHERE status != 'deleted'
  AND email_key IS NULL
  AND NOT EXISTS (
      SELECT 1 FROM customers held
      WHERE held.email_key = LOWER(TRIM(customers.email))
  );

-- 3. Anything still NULL lost the canonical key to a pre-existing holder:
--    suffix it too, so the "every live customer is keyed" invariant holds.
UPDATE customers
SET email_key = LOWER(TRIM(email)) || ' ' || id
WHERE status != 'deleted' AND email_key IS NULL;
