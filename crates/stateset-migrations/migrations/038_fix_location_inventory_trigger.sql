-- Fix the location_inventory updated_at trigger.
--
-- Migration 036 recreated update_location_inventory_timestamp with
-- `WHERE id = NEW.id`, but location_inventory has a composite primary key
-- (location_id, sku, lot_id) and no `id` column. As a result EVERY UPDATE on
-- location_inventory failed at runtime with "no such column: id", breaking
-- move_inventory and the existing-row path of adjust_inventory entirely.
--
-- Restore the correct composite-key predicate from migration 016 (lot_id is
-- nullable, so compare via COALESCE).
DROP TRIGGER IF EXISTS update_location_inventory_timestamp;
CREATE TRIGGER update_location_inventory_timestamp
AFTER UPDATE ON location_inventory
BEGIN
    UPDATE location_inventory SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
    WHERE location_id = NEW.location_id
      AND sku = NEW.sku
      AND COALESCE(lot_id, '') = COALESCE(NEW.lot_id, '');
END;
