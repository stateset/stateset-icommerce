-- Billing-worker claim leases on subscriptions (see the SQLite twin,
-- 084_billing_claim_lease.sql, for the full rationale).
--
-- A worker claims due subscriptions with `SELECT ... FOR UPDATE SKIP LOCKED`
-- and stamps a lease in the same transaction; a live lease hides the row from
-- other workers' claims and `create_billing_cycle` refuses a subscription
-- whose live lease belongs to someone else. Both columns are nullable and
-- unset for existing rows, so the migration is legacy-safe.
ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS billing_lease_owner TEXT;
ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS billing_lease_until TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_subscriptions_billing_lease
    ON subscriptions(billing_lease_until);
