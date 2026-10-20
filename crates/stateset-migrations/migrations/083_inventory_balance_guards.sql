-- Inventory round 5: database-enforced balance guards + backorder allocations
-- backed by real inventory reservations.
--
-- 1. `backorder_allocations.reservation_id` links an allocation to the
--    `inventory_reservations` row that actually holds its units
--    (reference_type = 'backorder', reference_id = backorder id). Nullable so
--    legacy allocations — which were bookkeeping rows with no inventory
--    write — keep loading; the engine only touches inventory for rows that
--    carry a reservation.
--
-- 2. Non-negative guards on `inventory_balances.quantity_available` /
--    `quantity_allocated`. The columns are TEXT decimals, so the sign check
--    casts to REAL (sign is exact under IEEE-754 for any decimal string).
--    Legacy-safe by construction: the UPDATE trigger only rejects a
--    transition FROM a non-negative value TO a negative one, so a row that
--    already violates the invariant (pre-fix drift) stays editable and can be
--    repaired, while every new write is held to the invariant. The engine
--    guards these in Rust as well; the triggers are the backstop for raw SQL.
ALTER TABLE backorder_allocations ADD COLUMN reservation_id TEXT;

CREATE INDEX IF NOT EXISTS idx_bo_alloc_reservation
    ON backorder_allocations(reservation_id)
    WHERE reservation_id IS NOT NULL;

CREATE TRIGGER IF NOT EXISTS trg_inventory_balances_non_negative_insert
BEFORE INSERT ON inventory_balances
WHEN CAST(NEW.quantity_available AS REAL) < 0 OR CAST(NEW.quantity_allocated AS REAL) < 0
BEGIN
    SELECT RAISE(ABORT, 'inventory_balances: quantity_available and quantity_allocated must be >= 0');
END;

CREATE TRIGGER IF NOT EXISTS trg_inventory_balances_non_negative_update
BEFORE UPDATE OF quantity_available, quantity_allocated ON inventory_balances
WHEN (CAST(NEW.quantity_available AS REAL) < 0 AND CAST(OLD.quantity_available AS REAL) >= 0)
  OR (CAST(NEW.quantity_allocated AS REAL) < 0 AND CAST(OLD.quantity_allocated AS REAL) >= 0)
BEGIN
    SELECT RAISE(ABORT, 'inventory_balances: quantity_available and quantity_allocated must be >= 0');
END;
