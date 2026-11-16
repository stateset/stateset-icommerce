-- Exact quantities on every stock ledger, as 080 did for inventory balances,
-- transactions and reservations. Stock moves now post to the physical,
-- bin, lot, receiving, picking and cost ledgers together, so a NUMERIC(12, 4)
-- column would round a fractional quantity (or overflow above 10^8) while the
-- balance kept it exact, and the ledgers would disagree. Widening never
-- rounds an existing value.
ALTER TABLE location_inventory
    ALTER COLUMN quantity_on_hand TYPE NUMERIC,
    ALTER COLUMN quantity_reserved TYPE NUMERIC;

ALTER TABLE inventory_movements
    ALTER COLUMN quantity TYPE NUMERIC;

ALTER TABLE inventory_bin_levels
    ALTER COLUMN quantity_on_hand TYPE NUMERIC,
    ALTER COLUMN quantity_allocated TYPE NUMERIC;

ALTER TABLE inventory_bin_movements
    ALTER COLUMN quantity TYPE NUMERIC;

ALTER TABLE lots
    ALTER COLUMN quantity_produced TYPE NUMERIC,
    ALTER COLUMN quantity_remaining TYPE NUMERIC,
    ALTER COLUMN quantity_reserved TYPE NUMERIC,
    ALTER COLUMN quantity_quarantined TYPE NUMERIC;

ALTER TABLE lot_locations
    ALTER COLUMN quantity TYPE NUMERIC;

ALTER TABLE lot_reservations
    ALTER COLUMN quantity TYPE NUMERIC;

ALTER TABLE lot_transactions
    ALTER COLUMN quantity TYPE NUMERIC;

ALTER TABLE lot_genealogy
    ALTER COLUMN quantity TYPE NUMERIC;

ALTER TABLE receipt_items
    ALTER COLUMN expected_quantity TYPE NUMERIC,
    ALTER COLUMN received_quantity TYPE NUMERIC,
    ALTER COLUMN rejected_quantity TYPE NUMERIC;

ALTER TABLE put_aways
    ALTER COLUMN quantity TYPE NUMERIC;

ALTER TABLE pick_tasks
    ALTER COLUMN quantity_requested TYPE NUMERIC,
    ALTER COLUMN quantity_picked TYPE NUMERIC,
    ALTER COLUMN quantity_short TYPE NUMERIC;

ALTER TABLE carton_items
    ALTER COLUMN quantity TYPE NUMERIC;

ALTER TABLE quality_holds
    ALTER COLUMN quantity_held TYPE NUMERIC;

ALTER TABLE cost_layers
    ALTER COLUMN quantity TYPE NUMERIC,
    ALTER COLUMN remaining_quantity TYPE NUMERIC;

ALTER TABLE cost_transactions
    ALTER COLUMN quantity TYPE NUMERIC;

ALTER TABLE cost_variances
    ALTER COLUMN quantity TYPE NUMERIC;
