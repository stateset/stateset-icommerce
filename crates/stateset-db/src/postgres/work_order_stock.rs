//! Transaction-scoped manufacturing stock and state transitions.
use super::{inventory::PgInventoryRepository, map_db_error};
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::{Postgres, Transaction};
use stateset_core::*;
use uuid::Uuid;

fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}

pub(super) async fn validate_stock(
    tx: &mut Transaction<'_, Postgres>,
    product: ProductId,
    stock: &WorkOrderStock,
) -> Result<()> {
    let locations: Vec<i32> = sqlx::query_scalar("SELECT l.id FROM locations l JOIN warehouses w ON w.id = l.warehouse_id WHERE l.id = ANY($1) AND l.is_active AND w.is_active AND (l.id != $2 OR l.is_receivable) ORDER BY l.id FOR SHARE OF l, w")
        .bind(vec![stock.source_location_id, stock.output_location_id]).bind(stock.output_location_id).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    let expected = if stock.source_location_id == stock.output_location_id { 1 } else { 2 };
    let variant: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM product_variants WHERE product_id = $1 AND sku = $2 FOR SHARE",
    )
    .bind(product)
    .bind(&stock.output_sku)
    .fetch_optional(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    if locations.len() != expected || variant.is_none() {
        return Err(invalid(
            "Work order needs active stock locations, a receivable output location, and an output SKU belonging to its product",
        ));
    }
    Ok(())
}

pub(super) async fn save_stock(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    product: ProductId,
    stock: &WorkOrderStock,
) -> Result<()> {
    validate_stock(tx, product, stock).await?;
    sqlx::query("UPDATE manufacturing_work_orders SET source_location_id = $1, output_location_id = $2, output_sku = $3 WHERE id = $4").bind(stock.source_location_id).bind(stock.output_location_id).bind(&stock.output_sku).bind(id).execute(tx.as_mut()).await.map_err(map_db_error)?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct Head {
    product_id: Uuid,
    status: String,
    quantity_to_build: Decimal,
    quantity_completed: Decimal,
    source_location_id: Option<i32>,
    output_location_id: Option<i32>,
    output_sku: Option<String>,
}
impl Head {
    fn status(&self) -> Result<WorkOrderStatus> {
        self.status.parse().map_err(|_| invalid("Invalid stored work-order status"))
    }
    fn stock(&self) -> Result<WorkOrderStock> {
        match (self.source_location_id, self.output_location_id, &self.output_sku) {
            (Some(source_location_id), Some(output_location_id), Some(output_sku)) => {
                Ok(WorkOrderStock {
                    source_location_id,
                    output_location_id,
                    output_sku: output_sku.clone(),
                })
            }
            _ => Err(invalid(
                "Set explicit component and finished-goods stock locations before releasing the work order",
            )),
        }
    }
}
async fn head(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<Head> {
    sqlx::query_as("SELECT product_id, status, quantity_to_build, quantity_completed, source_location_id, output_location_id, output_sku FROM manufacturing_work_orders WHERE id = $1 FOR UPDATE").bind(id).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?.ok_or(CommerceError::NotFound)
}

#[derive(sqlx::FromRow)]
struct Material {
    id: Uuid,
    component_sku: String,
    reserved_quantity: Decimal,
    consumed_quantity: Decimal,
    inventory_reservation_id: Option<Uuid>,
}
async fn materials(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<Vec<Material>> {
    sqlx::query_as("SELECT id, component_sku, reserved_quantity, consumed_quantity, inventory_reservation_id FROM manufacturing_work_order_materials WHERE work_order_id = $1 ORDER BY component_sku, id FOR UPDATE").bind(id).fetch_all(tx.as_mut()).await.map_err(map_db_error)
}

async fn reserve(
    tx: &mut Transaction<'_, Postgres>,
    inventory: &PgInventoryRepository,
    id: Uuid,
    routing: &WorkOrderStock,
) -> Result<()> {
    let warehouse =
        super::warehouse::warehouse_of_location_pg(tx.as_mut(), routing.source_location_id).await?;
    for material in materials(tx, id).await? {
        if material.inventory_reservation_id.is_some() {
            continue;
        }
        if material.consumed_quantity != Decimal::ZERO
            || material.reserved_quantity <= Decimal::ZERO
        {
            return Err(invalid(
                "Reconcile legacy material consumption before releasing this work order",
            ));
        }
        super::stock_guard::ensure_sellable(
            tx.as_mut(),
            &material.component_sku,
            None,
            None,
            Some(routing.source_location_id),
        )
        .await?;
        let (reservation, _) = inventory
            .reserve_for_line_in_tx(
                tx,
                &ReserveInventory {
                    sku: material.component_sku.clone(),
                    location_id: Some(warehouse),
                    quantity: material.reserved_quantity,
                    reference_type: "work_order".into(),
                    reference_id: id.to_string(),
                    expires_in_seconds: None,
                },
                None,
            )
            .await?;
        sqlx::query("UPDATE manufacturing_work_order_materials SET inventory_reservation_id = $1 WHERE id = $2").bind(reservation.id).bind(material.id).execute(tx.as_mut()).await.map_err(map_db_error)?;
    }
    Ok(())
}

async fn consume(
    tx: &mut Transaction<'_, Postgres>,
    _id: Uuid,
    routing: &WorkOrderStock,
    material: &Material,
    quantity: Decimal,
) -> Result<()> {
    if quantity <= Decimal::ZERO {
        return Err(invalid("Consumed material quantity must be positive"));
    }
    let consumed = material
        .consumed_quantity
        .checked_add(quantity)
        .ok_or_else(|| invalid("Material consumption overflow"))?;
    if consumed > material.reserved_quantity {
        return Err(invalid("Cannot consume more material than reserved"));
    }
    let reservation = material.inventory_reservation_id.ok_or_else(|| {
        invalid("Material must be reserved by releasing the work order before consumption")
    })?;
    super::stock_guard::ensure_sellable(
        tx.as_mut(),
        &material.component_sku,
        None,
        None,
        Some(routing.source_location_id),
    )
    .await?;
    let now = Utc::now();
    PgInventoryRepository::fulfil_reservation_in_tx(
        tx,
        reservation,
        quantity,
        "manufacturing consumption",
        now,
    )
    .await?;
    sqlx::query("UPDATE manufacturing_work_order_materials SET consumed_quantity = $1, updated_at = $2 WHERE id = $3").bind(consumed).bind(now).bind(material.id).execute(tx.as_mut()).await.map_err(map_db_error)?;
    Ok(())
}

pub(super) async fn transition(
    tx: &mut Transaction<'_, Postgres>,
    inventory: &PgInventoryRepository,
    id: Uuid,
    next: WorkOrderStatus,
) -> Result<()> {
    let current = head(tx, id).await?;
    let status = current.status()?;
    if status == next || !status.can_transition_to(next) {
        return Err(CommerceError::Conflict("Invalid work-order status transition".into()));
    }
    if next == WorkOrderStatus::InProgress {
        let routing = current.stock()?;
        validate_stock(tx, current.product_id.into(), &routing).await?;
        if current.quantity_to_build <= Decimal::ZERO {
            return Err(invalid("Quantity to build must be positive"));
        }
        reserve(tx, inventory, id, &routing).await?;
    }
    if next == WorkOrderStatus::Cancelled {
        for material in materials(tx, id).await? {
            if let Some(reservation) = material.inventory_reservation_id {
                let remaining = material.reserved_quantity - material.consumed_quantity;
                if remaining > Decimal::ZERO {
                    current.stock()?;
                    inventory.release_reservation_in_tx(tx, reservation).await?;
                }
            }
        }
    }
    let now = Utc::now();
    sqlx::query("UPDATE manufacturing_work_orders SET status = $1, actual_start = CASE WHEN $1 = 'in_progress' THEN COALESCE(actual_start, $2) ELSE actual_start END, updated_at = $2, version = version + 1 WHERE id = $3").bind(next.to_string()).bind(now).bind(id).execute(tx.as_mut()).await.map_err(map_db_error)?;
    Ok(())
}

pub(super) async fn complete(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    quantity: Decimal,
) -> Result<()> {
    let current = head(tx, id).await?;
    if !matches!(
        current.status()?,
        WorkOrderStatus::InProgress | WorkOrderStatus::PartiallyCompleted
    ) {
        return Err(CommerceError::Conflict("Only running work orders can complete units".into()));
    }
    if quantity <= Decimal::ZERO {
        return Err(invalid("Completed quantity must be positive"));
    }
    let completed = current
        .quantity_completed
        .checked_add(quantity)
        .ok_or_else(|| invalid("Completed quantity overflow"))?;
    if completed > current.quantity_to_build {
        return Err(invalid("Completed quantity would exceed quantity to build"));
    }
    let routing = current.stock()?;
    validate_stock(tx, current.product_id.into(), &routing).await?;
    for material in materials(tx, id).await? {
        let target = if completed == current.quantity_to_build {
            material.reserved_quantity
        } else {
            material
                .reserved_quantity
                .checked_mul(completed)
                .and_then(|v| v.checked_div(current.quantity_to_build))
                .ok_or_else(|| invalid("Material quantity overflow"))?
        };
        if target > material.consumed_quantity {
            consume(tx, id, &routing, &material, target - material.consumed_quantity).await?;
        }
    }
    let now = Utc::now();
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
        now,
    )
    .await?;
    let status =
        if completed == current.quantity_to_build { "completed" } else { "partially_completed" };
    sqlx::query("UPDATE manufacturing_work_orders SET quantity_completed = $1, status = $2, actual_end = CASE WHEN $2 = 'completed' THEN $3 ELSE actual_end END, updated_at = $3, version = version + 1 WHERE id = $4").bind(completed).bind(status).bind(now).bind(id).execute(tx.as_mut()).await.map_err(map_db_error)?;
    Ok(())
}

pub(super) async fn consume_material(
    tx: &mut Transaction<'_, Postgres>,
    material_id: Uuid,
    quantity: Decimal,
) -> Result<()> {
    // Acquire the parent before its materials, matching completion/release order.
    let id: Uuid = sqlx::query_scalar(
        "SELECT work_order_id FROM manufacturing_work_order_materials WHERE id = $1",
    )
    .bind(material_id)
    .fetch_optional(tx.as_mut())
    .await
    .map_err(map_db_error)?
    .ok_or(CommerceError::NotFound)?;
    let current = head(tx, id).await?;
    if !matches!(
        current.status()?,
        WorkOrderStatus::InProgress | WorkOrderStatus::PartiallyCompleted
    ) {
        return Err(CommerceError::Conflict(
            "Materials can be consumed only while the work order is running".into(),
        ));
    }
    let routing = current.stock()?;
    let material = materials(tx, id)
        .await?
        .into_iter()
        .find(|m| m.id == material_id)
        .ok_or(CommerceError::NotFound)?;
    consume(tx, id, &routing, &material, quantity).await
}

pub(super) async fn add_material(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    material: &AddWorkOrderMaterial,
) -> Result<Uuid> {
    if head(tx, id).await?.status()? != WorkOrderStatus::Planned {
        return Err(invalid("Material requirements can be edited only while planned"));
    }
    if material.quantity <= Decimal::ZERO || material.component_sku.trim().is_empty() {
        return Err(invalid("Materials require a SKU and positive quantity"));
    }
    let material_id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query("INSERT INTO manufacturing_work_order_materials (id, work_order_id, component_id, component_sku, component_name, reserved_quantity, consumed_quantity, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, 0, $7, $7)").bind(material_id).bind(id).bind(material.component_id).bind(&material.component_sku).bind(&material.component_name).bind(material.quantity).bind(now).execute(tx.as_mut()).await.map_err(map_db_error)?;
    Ok(material_id)
}

pub(super) async fn create(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    input: &CreateWorkOrder,
) -> Result<()> {
    if input.quantity_to_build <= Decimal::ZERO {
        return Err(invalid("Quantity to build must be positive"));
    }
    let product: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM products WHERE id = $1 FOR SHARE")
            .bind(input.product_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?;
    if product.is_none() {
        return Err(invalid("Work-order product does not exist"));
    }
    if let Some(bom) = input.bom_id {
        let valid: Option<Uuid> = sqlx::query_scalar("SELECT id FROM manufacturing_boms WHERE id = $1 AND product_id = $2 AND status = 'active' FOR SHARE").bind(bom).bind(input.product_id).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?;
        if valid.is_none() {
            return Err(invalid("Work order requires an active BOM belonging to its product"));
        }
    }
    let now = Utc::now();
    sqlx::query("INSERT INTO manufacturing_work_orders (id, work_order_number, product_id, bom_id, work_center_id, assigned_to, status, priority, quantity_to_build, quantity_completed, scheduled_start, scheduled_end, notes, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, 'planned', $7, $8, 0, $9, $10, $11, $12, $12)").bind(id).bind(WorkOrder::generate_work_order_number()).bind(input.product_id).bind(input.bom_id).bind(&input.work_center_id).bind(input.assigned_to).bind(input.priority.unwrap_or_default().to_string()).bind(input.quantity_to_build).bind(input.scheduled_start).bind(input.scheduled_end).bind(&input.notes).bind(now).execute(tx.as_mut()).await.map_err(map_db_error)?;
    if let Some(routing) = &input.stock {
        save_stock(tx, id, input.product_id, routing).await?;
    }
    if let Some(bom) = input.bom_id {
        type MaterialRow = (Uuid, Option<Uuid>, Option<String>, String, Decimal);
        let components: Vec<MaterialRow> = sqlx::query_as("SELECT id, component_product_id, component_sku, name, quantity FROM manufacturing_bom_components WHERE bom_id = $1 ORDER BY id FOR SHARE").bind(bom).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
        if components.is_empty() {
            return Err(invalid("An active BOM must have material requirements"));
        }
        for (component, product, sku, name, quantity) in components {
            if quantity <= Decimal::ZERO {
                return Err(invalid("BOM component quantity must be positive"));
            }
            let sku = match sku.filter(|v| !v.trim().is_empty()) {
                Some(sku) => {
                    if let Some(product) = product {
                        let valid: Option<Uuid> = sqlx::query_scalar("SELECT id FROM product_variants WHERE product_id = $1 AND sku = $2 FOR SHARE").bind(product).bind(&sku).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?;
                        if valid.is_none() {
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
                    let skus: Vec<String> = sqlx::query_scalar("SELECT sku FROM product_variants WHERE product_id = $1 ORDER BY sku FOR SHARE").bind(product).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
                    component_id: Some(component),
                    component_sku: sku,
                    component_name: name,
                    quantity,
                },
            )
            .await?;
        }
    }
    for task in input.tasks.as_deref().unwrap_or_default() {
        if task.task_name.trim().is_empty()
            || task.estimated_hours.is_some_and(|v| v < Decimal::ZERO)
        {
            return Err(invalid("Task name and estimated hours are invalid"));
        }
        sqlx::query("INSERT INTO manufacturing_work_order_tasks (id, work_order_id, sequence, task_name, status, estimated_hours, assigned_to, notes, created_at, updated_at) VALUES ($1, $2, $3, $4, 'pending', $5, $6, $7, $8, $8)").bind(Uuid::new_v4()).bind(id).bind(task.sequence.unwrap_or(1)).bind(&task.task_name).bind(task.estimated_hours).bind(task.assigned_to).bind(&task.notes).bind(now).execute(tx.as_mut()).await.map_err(map_db_error)?;
    }
    Ok(())
}

pub(super) async fn update(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    input: &UpdateWorkOrder,
) -> Result<()> {
    let current = head(tx, id).await?;
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
        && current.status()? != WorkOrderStatus::Planned
    {
        return Err(invalid("Stock routing and build quantity can change only while planned"));
    }
    if let Some(routing) = &input.stock {
        save_stock(tx, id, current.product_id.into(), routing).await?;
    }
    if let Some(quantity) = input.quantity_to_build {
        if quantity <= Decimal::ZERO || current.quantity_to_build <= Decimal::ZERO {
            return Err(invalid("Quantity to build must be positive"));
        }
        for material in materials(tx, id).await? {
            let required = material
                .reserved_quantity
                .checked_mul(quantity)
                .and_then(|v| v.checked_div(current.quantity_to_build))
                .ok_or_else(|| invalid("Material requirement overflow"))?;
            if required <= Decimal::ZERO {
                return Err(invalid("Material quantity is below decimal precision"));
            }
            sqlx::query("UPDATE manufacturing_work_order_materials SET reserved_quantity = $1 WHERE id = $2").bind(required).bind(material.id).execute(tx.as_mut()).await.map_err(map_db_error)?;
        }
    }
    sqlx::query("UPDATE manufacturing_work_orders SET work_center_id = COALESCE($1, work_center_id), assigned_to = COALESCE($2, assigned_to), priority = COALESCE($3, priority), scheduled_start = COALESCE($4, scheduled_start), scheduled_end = COALESCE($5, scheduled_end), notes = COALESCE($6, notes), quantity_to_build = COALESCE($7, quantity_to_build), updated_at = $8, version = version + 1 WHERE id = $9").bind(&input.work_center_id).bind(input.assigned_to).bind(input.priority.map(|v| v.to_string())).bind(input.scheduled_start).bind(input.scheduled_end).bind(&input.notes).bind(input.quantity_to_build).bind(Utc::now()).bind(id).execute(tx.as_mut()).await.map_err(map_db_error)?;
    Ok(())
}
