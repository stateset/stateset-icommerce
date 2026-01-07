-- Fulfillment (Pick/Pack/Ship) tables

-- Waves (groups orders for efficient picking)
CREATE TABLE IF NOT EXISTS waves (
    id TEXT PRIMARY KEY,
    wave_number TEXT UNIQUE NOT NULL,
    warehouse_id INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    order_count INTEGER NOT NULL DEFAULT 0,
    pick_count INTEGER NOT NULL DEFAULT 0,
    completed_pick_count INTEGER NOT NULL DEFAULT 0,
    priority INTEGER NOT NULL DEFAULT 0,
    started_at TEXT,
    completed_at TEXT,
    notes TEXT,
    created_by TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (warehouse_id) REFERENCES warehouses(id)
);

CREATE INDEX IF NOT EXISTS idx_waves_number ON waves(wave_number);
CREATE INDEX IF NOT EXISTS idx_waves_status ON waves(status);
CREATE INDEX IF NOT EXISTS idx_waves_warehouse ON waves(warehouse_id);

-- Wave orders (junction table)
CREATE TABLE IF NOT EXISTS wave_orders (
    wave_id TEXT NOT NULL,
    order_id TEXT NOT NULL,
    PRIMARY KEY (wave_id, order_id),
    FOREIGN KEY (wave_id) REFERENCES waves(id) ON DELETE CASCADE
);

-- Pick tasks
CREATE TABLE IF NOT EXISTS pick_tasks (
    id TEXT PRIMARY KEY,
    wave_id TEXT,
    order_id TEXT NOT NULL,
    order_item_id TEXT NOT NULL,
    warehouse_id INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    sku TEXT NOT NULL,
    product_name TEXT,
    source_location_id INTEGER NOT NULL,
    source_location_code TEXT,
    quantity_requested TEXT NOT NULL,
    quantity_picked TEXT NOT NULL DEFAULT '0',
    quantity_short TEXT NOT NULL DEFAULT '0',
    lot_id TEXT,
    serial_number TEXT,
    assigned_to TEXT,
    priority INTEGER NOT NULL DEFAULT 0,
    pick_sequence INTEGER NOT NULL DEFAULT 0,
    started_at TEXT,
    completed_at TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (wave_id) REFERENCES waves(id),
    FOREIGN KEY (warehouse_id) REFERENCES warehouses(id),
    FOREIGN KEY (source_location_id) REFERENCES locations(id)
);

CREATE INDEX IF NOT EXISTS idx_pick_tasks_wave ON pick_tasks(wave_id);
CREATE INDEX IF NOT EXISTS idx_pick_tasks_order ON pick_tasks(order_id);
CREATE INDEX IF NOT EXISTS idx_pick_tasks_status ON pick_tasks(status);
CREATE INDEX IF NOT EXISTS idx_pick_tasks_assigned ON pick_tasks(assigned_to);
CREATE INDEX IF NOT EXISTS idx_pick_tasks_location ON pick_tasks(source_location_id);

-- Pack tasks
CREATE TABLE IF NOT EXISTS pack_tasks (
    id TEXT PRIMARY KEY,
    order_id TEXT NOT NULL,
    shipment_id TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    carton_count INTEGER NOT NULL DEFAULT 0,
    total_weight_kg TEXT,
    assigned_to TEXT,
    packing_station TEXT,
    started_at TEXT,
    completed_at TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pack_tasks_order ON pack_tasks(order_id);
CREATE INDEX IF NOT EXISTS idx_pack_tasks_status ON pack_tasks(status);
CREATE INDEX IF NOT EXISTS idx_pack_tasks_assigned ON pack_tasks(assigned_to);

-- Cartons
CREATE TABLE IF NOT EXISTS cartons (
    id TEXT PRIMARY KEY,
    pack_task_id TEXT NOT NULL,
    carton_number TEXT NOT NULL,
    package_type TEXT NOT NULL DEFAULT 'box',
    weight_kg TEXT,
    length_cm TEXT,
    width_cm TEXT,
    height_cm TEXT,
    tracking_number TEXT,
    label_printed INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    FOREIGN KEY (pack_task_id) REFERENCES pack_tasks(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_cartons_pack_task ON cartons(pack_task_id);
CREATE INDEX IF NOT EXISTS idx_cartons_number ON cartons(carton_number);

-- Carton items
CREATE TABLE IF NOT EXISTS carton_items (
    id TEXT PRIMARY KEY,
    carton_id TEXT NOT NULL,
    sku TEXT NOT NULL,
    quantity TEXT NOT NULL,
    lot_id TEXT,
    serial_number TEXT,
    FOREIGN KEY (carton_id) REFERENCES cartons(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_carton_items_carton ON carton_items(carton_id);

-- Ship tasks
CREATE TABLE IF NOT EXISTS ship_tasks (
    id TEXT PRIMARY KEY,
    order_id TEXT NOT NULL,
    shipment_id TEXT NOT NULL,
    pack_task_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    carrier TEXT,
    service_level TEXT,
    tracking_number TEXT,
    label_url TEXT,
    shipping_cost TEXT,
    assigned_to TEXT,
    shipped_at TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (pack_task_id) REFERENCES pack_tasks(id)
);

CREATE INDEX IF NOT EXISTS idx_ship_tasks_order ON ship_tasks(order_id);
CREATE INDEX IF NOT EXISTS idx_ship_tasks_status ON ship_tasks(status);
CREATE INDEX IF NOT EXISTS idx_ship_tasks_carrier ON ship_tasks(carrier);

-- Auto-update timestamps
CREATE TRIGGER IF NOT EXISTS waves_updated_at
AFTER UPDATE ON waves
BEGIN
    UPDATE waves SET updated_at = datetime('now') WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS pick_tasks_updated_at
AFTER UPDATE ON pick_tasks
BEGIN
    UPDATE pick_tasks SET updated_at = datetime('now') WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS pack_tasks_updated_at
AFTER UPDATE ON pack_tasks
BEGIN
    UPDATE pack_tasks SET updated_at = datetime('now') WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS ship_tasks_updated_at
AFTER UPDATE ON ship_tasks
BEGIN
    UPDATE ship_tasks SET updated_at = datetime('now') WHERE id = NEW.id;
END;
