CREATE TABLE stock_location_bins (
    location_id INTEGER PRIMARY KEY REFERENCES locations(id),
    bin_id INTEGER NOT NULL UNIQUE REFERENCES warehouse_bins(id)
);
CREATE TABLE stock_warehouse_locations (
    warehouse_id INTEGER NOT NULL REFERENCES warehouses(id),
    role TEXT NOT NULL CHECK (role IN ('unassigned', 'staging')),
    location_id INTEGER NOT NULL UNIQUE REFERENCES locations(id),
    PRIMARY KEY (warehouse_id, role)
);
CREATE TABLE inventory_reservation_stock (
    reservation_id UUID NOT NULL REFERENCES inventory_reservations(id),
    location_id INTEGER NOT NULL REFERENCES locations(id),
    lot_id UUID NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000',
    quantity NUMERIC NOT NULL CHECK (quantity >= 0),
    PRIMARY KEY (reservation_id, location_id, lot_id)
);
CREATE TABLE pick_stock_custody (
    pick_id UUID PRIMARY KEY REFERENCES pick_tasks(id),
    staging_location_id INTEGER NOT NULL REFERENCES locations(id),
    quantity NUMERIC NOT NULL CHECK (quantity >= 0),
    quantity_shipped NUMERIC NOT NULL DEFAULT 0 CHECK (quantity_shipped >= 0 AND quantity_shipped <= quantity)
);
