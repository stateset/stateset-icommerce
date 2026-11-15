CREATE TABLE stock_snapshot_lines_ledger (
    id TEXT PRIMARY KEY,
    stock_snapshot_id TEXT NOT NULL REFERENCES stock_snapshots(id) ON DELETE CASCADE,
    product_id TEXT,
    sku TEXT NOT NULL,
    quantity_on_hand TEXT NOT NULL DEFAULT '0',
    quantity_available TEXT NOT NULL DEFAULT '0',
    location TEXT
);
INSERT INTO stock_snapshot_lines_ledger SELECT * FROM stock_snapshot_lines;
DROP TABLE stock_snapshot_lines;
ALTER TABLE stock_snapshot_lines_ledger RENAME TO stock_snapshot_lines;
CREATE INDEX idx_stock_snapshot_lines_snapshot ON stock_snapshot_lines(stock_snapshot_id);
