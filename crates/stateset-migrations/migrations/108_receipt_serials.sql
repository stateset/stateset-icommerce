CREATE TABLE receipt_serials (
    serial_id TEXT PRIMARY KEY REFERENCES serial_numbers(id),
    receipt_item_id TEXT NOT NULL REFERENCES receipt_items(id),
    put_away_id TEXT REFERENCES put_aways(id)
);
CREATE INDEX idx_receipt_serials_item ON receipt_serials(receipt_item_id, put_away_id);
