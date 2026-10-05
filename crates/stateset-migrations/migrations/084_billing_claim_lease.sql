-- Billing-worker claim leases on subscriptions.
--
-- `get_due_for_billing` was a read-only list: two billing workers polling at
-- the same moment both saw the same due subscription and both charged the
-- customer before the (subscription, cycle_number) uniqueness backstop could
-- stop the second one. A worker now CLAIMS a batch of due subscriptions
-- first (`claim_due_for_billing`): the claim stamps a lease on each row
-- inside the write transaction, a live lease hides the row from every other
-- worker's claim, and `create_billing_cycle` refuses to bill a subscription
-- whose live lease belongs to someone else.
--
-- Both columns are nullable and unset for every existing row, so this
-- migration is legacy-safe: nothing is leased until a worker claims it.
--   * `billing_lease_owner` — opaque worker id that holds the lease.
--   * `billing_lease_until` — RFC3339 expiry; a lease past this instant is
--     dead and may be re-claimed, so a crashed worker never wedges billing.
ALTER TABLE subscriptions ADD COLUMN billing_lease_owner TEXT;
ALTER TABLE subscriptions ADD COLUMN billing_lease_until TEXT;

CREATE INDEX IF NOT EXISTS idx_subscriptions_billing_lease
    ON subscriptions(billing_lease_until);
