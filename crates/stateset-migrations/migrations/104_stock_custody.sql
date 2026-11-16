-- Physical locations and bin views describe the same shelf stock.
CREATE TABLE stock_location_bins (
    location_id INTEGER PRIMARY KEY REFERENCES locations(id),
    bin_id INTEGER NOT NULL UNIQUE REFERENCES warehouse_bins(id)
);
-- Explicit locations for stock without a shelf assignment and for picked goods.
CREATE TABLE stock_warehouse_locations (
    warehouse_id INTEGER NOT NULL REFERENCES warehouses(id),
    role TEXT NOT NULL CHECK (role IN ('unassigned', 'staging')),
    location_id INTEGER NOT NULL UNIQUE REFERENCES locations(id),
    PRIMARY KEY (warehouse_id, role)
);
-- A reservation's physical allocation, including its exact lot identity.
CREATE TABLE inventory_reservation_stock (
    reservation_id TEXT NOT NULL REFERENCES inventory_reservations(id),
    location_id INTEGER NOT NULL REFERENCES locations(id),
    lot_id TEXT NOT NULL DEFAULT '',
    quantity TEXT NOT NULL,
    PRIMARY KEY (reservation_id, location_id, lot_id)
);
-- Picked units remain in staging until dispatched; each unit can ship once.
CREATE TABLE pick_stock_custody (
    pick_id TEXT PRIMARY KEY REFERENCES pick_tasks(id),
    staging_location_id INTEGER NOT NULL REFERENCES locations(id),
    quantity TEXT NOT NULL,
    quantity_shipped TEXT NOT NULL DEFAULT '0'
);
