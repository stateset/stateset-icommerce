CREATE TABLE pick_stock_reservations (
    pick_id UUID NOT NULL REFERENCES pick_tasks(id),
    reservation_id UUID NOT NULL REFERENCES inventory_reservations(id),
    quantity NUMERIC NOT NULL,
    quantity_shipped NUMERIC NOT NULL DEFAULT '0',
    PRIMARY KEY (pick_id, reservation_id)
);
