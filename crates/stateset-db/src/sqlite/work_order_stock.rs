//! Transaction-scoped manufacturing stock and state transitions.
use super::{
    bins::smuggle, inventory::SqliteInventoryRepository, parse_decimal_row, parse_enum_row,
    parse_uuid_opt_row, parse_uuid_row,
};
use chrono::Utc;
use rusqlite::{Connection, Transaction, params};
use rust_decimal::Decimal;
use stateset_core::*;
use uuid::Uuid;

type DbResult<T> = rusqlite::Result<T>;
fn invalid(message: &str) -> rusqlite::Error {
    smuggle(CommerceError::ValidationError(message.into()))
}

pub(super) fn stock(conn: &Connection, id: Uuid) -> DbResult<Option<WorkOrderStock>> {
    conn.query_row("SELECT source_location_id, output_location_id, output_sku FROM manufacturing_work_orders WHERE id = ?1", [id.to_string()], |r| {
        let source: Option<i32> = r.get(0)?;
        let output: Option<i32> = r.get(1)?;
        let sku: Option<String> = r.get(2)?;
        match (source, output, sku) {
            (None, None, None) => Ok(None),
            (Some(source_location_id), Some(output_location_id), Some(output_sku)) => Ok(Some(WorkOrderStock { source_location_id, output_location_id, output_sku })),
            _ => Err(invalid("Incomplete work-order stock routing")),
        }
    })
}

pub(super) fn validate_stock(
    tx: &Transaction<'_>,
    product: ProductId,
    stock: &WorkOrderStock,
) -> DbResult<()> {
    let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM locations s JOIN warehouses sw ON sw.id = s.warehouse_id JOIN locations d ON d.id = ?2 JOIN warehouses dw ON dw.id = d.warehouse_id JOIN product_variants v ON v.product_id = ?3 AND v.sku = ?4 WHERE s.id = ?1 AND s.is_active = 1 AND sw.is_active = 1 AND d.is_active = 1 AND dw.is_active = 1 AND d.is_receivable = 1)", params![stock.source_location_id, stock.output_location_id, product.to_string(), stock.output_sku], |r| r.get(0))?;
    if !valid {
        return Err(invalid(
            "Work order needs active stock locations, a receivable output location, and an output SKU belonging to its product",
        ));
    }
    Ok(())
}

pub(super) fn save_stock(
    tx: &Transaction<'_>,
    id: Uuid,
    product: ProductId,
    stock: &WorkOrderStock,
) -> DbResult<()> {
    validate_stock(tx, product, stock)?;
    tx.execute("UPDATE manufacturing_work_orders SET source_location_id = ?1, output_location_id = ?2, output_sku = ?3 WHERE id = ?4", params![stock.source_location_id, stock.output_location_id, stock.output_sku, id.to_string()])?;
    Ok(())
}

struct Head {
    product: ProductId,
    status: WorkOrderStatus,
    target: Decimal,
    completed: Decimal,
}
fn head(tx: &Transaction<'_>, id: Uuid) -> DbResult<Head> {
    tx.query_row("SELECT product_id, status, quantity_to_build, quantity_completed FROM manufacturing_work_orders WHERE id = ?1", [id.to_string()], |r| Ok(Head {
        product: parse_uuid_row(&r.get::<_, String>(0)?, "work_order", "product_id")?.into(),
        status: parse_enum_row(&r.get::<_, String>(1)?, "work_order", "status")?,
        target: parse_decimal_row(&r.get::<_, String>(2)?, "work_order", "quantity_to_build")?,
        completed: parse_decimal_row(&r.get::<_, String>(3)?, "work_order", "quantity_completed")?,
    }))
}

struct Material {
    id: Uuid,
    sku: String,
    required: Decimal,
    consumed: Decimal,
    reservation: Option<Uuid>,
}
fn materials(tx: &Transaction<'_>, id: Uuid) -> DbResult<Vec<Material>> {
    let mut stmt = tx.prepare("SELECT id, component_sku, reserved_quantity, consumed_quantity, inventory_reservation_id FROM manufacturing_work_order_materials WHERE work_order_id = ?1 ORDER BY component_sku, id")?;
    stmt.query_map([id.to_string()], |r| {
        Ok(Material {
            id: parse_uuid_row(&r.get::<_, String>(0)?, "material", "id")?,
            sku: r.get(1)?,
            required: parse_decimal_row(&r.get::<_, String>(2)?, "material", "reserved_quantity")?,
            consumed: parse_decimal_row(&r.get::<_, String>(3)?, "material", "consumed_quantity")?,
            reservation: parse_uuid_opt_row(r.get(4)?, "material", "inventory_reservation_id")?,
        })
    })?
    .collect()
}

// Reserve physical shelf stock as well as its warehouse aggregate. Null-lot
// materials deliberately use the untracked row; lot-specific routing cannot
// silently borrow tracked stock from another lot.
fn reserve(tx: &Transaction<'_>, id: Uuid, routing: &WorkOrderStock) -> DbResult<()> {
    let warehouse = super::warehouse::warehouse_of_location_tx(tx, routing.source_location_id)?;
    for material in materials(tx, id)? {
        if material.reservation.is_some() {
            continue;
        }
        if material.consumed != Decimal::ZERO || material.required <= Decimal::ZERO {
            return Err(invalid(
                "Reconcile legacy material consumption before releasing this work order",
            ));
        }
        super::stock_guard::ensure_sellable(
            tx,
            &material.sku,
            None,
            None,
            Some(routing.source_location_id),
        )
        .map_err(smuggle)?;
        let (reservation, _) = SqliteInventoryRepository::reserve_for_line_in_tx(
            tx,
            &ReserveInventory {
                sku: material.sku,
                location_id: Some(warehouse),
                quantity: material.required,
                reference_type: "work_order".into(),
                reference_id: id.to_string(),
                expires_in_seconds: None,
            },
            None,
        )?;
        tx.execute("UPDATE manufacturing_work_order_materials SET inventory_reservation_id = ?1 WHERE id = ?2", params![reservation.id.to_string(), material.id.to_string()])?;
    }
    Ok(())
}

fn consume(
    tx: &Transaction<'_>,
    _id: Uuid,
    routing: &WorkOrderStock,
    material: &Material,
    quantity: Decimal,
) -> DbResult<()> {
    if quantity <= Decimal::ZERO {
        return Err(invalid("Consumed material quantity must be positive"));
    }
    let consumed = material
        .consumed
        .checked_add(quantity)
        .ok_or_else(|| invalid("Material consumption overflow"))?;
    if consumed > material.required {
        return Err(invalid("Cannot consume more material than reserved"));
    }
    let reservation = material.reservation.ok_or_else(|| {
        invalid("Material must be reserved by releasing the work order before consumption")
    })?;
    super::stock_guard::ensure_sellable(
        tx,
        &material.sku,
        None,
        None,
        Some(routing.source_location_id),
    )
    .map_err(smuggle)?;
    let now = Utc::now();
    SqliteInventoryRepository::fulfil_reservation_in_tx(
        tx,
        reservation,
        quantity,
        "manufacturing consumption",
        now,
    )
    .map_err(smuggle)?;
    tx.execute("UPDATE manufacturing_work_order_materials SET consumed_quantity = ?1, updated_at = ?2 WHERE id = ?3", params![consumed.to_string(), now.to_rfc3339(), material.id.to_string()])?;
    Ok(())
}

pub(super) fn transition(tx: &Transaction<'_>, id: Uuid, next: WorkOrderStatus) -> DbResult<()> {
    let current = head(tx, id)?;
    if current.status == next || !current.status.can_transition_to(next) {
        return Err(smuggle(CommerceError::Conflict(
            "Invalid work-order status transition".into(),
        )));
    }
    let routing = stock(tx, id)?;
    if next == WorkOrderStatus::InProgress {
        let routing = routing.as_ref().ok_or_else(|| invalid("Set explicit component and finished-goods stock locations before releasing the work order"))?;
        validate_stock(tx, current.product, routing)?;
        if current.target <= Decimal::ZERO {
            return Err(invalid("Quantity to build must be positive"));
        }
        reserve(tx, id, routing)?;
    }
    if next == WorkOrderStatus::Cancelled {
        for material in materials(tx, id)? {
            if let Some(reservation) = material.reservation {
                let remaining = material.required - material.consumed;
                if remaining > Decimal::ZERO {
                    routing
                        .as_ref()
                        .ok_or_else(|| invalid("Missing stock routing for reserved materials"))?;
                    SqliteInventoryRepository::release_reservation_in_tx(tx, reservation)?;
                }
            }
        }
    }
    let now = Utc::now().to_rfc3339();
    tx.execute("UPDATE manufacturing_work_orders SET status = ?1, actual_start = CASE WHEN ?1 = 'in_progress' THEN COALESCE(actual_start, ?2) ELSE actual_start END, updated_at = ?2, version = version + 1 WHERE id = ?3", params![next.to_string(), now, id.to_string()])?;
    Ok(())
}

pub(super) fn complete(tx: &Transaction<'_>, id: Uuid, quantity: Decimal) -> DbResult<()> {
    let current = head(tx, id)?;
    if !matches!(current.status, WorkOrderStatus::InProgress | WorkOrderStatus::PartiallyCompleted)
    {
        return Err(smuggle(CommerceError::Conflict(
            "Only running work orders can complete units".into(),
        )));
    }
    if quantity <= Decimal::ZERO {
        return Err(invalid("Completed quantity must be positive"));
    }
    let completed = current
        .completed
        .checked_add(quantity)
        .ok_or_else(|| invalid("Completed quantity overflow"))?;
    if completed > current.target {
        return Err(invalid("Completed quantity would exceed quantity to build"));
    }
    let routing = stock(tx, id)?.ok_or_else(|| invalid("Work order has no stock routing"))?;
    validate_stock(tx, current.product, &routing)?;
    for material in materials(tx, id)? {
        // Compute against the cumulative total so fractional partial completions
        // cannot accumulate rounding loss. The final completion consumes exactly
        // the remaining requirement, including any explicitly recorded use.
        let target = if completed == current.target {
            material.required
        } else {
            material
                .required
                .checked_mul(completed)
                .and_then(|v| v.checked_div(current.target))
                .ok_or_else(|| invalid("Material quantity overflow"))?
        };
        if target > material.consumed {
            consume(tx, id, &routing, &material, target - material.consumed)?;
        }
    }
    let now = Utc::now().to_rfc3339();
    super::stock_move::post(
        tx,
        &crate::stock_move::StockMove {
            id: Uuid::new_v4(),
            sku: &routing.output_sku,
            lot_id: None,
            from: None,
            to: Some(routing.output_location_id),
            quantity,
            release_reserved: Decimal::ZERO,
            reserve_received: Decimal::ZERO,
            movement_type: MovementType::Receipt,
            lot_effect: crate::stock_move::LotEffect::Track,
            check_sellable: false,
            reference_type: Some("work_order_output"),
            reference_id: Some(id),
            reference_label: None,
            reason: Some("manufacturing output"),
            performed_by: None,
        },
        &now,
    )?;
    let status = if completed == current.target { "completed" } else { "partially_completed" };
    tx.execute("UPDATE manufacturing_work_orders SET quantity_completed = ?1, status = ?2, actual_end = CASE WHEN ?2 = 'completed' THEN ?3 ELSE actual_end END, updated_at = ?3, version = version + 1 WHERE id = ?4", params![completed.to_string(), status, now, id.to_string()])?;
    Ok(())
}

pub(super) fn consume_material(
    tx: &Transaction<'_>,
    material_id: Uuid,
    quantity: Decimal,
) -> DbResult<()> {
    let id: Uuid = tx.query_row(
        "SELECT work_order_id FROM manufacturing_work_order_materials WHERE id = ?1",
        [material_id.to_string()],
        |r| parse_uuid_row(&r.get::<_, String>(0)?, "material", "work_order_id"),
    )?;
    let current = head(tx, id)?;
    if !matches!(current.status, WorkOrderStatus::InProgress | WorkOrderStatus::PartiallyCompleted)
    {
        return Err(smuggle(CommerceError::Conflict(
            "Materials can be consumed only while the work order is running".into(),
        )));
    }
    let routing = stock(tx, id)?.ok_or_else(|| invalid("Work order has no stock routing"))?;
    let material = materials(tx, id)?
        .into_iter()
        .find(|m| m.id == material_id)
        .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    consume(tx, id, &routing, &material, quantity)
}

pub(super) fn add_material(
    tx: &Transaction<'_>,
    id: Uuid,
    material: &AddWorkOrderMaterial,
) -> DbResult<Uuid> {
    if head(tx, id)?.status != WorkOrderStatus::Planned {
        return Err(invalid("Material requirements can be edited only while planned"));
    }
    if material.quantity <= Decimal::ZERO || material.component_sku.trim().is_empty() {
        return Err(invalid("Materials require a SKU and positive quantity"));
    }
    let material_id = Uuid::new_v4();
    let now = Utc::now().to_rfc3339();
    tx.execute("INSERT INTO manufacturing_work_order_materials (id, work_order_id, component_id, component_sku, component_name, reserved_quantity, consumed_quantity, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, '0', ?7, ?7)", params![material_id.to_string(), id.to_string(), material.component_id.map(|v| v.to_string()), material.component_sku, material.component_name, material.quantity.to_string(), now])?;
    Ok(material_id)
}

pub(super) fn create(tx: &Transaction<'_>, id: Uuid, input: &CreateWorkOrder) -> DbResult<()> {
    if input.quantity_to_build <= Decimal::ZERO {
        return Err(invalid("Quantity to build must be positive"));
    }
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM products WHERE id = ?1)",
        [input.product_id.to_string()],
        |r| r.get(0),
    )?;
    if !exists {
        return Err(invalid("Work-order product does not exist"));
    }
    if let Some(bom) = input.bom_id {
        let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM manufacturing_boms WHERE id = ?1 AND product_id = ?2 AND status = 'active')", params![bom.to_string(), input.product_id.to_string()], |r| r.get(0))?;
        if !valid {
            return Err(invalid("Work order requires an active BOM belonging to its product"));
        }
    }
    let now = Utc::now().to_rfc3339();
    tx.execute("INSERT INTO manufacturing_work_orders (id, work_order_number, product_id, bom_id, work_center_id, assigned_to, status, priority, quantity_to_build, quantity_completed, scheduled_start, scheduled_end, notes, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'planned', ?7, ?8, '0', ?9, ?10, ?11, ?12, ?12)", params![id.to_string(), WorkOrder::generate_work_order_number(), input.product_id.to_string(), input.bom_id.map(|v| v.to_string()), input.work_center_id, input.assigned_to.map(|v| v.to_string()), input.priority.unwrap_or_default().to_string(), input.quantity_to_build.to_string(), input.scheduled_start.map(|v| v.to_rfc3339()), input.scheduled_end.map(|v| v.to_rfc3339()), input.notes, now])?;
    if let Some(routing) = &input.stock {
        save_stock(tx, id, input.product_id, routing)?;
    }
    if let Some(bom) = input.bom_id {
        let mut stmt = tx.prepare("SELECT id, component_product_id, component_sku, name, quantity FROM manufacturing_bom_components WHERE bom_id = ?1 ORDER BY id")?;
        let components = stmt
            .query_map([bom.to_string()], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    parse_decimal_row(&r.get::<_, String>(4)?, "bom_component", "quantity")?,
                ))
            })?
            .collect::<DbResult<Vec<_>>>()?;
        if components.is_empty() {
            return Err(invalid("An active BOM must have material requirements"));
        }
        for (component, product, sku, name, quantity) in components {
            if quantity <= Decimal::ZERO {
                return Err(invalid("BOM component quantity must be positive"));
            }
            let sku = match sku.filter(|v| !v.trim().is_empty()) {
                Some(sku) => {
                    if let Some(product) = &product {
                        let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM product_variants WHERE product_id = ?1 AND sku = ?2)", params![product, sku], |r| r.get(0))?;
                        if !valid {
                            return Err(invalid(
                                "BOM component SKU does not belong to its product",
                            ));
                        }
                    }
                    sku
                }
                None => {
                    let product = product
                        .ok_or_else(|| invalid("BOM component requires a SKU or product"))?;
                    let mut stmt = tx.prepare(
                        "SELECT sku FROM product_variants WHERE product_id = ?1 ORDER BY sku",
                    )?;
                    let skus = stmt
                        .query_map([product], |r| r.get::<_, String>(0))?
                        .collect::<DbResult<Vec<_>>>()?;
                    if skus.len() != 1 {
                        return Err(invalid(
                            "BOM component needs an explicit SKU when its product has multiple variants",
                        ));
                    }
                    skus[0].clone()
                }
            };
            let quantity = quantity
                .checked_mul(input.quantity_to_build)
                .ok_or_else(|| invalid("Material requirement overflow"))?;
            add_material(
                tx,
                id,
                &AddWorkOrderMaterial {
                    component_id: Some(
                        super::parse_uuid(&component, "bom_component", "id").map_err(smuggle)?,
                    ),
                    component_sku: sku,
                    component_name: name,
                    quantity,
                },
            )?;
        }
    }
    for task in input.tasks.as_deref().unwrap_or_default() {
        if task.task_name.trim().is_empty()
            || task.estimated_hours.is_some_and(|v| v < Decimal::ZERO)
        {
            return Err(invalid("Task name and estimated hours are invalid"));
        }
        tx.execute("INSERT INTO manufacturing_work_order_tasks (id, work_order_id, sequence, task_name, status, estimated_hours, assigned_to, notes, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'pending', ?5, ?6, ?7, ?8, ?8)", params![Uuid::new_v4().to_string(), id.to_string(), task.sequence.unwrap_or(1), task.task_name, task.estimated_hours.map(|v| v.to_string()), task.assigned_to.map(|v| v.to_string()), task.notes, now])?;
    }
    Ok(())
}

pub(super) fn update(tx: &Transaction<'_>, id: Uuid, input: &UpdateWorkOrder) -> DbResult<()> {
    let current = head(tx, id)?;
    if input.status.is_some()
        || input.quantity_completed.is_some()
        || input.actual_start.is_some()
        || input.actual_end.is_some()
    {
        return Err(invalid(
            "Use work-order lifecycle operations to change status, completion, or actual dates",
        ));
    }
    if (input.stock.is_some() || input.quantity_to_build.is_some())
        && current.status != WorkOrderStatus::Planned
    {
        return Err(invalid("Stock routing and build quantity can change only while planned"));
    }
    if let Some(routing) = &input.stock {
        save_stock(tx, id, current.product, routing)?;
    }
    if let Some(quantity) = input.quantity_to_build {
        if quantity <= Decimal::ZERO || current.target <= Decimal::ZERO {
            return Err(invalid("Quantity to build must be positive"));
        }
        for material in materials(tx, id)? {
            let required = material
                .required
                .checked_mul(quantity)
                .and_then(|v| v.checked_div(current.target))
                .ok_or_else(|| invalid("Material requirement overflow"))?;
            if required <= Decimal::ZERO {
                return Err(invalid("Material quantity is below decimal precision"));
            }
            tx.execute("UPDATE manufacturing_work_order_materials SET reserved_quantity = ?1 WHERE id = ?2", params![required.to_string(), material.id.to_string()])?;
        }
    }
    // COALESCE applies only supplied fields. No status or completed-quantity
    // snapshot is written back over a concurrent lifecycle operation.
    tx.execute("UPDATE manufacturing_work_orders SET work_center_id = COALESCE(?1, work_center_id), assigned_to = COALESCE(?2, assigned_to), priority = COALESCE(?3, priority), scheduled_start = COALESCE(?4, scheduled_start), scheduled_end = COALESCE(?5, scheduled_end), notes = COALESCE(?6, notes), quantity_to_build = COALESCE(?7, quantity_to_build), updated_at = ?8, version = version + 1 WHERE id = ?9", params![input.work_center_id, input.assigned_to.map(|v| v.to_string()), input.priority.map(|v| v.to_string()), input.scheduled_start.map(|v| v.to_rfc3339()), input.scheduled_end.map(|v| v.to_rfc3339()), input.notes, input.quantity_to_build.map(|v| v.to_string()), Utc::now().to_rfc3339(), id.to_string()])?;
    Ok(())
}
