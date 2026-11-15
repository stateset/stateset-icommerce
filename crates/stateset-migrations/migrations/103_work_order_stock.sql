-- Explicit physical routing; old document-only jobs remain unmapped until edited.
ALTER TABLE manufacturing_work_orders ADD COLUMN source_location_id INTEGER REFERENCES locations(id);
ALTER TABLE manufacturing_work_orders ADD COLUMN output_location_id INTEGER REFERENCES locations(id);
ALTER TABLE manufacturing_work_orders ADD COLUMN output_sku TEXT;
ALTER TABLE manufacturing_work_orders ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
