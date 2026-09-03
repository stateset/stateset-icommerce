-- Inventory round 6: the balance identity, enforced at the database
-- (see the SQLite twin, 092_inventory_balance_identity.sql, for the full
-- rationale).
--
--     quantity_available = quantity_on_hand - quantity_allocated
--
-- The columns are exact NUMERIC here (migration 080), so unlike the SQLite
-- trigger this needs no float tolerance.
--
-- LEGACY-SAFE: the CHECK is added NOT VALID, so the ALTER never inspects
-- existing rows and a deployment whose balances already drifted still
-- migrates — but every new write is validated from now on. It is then
-- VALIDATEd only when no violating row exists, so a clean database ends up
-- with a fully validated constraint and a drifted one keeps the NOT VALID
-- guard until the rows are repaired (the engine's clamp paths now repair
-- them instead of only logging the drift; re-running this migration after a
-- repair is a no-op that promotes the constraint).
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'chk_inventory_balances_identity'
          AND conrelid = 'inventory_balances'::regclass
    ) THEN
        ALTER TABLE inventory_balances
            ADD CONSTRAINT chk_inventory_balances_identity
            CHECK (quantity_available = quantity_on_hand - quantity_allocated) NOT VALID;
    END IF;

    -- Skip validation (leave the constraint NOT VALID) if legacy rows violate it.
    IF NOT EXISTS (
        SELECT 1 FROM inventory_balances
        WHERE quantity_available <> quantity_on_hand - quantity_allocated
    ) THEN
        ALTER TABLE inventory_balances VALIDATE CONSTRAINT chk_inventory_balances_identity;
    END IF;
END $$;
