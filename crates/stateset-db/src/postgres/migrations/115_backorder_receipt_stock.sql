CREATE TABLE backorder_receipt_stock (
    backorder_id UUID NOT NULL REFERENCES backorders(id),
    put_away_id UUID NOT NULL REFERENCES put_aways(id),
    quantity NUMERIC NOT NULL,
    PRIMARY KEY (backorder_id, put_away_id)
);
CREATE INDEX idx_backorder_receipt_stock_put_away ON backorder_receipt_stock(put_away_id);
