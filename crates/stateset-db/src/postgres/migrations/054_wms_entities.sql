-- WMS / operations entities ported from the SQLite backend:
-- transfer orders, units of measure, inbound shipments (ASNs),
-- print stations, and stock snapshots.
--
-- Repositories:
--   crates/stateset-db/src/postgres/transfer_orders.rs
--   crates/stateset-db/src/postgres/units_of_measure.rs
--   crates/stateset-db/src/postgres/inbound_shipments.rs
--   crates/stateset-db/src/postgres/print_stations.rs
--   crates/stateset-db/src/postgres/stock_snapshots.rs

-- ============================================================================
-- Transfer orders
-- ============================================================================
CREATE TABLE IF NOT EXISTS transfer_orders (
    id UUID PRIMARY KEY,
    number TEXT NOT NULL,
    source_warehouse_id UUID NOT NULL,
    destination_warehouse_id UUID NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    expected_at TIMESTAMPTZ,
    shipped_at TIMESTAMPTZ,
    received_at TIMESTAMPTZ,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_transfer_orders_status ON transfer_orders(status);
CREATE INDEX IF NOT EXISTS idx_transfer_orders_source ON transfer_orders(source_warehouse_id);
CREATE INDEX IF NOT EXISTS idx_transfer_orders_dest ON transfer_orders(destination_warehouse_id);

CREATE TABLE IF NOT EXISTS transfer_order_items (
    id UUID PRIMARY KEY,
    transfer_order_id UUID NOT NULL REFERENCES transfer_orders(id) ON DELETE CASCADE,
    product_id UUID NOT NULL,
    sku TEXT NOT NULL,
    quantity NUMERIC NOT NULL,
    quantity_shipped NUMERIC NOT NULL DEFAULT 0,
    quantity_received NUMERIC NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_transfer_order_items_order ON transfer_order_items(transfer_order_id);

-- ============================================================================
-- Units of measure
-- ============================================================================
CREATE TABLE IF NOT EXISTS unit_classes (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    base_uom_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS units_of_measure (
    id UUID PRIMARY KEY,
    unit_class_id UUID NOT NULL REFERENCES unit_classes(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    abbreviation TEXT NOT NULL,
    factor NUMERIC NOT NULL DEFAULT 1,
    is_base BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_uoms_class ON units_of_measure(unit_class_id);

CREATE TABLE IF NOT EXISTS unit_conversion_rules (
    id UUID PRIMARY KEY,
    rule_type TEXT NOT NULL DEFAULT 'SYSTEM',
    product_id UUID,
    from_uom_id UUID NOT NULL,
    to_uom_id UUID NOT NULL,
    factor NUMERIC NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_conversion_rules_product ON unit_conversion_rules(product_id);

-- ============================================================================
-- Inbound shipments (advance ship notices)
-- ============================================================================
CREATE TABLE IF NOT EXISTS inbound_shipments (
    id UUID PRIMARY KEY,
    number TEXT NOT NULL,
    supplier_id UUID NOT NULL,
    purchase_order_id UUID,
    warehouse_id UUID,
    carrier TEXT,
    tracking_number TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    expected_at TIMESTAMPTZ,
    received_at TIMESTAMPTZ,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_inbound_shipments_supplier ON inbound_shipments(supplier_id);
CREATE INDEX IF NOT EXISTS idx_inbound_shipments_status ON inbound_shipments(status);
CREATE INDEX IF NOT EXISTS idx_inbound_shipments_warehouse ON inbound_shipments(warehouse_id);

CREATE TABLE IF NOT EXISTS inbound_shipment_items (
    id UUID PRIMARY KEY,
    inbound_shipment_id UUID NOT NULL REFERENCES inbound_shipments(id) ON DELETE CASCADE,
    product_id UUID NOT NULL,
    sku TEXT NOT NULL,
    quantity_expected NUMERIC NOT NULL,
    quantity_received NUMERIC NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_inbound_shipment_items_shipment ON inbound_shipment_items(inbound_shipment_id);

-- ============================================================================
-- Print stations: only the SHA-256 hash of the pairing token is stored;
-- the plaintext token is returned once at pairing. Printers as JSON TEXT.
-- ============================================================================
CREATE TABLE IF NOT EXISTS print_stations (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    printers TEXT NOT NULL DEFAULT '[]',
    revoked BOOLEAN NOT NULL DEFAULT FALSE,
    last_seen_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_print_stations_revoked ON print_stations(revoked);

CREATE TABLE IF NOT EXISTS print_jobs (
    id UUID PRIMARY KEY,
    station_id UUID NOT NULL REFERENCES print_stations(id) ON DELETE CASCADE,
    printer_name TEXT,
    payload_kind TEXT NOT NULL DEFAULT 'zpl',
    payload TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    picked_up_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_print_jobs_station ON print_jobs(station_id, status);

-- ============================================================================
-- Stock snapshots
-- ============================================================================
CREATE TABLE IF NOT EXISTS stock_snapshots (
    id UUID PRIMARY KEY,
    label TEXT,
    total_skus BIGINT NOT NULL DEFAULT 0,
    total_units NUMERIC NOT NULL DEFAULT 0,
    captured_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_stock_snapshots_captured ON stock_snapshots(captured_at);

CREATE TABLE IF NOT EXISTS stock_snapshot_lines (
    id UUID PRIMARY KEY,
    stock_snapshot_id UUID NOT NULL REFERENCES stock_snapshots(id) ON DELETE CASCADE,
    product_id UUID NOT NULL,
    sku TEXT NOT NULL,
    quantity_on_hand NUMERIC NOT NULL DEFAULT 0,
    quantity_available NUMERIC NOT NULL DEFAULT 0,
    location TEXT
);
CREATE INDEX IF NOT EXISTS idx_stock_snapshot_lines_snapshot ON stock_snapshot_lines(stock_snapshot_id);
