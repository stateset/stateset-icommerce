-- Inventory round 5: database-enforced balance guards + backorder allocations
-- backed by real inventory reservations (see the SQLite twin,
-- 083_inventory_balance_guards.sql, for the full rationale).
--
-- The CHECK is added NOT VALID so existing rows are never inspected by the
-- ALTER (legacy-safe: a deployment with drifted negative balances still
-- migrates); every new write is validated from now on. It is then validated
-- only when no violating rows exist, so a clean database gets the fully
-- validated constraint and a drifted one keeps the NOT VALID guard until the
-- rows are repaired.
ALTER TABLE backorder_allocations ADD COLUMN IF NOT EXISTS reservation_id UUID;

CREATE INDEX IF NOT EXISTS idx_bo_alloc_reservation
    ON backorder_allocations(reservation_id)
    WHERE reservation_id IS NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'chk_inventory_balances_non_negative'
          AND conrelid = 'inventory_balances'::regclass
    ) THEN
        ALTER TABLE inventory_balances
            ADD CONSTRAINT chk_inventory_balances_non_negative
            CHECK (quantity_available >= 0 AND quantity_allocated >= 0) NOT VALID;
    END IF;

    -- Skip validation (leave the constraint NOT VALID) if legacy rows violate it.
    IF NOT EXISTS (
        SELECT 1 FROM inventory_balances
        WHERE quantity_available < 0 OR quantity_allocated < 0
    ) THEN
        ALTER TABLE inventory_balances VALIDATE CONSTRAINT chk_inventory_balances_non_negative;
    END IF;
END $$;
