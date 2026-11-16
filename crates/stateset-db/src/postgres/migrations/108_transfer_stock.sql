-- Bind transfers to physical stock. Existing documents remain readable; no
-- warehouse UUID is guessed to be an integer stock location.
ALTER TABLE transfer_orders ADD COLUMN source_location_id INTEGER REFERENCES locations(id);
ALTER TABLE transfer_orders ADD COLUMN destination_location_id INTEGER REFERENCES locations(id);
ALTER TABLE transfer_order_items ADD COLUMN lot_id UUID REFERENCES lots(id);
ALTER TABLE transfer_order_items ADD COLUMN quantity_variance NUMERIC NOT NULL DEFAULT 0;
