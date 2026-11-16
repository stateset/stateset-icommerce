-- Every lot reservation/quarantine owns exact physical stock slices.
CREATE TABLE lot_stock_holds (
    owner_id UUID NOT NULL,
    lot_id UUID NOT NULL REFERENCES lots(id),
    location_id INTEGER NOT NULL REFERENCES locations(id),
    quantity NUMERIC NOT NULL,
    PRIMARY KEY(owner_id, lot_id, location_id)
);
