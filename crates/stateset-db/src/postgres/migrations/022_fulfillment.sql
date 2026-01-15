-- Fulfillment (Pick/Pack/Ship) schema (PostgreSQL)

CREATE TABLE IF NOT EXISTS waves (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    wave_number TEXT UNIQUE NOT NULL,
    warehouse_id INTEGER NOT NULL REFERENCES warehouses(id),
    status TEXT NOT NULL DEFAULT 'draft',
    order_count INTEGER NOT NULL DEFAULT 0,
    pick_count INTEGER NOT NULL DEFAULT 0,
    completed_pick_count INTEGER NOT NULL DEFAULT 0,
    priority INTEGER NOT NULL DEFAULT 0,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    notes TEXT,
    created_by TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_waves_number ON waves(wave_number);
CREATE INDEX IF NOT EXISTS idx_waves_status ON waves(status);
CREATE INDEX IF NOT EXISTS idx_waves_warehouse ON waves(warehouse_id);

CREATE TABLE IF NOT EXISTS wave_orders (
    wave_id UUID NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    order_id UUID NOT NULL,
    PRIMARY KEY (wave_id, order_id)
);

CREATE TABLE IF NOT EXISTS pick_tasks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    wave_id UUID REFERENCES waves(id),
    order_id UUID NOT NULL,
    order_item_id UUID NOT NULL,
    warehouse_id INTEGER NOT NULL REFERENCES warehouses(id),
    status TEXT NOT NULL DEFAULT 'pending',
    sku TEXT NOT NULL,
    product_name TEXT,
    source_location_id INTEGER NOT NULL REFERENCES locations(id),
    source_location_code TEXT,
    quantity_requested NUMERIC(12, 4) NOT NULL,
    quantity_picked NUMERIC(12, 4) NOT NULL DEFAULT 0,
    quantity_short NUMERIC(12, 4) NOT NULL DEFAULT 0,
    lot_id UUID,
    serial_number TEXT,
    assigned_to TEXT,
    priority INTEGER NOT NULL DEFAULT 0,
    pick_sequence INTEGER NOT NULL DEFAULT 0,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pick_tasks_wave ON pick_tasks(wave_id);
CREATE INDEX IF NOT EXISTS idx_pick_tasks_order ON pick_tasks(order_id);
CREATE INDEX IF NOT EXISTS idx_pick_tasks_status ON pick_tasks(status);
CREATE INDEX IF NOT EXISTS idx_pick_tasks_assigned ON pick_tasks(assigned_to);
CREATE INDEX IF NOT EXISTS idx_pick_tasks_location ON pick_tasks(source_location_id);

CREATE TABLE IF NOT EXISTS pack_tasks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_id UUID NOT NULL,
    shipment_id UUID,
    status TEXT NOT NULL DEFAULT 'pending',
    carton_count INTEGER NOT NULL DEFAULT 0,
    total_weight_kg NUMERIC(12, 4),
    assigned_to TEXT,
    packing_station TEXT,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pack_tasks_order ON pack_tasks(order_id);
CREATE INDEX IF NOT EXISTS idx_pack_tasks_status ON pack_tasks(status);
CREATE INDEX IF NOT EXISTS idx_pack_tasks_assigned ON pack_tasks(assigned_to);

CREATE TABLE IF NOT EXISTS cartons (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    pack_task_id UUID NOT NULL REFERENCES pack_tasks(id) ON DELETE CASCADE,
    carton_number TEXT NOT NULL,
    package_type TEXT NOT NULL DEFAULT 'box',
    weight_kg NUMERIC(12, 4),
    length_cm NUMERIC(12, 4),
    width_cm NUMERIC(12, 4),
    height_cm NUMERIC(12, 4),
    tracking_number TEXT,
    label_printed BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cartons_pack_task ON cartons(pack_task_id);
CREATE INDEX IF NOT EXISTS idx_cartons_number ON cartons(carton_number);

CREATE TABLE IF NOT EXISTS carton_items (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    carton_id UUID NOT NULL REFERENCES cartons(id) ON DELETE CASCADE,
    sku TEXT NOT NULL,
    quantity NUMERIC(12, 4) NOT NULL,
    lot_id UUID,
    serial_number TEXT
);

CREATE INDEX IF NOT EXISTS idx_carton_items_carton ON carton_items(carton_id);

CREATE TABLE IF NOT EXISTS ship_tasks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_id UUID NOT NULL,
    shipment_id UUID NOT NULL,
    pack_task_id UUID NOT NULL REFERENCES pack_tasks(id),
    status TEXT NOT NULL DEFAULT 'pending',
    carrier TEXT,
    service_level TEXT,
    tracking_number TEXT,
    label_url TEXT,
    shipping_cost NUMERIC(12, 4),
    assigned_to TEXT,
    shipped_at TIMESTAMPTZ,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ship_tasks_order ON ship_tasks(order_id);
CREATE INDEX IF NOT EXISTS idx_ship_tasks_status ON ship_tasks(status);
CREATE INDEX IF NOT EXISTS idx_ship_tasks_carrier ON ship_tasks(carrier);
