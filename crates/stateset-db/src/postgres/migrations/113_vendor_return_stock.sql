ALTER TABLE vendor_returns ADD COLUMN warehouse_id INTEGER REFERENCES warehouses(id);
ALTER TABLE vendor_return_items ADD COLUMN source_location_id INTEGER REFERENCES locations(id);
ALTER TABLE vendor_return_items ADD COLUMN lot_id UUID REFERENCES lots(id);
CREATE INDEX vendor_credits_return ON vendor_credits(vendor_return_id);
