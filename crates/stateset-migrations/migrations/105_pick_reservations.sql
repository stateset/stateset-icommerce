CREATE TABLE pick_stock_reservations (
    pick_id TEXT NOT NULL REFERENCES pick_tasks(id),
    reservation_id TEXT NOT NULL REFERENCES inventory_reservations(id),
    quantity TEXT NOT NULL,
    quantity_shipped TEXT NOT NULL DEFAULT '0',
    PRIMARY KEY (pick_id, reservation_id)
);
