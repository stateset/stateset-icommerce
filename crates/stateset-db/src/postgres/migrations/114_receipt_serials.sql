CREATE TABLE receipt_serials (
    serial_id UUID PRIMARY KEY REFERENCES serial_numbers(id),
    receipt_item_id UUID NOT NULL REFERENCES receipt_items(id),
    put_away_id UUID REFERENCES put_aways(id)
);
CREATE INDEX idx_receipt_serials_item ON receipt_serials(receipt_item_id, put_away_id);
