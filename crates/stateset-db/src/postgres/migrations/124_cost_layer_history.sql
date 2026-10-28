-- Every cost-layer quantity and carrying-value change has a dated history.
CREATE TABLE stock_cost_layer_changes (
 id UUID PRIMARY KEY, layer_id UUID NOT NULL REFERENCES cost_layers(id),
 movement_id UUID REFERENCES inventory_movements(id),
 quantity_delta NUMERIC NOT NULL, value_delta NUMERIC NOT NULL,
 currency TEXT NOT NULL, created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX stock_cost_layer_changes_layer_date ON stock_cost_layer_changes(layer_id,created_at);
ALTER TABLE cost_transactions ADD COLUMN currency TEXT;
ALTER TABLE stock_cost_events ADD COLUMN settlement_currency TEXT;
ALTER TABLE stock_cost_events ADD COLUMN settlement_rate NUMERIC;
