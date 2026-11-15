CREATE TABLE backorder_receipt_stock (
    backorder_id TEXT NOT NULL REFERENCES backorders(id),
    put_away_id TEXT NOT NULL REFERENCES put_aways(id),
    quantity TEXT NOT NULL,
    PRIMARY KEY (backorder_id, put_away_id)
);
CREATE INDEX idx_backorder_receipt_stock_put_away ON backorder_receipt_stock(put_away_id);
