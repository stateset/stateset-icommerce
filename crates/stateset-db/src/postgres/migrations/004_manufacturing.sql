-- Manufacturing tables: BOM and Work Orders for PostgreSQL

-- Bill of Materials (BOM)
CREATE TABLE IF NOT EXISTS manufacturing_boms (
    id UUID PRIMARY KEY,
    bom_number TEXT NOT NULL UNIQUE,
    product_id UUID NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    revision TEXT NOT NULL DEFAULT 'A',
    status TEXT NOT NULL DEFAULT 'draft',
    created_by UUID,
    updated_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_boms_product_id ON manufacturing_boms(product_id);
CREATE INDEX IF NOT EXISTS idx_boms_status ON manufacturing_boms(status);
CREATE INDEX IF NOT EXISTS idx_boms_bom_number ON manufacturing_boms(bom_number);

-- BOM Components
CREATE TABLE IF NOT EXISTS manufacturing_bom_components (
    id UUID PRIMARY KEY,
    bom_id UUID NOT NULL REFERENCES manufacturing_boms(id) ON DELETE CASCADE,
    component_product_id UUID,
    component_sku TEXT,
    name TEXT NOT NULL,
    quantity DECIMAL NOT NULL,
    unit_of_measure TEXT NOT NULL DEFAULT 'each',
    position TEXT,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_bom_components_bom_id ON manufacturing_bom_components(bom_id);
CREATE INDEX IF NOT EXISTS idx_bom_components_sku ON manufacturing_bom_components(component_sku);

-- Work Orders
CREATE TABLE IF NOT EXISTS manufacturing_work_orders (
    id UUID PRIMARY KEY,
    work_order_number TEXT NOT NULL UNIQUE,
    product_id UUID NOT NULL,
    bom_id UUID REFERENCES manufacturing_boms(id),
    work_center_id TEXT,
    assigned_to UUID,
    status TEXT NOT NULL DEFAULT 'planned',
    priority TEXT NOT NULL DEFAULT 'normal',
    quantity_to_build DECIMAL NOT NULL,
    quantity_completed DECIMAL NOT NULL DEFAULT 0,
    scheduled_start TIMESTAMPTZ,
    scheduled_end TIMESTAMPTZ,
    actual_start TIMESTAMPTZ,
    actual_end TIMESTAMPTZ,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_work_orders_product_id ON manufacturing_work_orders(product_id);
CREATE INDEX IF NOT EXISTS idx_work_orders_bom_id ON manufacturing_work_orders(bom_id);
CREATE INDEX IF NOT EXISTS idx_work_orders_status ON manufacturing_work_orders(status);
CREATE INDEX IF NOT EXISTS idx_work_orders_priority ON manufacturing_work_orders(priority);
CREATE INDEX IF NOT EXISTS idx_work_orders_assigned_to ON manufacturing_work_orders(assigned_to);
CREATE INDEX IF NOT EXISTS idx_work_orders_number ON manufacturing_work_orders(work_order_number);

-- Work Order Tasks
CREATE TABLE IF NOT EXISTS manufacturing_work_order_tasks (
    id UUID PRIMARY KEY,
    work_order_id UUID NOT NULL REFERENCES manufacturing_work_orders(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL DEFAULT 1,
    task_name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    estimated_hours DECIMAL,
    actual_hours DECIMAL,
    assigned_to UUID,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_work_order_tasks_work_order_id ON manufacturing_work_order_tasks(work_order_id);
CREATE INDEX IF NOT EXISTS idx_work_order_tasks_status ON manufacturing_work_order_tasks(status);

-- Work Order Materials
CREATE TABLE IF NOT EXISTS manufacturing_work_order_materials (
    id UUID PRIMARY KEY,
    work_order_id UUID NOT NULL REFERENCES manufacturing_work_orders(id) ON DELETE CASCADE,
    component_id UUID,
    component_sku TEXT NOT NULL,
    component_name TEXT NOT NULL,
    reserved_quantity DECIMAL NOT NULL DEFAULT 0,
    consumed_quantity DECIMAL NOT NULL DEFAULT 0,
    inventory_reservation_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_work_order_materials_work_order_id ON manufacturing_work_order_materials(work_order_id);
CREATE INDEX IF NOT EXISTS idx_work_order_materials_sku ON manufacturing_work_order_materials(component_sku);
