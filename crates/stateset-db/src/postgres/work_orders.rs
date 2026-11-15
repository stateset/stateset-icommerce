//! PostgreSQL Work Order repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, QueryBuilder};
use stateset_core::{
    AddWorkOrderMaterial, BatchResult, CommerceError, CreateWorkOrder, CreateWorkOrderTask, Result,
    TaskStatus, UpdateWorkOrder, UpdateWorkOrderTask, WorkOrder, WorkOrderFilter,
    WorkOrderMaterial, WorkOrderPriority, WorkOrderRepository, WorkOrderStatus, WorkOrderTask,
    validate_batch_size,
};
use uuid::Uuid;

/// PostgreSQL implementation of `WorkOrderRepository`
#[derive(Debug, Clone)]
pub struct PgWorkOrderRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct WorkOrderRow {
    source_location_id: Option<i32>,
    output_location_id: Option<i32>,
    output_sku: Option<String>,
    id: Uuid,
    work_order_number: String,
    product_id: Uuid,
    bom_id: Option<Uuid>,
    work_center_id: Option<String>,
    assigned_to: Option<Uuid>,
    status: String,
    priority: String,
    quantity_to_build: Decimal,
    quantity_completed: Decimal,
    scheduled_start: Option<DateTime<Utc>>,
    scheduled_end: Option<DateTime<Utc>>,
    actual_start: Option<DateTime<Utc>>,
    actual_end: Option<DateTime<Utc>>,
    notes: Option<String>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct WorkOrderTaskRow {
    id: Uuid,
    work_order_id: Uuid,
    sequence: i32,
    task_name: String,
    status: String,
    estimated_hours: Option<Decimal>,
    actual_hours: Option<Decimal>,
    assigned_to: Option<Uuid>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct WorkOrderMaterialRow {
    id: Uuid,
    work_order_id: Uuid,
    component_id: Option<Uuid>,
    component_sku: String,
    component_name: String,
    reserved_quantity: Decimal,
    consumed_quantity: Decimal,
    inventory_reservation_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgWorkOrderRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_work_order(
        row: WorkOrderRow,
        tasks: Vec<WorkOrderTask>,
        materials: Vec<WorkOrderMaterial>,
    ) -> Result<WorkOrder> {
        let WorkOrderRow {
            source_location_id,
            output_location_id,
            output_sku,
            id,
            work_order_number,
            product_id,
            bom_id,
            work_center_id,
            assigned_to,
            status,
            priority,
            quantity_to_build,
            quantity_completed,
            scheduled_start,
            scheduled_end,
            actual_start,
            actual_end,
            notes,
            version,
            created_at,
            updated_at,
        } = row;

        let status: WorkOrderStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid work_order.status '{}': {}", status, e))
        })?;
        let priority: WorkOrderPriority = priority.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid work_order.priority '{}': {}",
                priority, e
            ))
        })?;

        Ok(WorkOrder {
            stock: match (source_location_id, output_location_id, output_sku) {
                (None, None, None) => None,
                (Some(source_location_id), Some(output_location_id), Some(output_sku)) => {
                    Some(stateset_core::WorkOrderStock {
                        source_location_id,
                        output_location_id,
                        output_sku,
                    })
                }
                _ => {
                    return Err(CommerceError::DatabaseError(
                        "Incomplete work-order stock routing".into(),
                    ));
                }
            },
            id,
            work_order_number,
            product_id: product_id.into(),
            bom_id,
            work_center_id,
            assigned_to,
            status,
            priority,
            quantity_to_build,
            quantity_completed,
            scheduled_start,
            scheduled_end,
            actual_start,
            actual_end,
            notes,
            tasks,
            materials,
            version,
            created_at,
            updated_at,
        })
    }

    fn row_to_task(row: WorkOrderTaskRow) -> Result<WorkOrderTask> {
        let WorkOrderTaskRow {
            id,
            work_order_id,
            sequence,
            task_name,
            status,
            estimated_hours,
            actual_hours,
            assigned_to,
            started_at,
            completed_at,
            notes,
            created_at,
            updated_at,
        } = row;

        let status: TaskStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid work_order_task.status '{}': {}",
                status, e
            ))
        })?;

        Ok(WorkOrderTask {
            id,
            work_order_id,
            sequence,
            task_name,
            status,
            estimated_hours,
            actual_hours,
            assigned_to,
            started_at,
            completed_at,
            notes,
            created_at,
            updated_at,
        })
    }

    fn row_to_material(row: WorkOrderMaterialRow) -> WorkOrderMaterial {
        WorkOrderMaterial {
            id: row.id,
            work_order_id: row.work_order_id,
            component_id: row.component_id,
            component_sku: row.component_sku,
            component_name: row.component_name,
            reserved_quantity: row.reserved_quantity,
            consumed_quantity: row.consumed_quantity,
            inventory_reservation_id: row.inventory_reservation_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    async fn get_tasks_async(&self, work_order_id: Uuid) -> Result<Vec<WorkOrderTask>> {
        let rows = sqlx::query_as::<_, WorkOrderTaskRow>(
            "SELECT * FROM manufacturing_work_order_tasks WHERE work_order_id = $1 ORDER BY sequence",
        )
        .bind(work_order_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut tasks = Vec::with_capacity(rows.len());
        for row in rows {
            tasks.push(Self::row_to_task(row)?);
        }
        Ok(tasks)
    }

    async fn get_materials_async_internal(
        &self,
        work_order_id: Uuid,
    ) -> Result<Vec<WorkOrderMaterial>> {
        let rows = sqlx::query_as::<_, WorkOrderMaterialRow>(
            "SELECT * FROM manufacturing_work_order_materials WHERE work_order_id = $1",
        )
        .bind(work_order_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_material).collect())
    }

    async fn get_tasks_batch_async(
        &self,
        ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, Vec<WorkOrderTask>>> {
        let mut map: std::collections::HashMap<Uuid, Vec<WorkOrderTask>> =
            std::collections::HashMap::with_capacity(ids.len());
        if ids.is_empty() {
            return Ok(map);
        }
        let rows = sqlx::query_as::<_, WorkOrderTaskRow>(
            "SELECT * FROM manufacturing_work_order_tasks WHERE work_order_id = ANY($1) ORDER BY sequence",
        )
        .bind(ids.to_vec())
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        for row in rows {
            let parent = row.work_order_id;
            map.entry(parent).or_default().push(Self::row_to_task(row)?);
        }
        Ok(map)
    }

    async fn get_materials_batch_async(
        &self,
        ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, Vec<WorkOrderMaterial>>> {
        let mut map: std::collections::HashMap<Uuid, Vec<WorkOrderMaterial>> =
            std::collections::HashMap::with_capacity(ids.len());
        if ids.is_empty() {
            return Ok(map);
        }
        let rows = sqlx::query_as::<_, WorkOrderMaterialRow>(
            "SELECT * FROM manufacturing_work_order_materials WHERE work_order_id = ANY($1)",
        )
        .bind(ids.to_vec())
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        for row in rows {
            let parent = row.work_order_id;
            map.entry(parent).or_default().push(Self::row_to_material(row));
        }
        Ok(map)
    }

    async fn get_task_by_id(&self, task_id: Uuid) -> Result<WorkOrderTask> {
        let row = sqlx::query_as::<_, WorkOrderTaskRow>(
            "SELECT * FROM manufacturing_work_order_tasks WHERE id = $1",
        )
        .bind(task_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Self::row_to_task(row)
    }

    async fn get_material_by_id(&self, material_id: Uuid) -> Result<WorkOrderMaterial> {
        let row = sqlx::query_as::<_, WorkOrderMaterialRow>(
            "SELECT * FROM manufacturing_work_order_materials WHERE id = $1",
        )
        .bind(material_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(Self::row_to_material(row))
    }

    /// Create work order (async)
    pub async fn create_async(&self, input: CreateWorkOrder) -> Result<WorkOrder> {
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        super::work_order_stock::create(&mut tx, id, &input).await?;
        tx.commit().await.map_err(map_db_error)?;
        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Get work order by ID (async)
    pub async fn get_async(&self, id: Uuid) -> Result<Option<WorkOrder>> {
        let result = sqlx::query_as::<_, WorkOrderRow>(
            "SELECT * FROM manufacturing_work_orders WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match result {
            Some(row) => {
                let tasks = self.get_tasks_async(row.id).await?;
                let materials = self.get_materials_async_internal(row.id).await?;
                Ok(Some(Self::row_to_work_order(row, tasks, materials)?))
            }
            None => Ok(None),
        }
    }

    /// Get by number (async)
    pub async fn get_by_number_async(&self, work_order_number: &str) -> Result<Option<WorkOrder>> {
        let result = sqlx::query_as::<_, WorkOrderRow>(
            "SELECT * FROM manufacturing_work_orders WHERE work_order_number = $1",
        )
        .bind(work_order_number)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match result {
            Some(row) => {
                let tasks = self.get_tasks_async(row.id).await?;
                let materials = self.get_materials_async_internal(row.id).await?;
                Ok(Some(Self::row_to_work_order(row, tasks, materials)?))
            }
            None => Ok(None),
        }
    }

    /// Update work order (async)
    pub async fn update_async(&self, id: Uuid, input: UpdateWorkOrder) -> Result<WorkOrder> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        super::work_order_stock::update(&mut tx, id, &input).await?;
        tx.commit().await.map_err(map_db_error)?;
        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// List work orders (async)
    pub async fn list_async(&self, filter: WorkOrderFilter) -> Result<Vec<WorkOrder>> {
        let WorkOrderFilter {
            product_id,
            bom_id,
            status,
            priority,
            assigned_to,
            work_center_id,
            overdue_only,
            limit,
            offset,
            after_cursor,
        } = filter;
        let after_cursor = super::parse_after_cursor(after_cursor.as_ref())?;

        let mut builder = QueryBuilder::new("SELECT * FROM manufacturing_work_orders WHERE 1=1");

        if let Some(product_id) = product_id {
            builder.push(" AND product_id = ").push_bind(product_id);
        }
        if let Some(bom_id) = bom_id {
            builder.push(" AND bom_id = ").push_bind(bom_id);
        }
        if let Some(status) = status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(priority) = priority {
            builder.push(" AND priority = ").push_bind(priority.to_string());
        }
        if let Some(assigned_to) = assigned_to {
            builder.push(" AND assigned_to = ").push_bind(assigned_to);
        }
        if let Some(work_center_id) = work_center_id {
            builder.push(" AND work_center_id = ").push_bind(work_center_id);
        }
        if overdue_only.unwrap_or(false) {
            let now = Utc::now();
            builder
                .push(" AND scheduled_end IS NOT NULL AND scheduled_end < ")
                .push_bind(now)
                .push(" AND status NOT IN ('completed', 'cancelled')");
        }

        if let Some((cursor_created, cursor_id)) = after_cursor {
            // Keyset cursor: (created_at, id) for stable DESC ordering
            builder
                .push(" AND (created_at < ")
                .push_bind(cursor_created)
                .push(" OR (created_at = ")
                .push_bind(cursor_created)
                .push(" AND id < ")
                .push_bind(cursor_id)
                .push("))");
        }

        builder.push(" ORDER BY created_at DESC, id DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(limit));
        // Offset pagination applies only in non-cursor mode.
        let offset = if after_cursor.is_none() { offset } else { Some(0) };
        if let Some(offset) = offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<WorkOrderRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
        let mut tasks_by_id = self.get_tasks_batch_async(&ids).await?;
        let mut materials_by_id = self.get_materials_batch_async(&ids).await?;
        let mut work_orders = Vec::new();
        for row in rows {
            let tasks = tasks_by_id.remove(&row.id).unwrap_or_default();
            let materials = materials_by_id.remove(&row.id).unwrap_or_default();
            work_orders.push(Self::row_to_work_order(row, tasks, materials)?);
        }

        Ok(work_orders)
    }

    /// Delete work order (async) - cancels
    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        self.cancel_async(id).await.map(|_| ())
    }

    /// Start work order (async)
    pub async fn start_async(&self, id: Uuid) -> Result<WorkOrder> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let inventory = super::inventory::PgInventoryRepository::new(self.pool.clone());
        super::work_order_stock::transition(&mut tx, &inventory, id, WorkOrderStatus::InProgress)
            .await?;
        tx.commit().await.map_err(map_db_error)?;
        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Complete work order (async)
    pub async fn complete_async(&self, id: Uuid, quantity_completed: Decimal) -> Result<WorkOrder> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        super::work_order_stock::complete(&mut tx, id, quantity_completed).await?;
        tx.commit().await.map_err(map_db_error)?;
        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Hold work order (async)
    pub async fn hold_async(&self, id: Uuid) -> Result<WorkOrder> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let inventory = super::inventory::PgInventoryRepository::new(self.pool.clone());
        super::work_order_stock::transition(&mut tx, &inventory, id, WorkOrderStatus::OnHold)
            .await?;
        tx.commit().await.map_err(map_db_error)?;
        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Resume work order (async)
    pub async fn resume_async(&self, id: Uuid) -> Result<WorkOrder> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let inventory = super::inventory::PgInventoryRepository::new(self.pool.clone());
        super::work_order_stock::transition(&mut tx, &inventory, id, WorkOrderStatus::InProgress)
            .await?;
        tx.commit().await.map_err(map_db_error)?;
        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Cancel work order (async)
    pub async fn cancel_async(&self, id: Uuid) -> Result<WorkOrder> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let inventory = super::inventory::PgInventoryRepository::new(self.pool.clone());
        super::work_order_stock::transition(&mut tx, &inventory, id, WorkOrderStatus::Cancelled)
            .await?;
        tx.commit().await.map_err(map_db_error)?;
        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Add task (async)
    pub async fn add_task_async(
        &self,
        work_order_id: Uuid,
        task: CreateWorkOrderTask,
    ) -> Result<WorkOrderTask> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let sequence = task.sequence.unwrap_or(1);

        sqlx::query(
            r#"
            INSERT INTO manufacturing_work_order_tasks (id, work_order_id, sequence, task_name, status, estimated_hours, assigned_to, notes, created_at, updated_at)
            VALUES ($1, $2, $3, $4, 'pending', $5, $6, $7, $8, $9)
            "#,
        )
        .bind(id)
        .bind(work_order_id)
        .bind(sequence)
        .bind(&task.task_name)
        .bind(task.estimated_hours)
        .bind(task.assigned_to)
        .bind(&task.notes)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(WorkOrderTask {
            id,
            work_order_id,
            sequence,
            task_name: task.task_name,
            status: TaskStatus::Pending,
            estimated_hours: task.estimated_hours,
            actual_hours: None,
            assigned_to: task.assigned_to,
            started_at: None,
            completed_at: None,
            notes: task.notes,
            created_at: now,
            updated_at: now,
        })
    }

    /// Update task (async)
    pub async fn update_task_async(
        &self,
        task_id: Uuid,
        task: UpdateWorkOrderTask,
    ) -> Result<WorkOrderTask> {
        let existing = self.get_task_by_id(task_id).await?;
        let now = Utc::now();

        let new_sequence = task.sequence.unwrap_or(existing.sequence);
        let new_task_name = task.task_name.unwrap_or(existing.task_name);
        let new_status = task.status.unwrap_or(existing.status);
        let new_estimated = task.estimated_hours.or(existing.estimated_hours);
        let new_actual = task.actual_hours.or(existing.actual_hours);
        let new_assigned = task.assigned_to.or(existing.assigned_to);
        let new_started = task.started_at.or(existing.started_at);
        let new_completed = task.completed_at.or(existing.completed_at);
        let new_notes = task.notes.or(existing.notes);

        sqlx::query(
            "UPDATE manufacturing_work_order_tasks SET sequence = $1, task_name = $2, status = $3, estimated_hours = $4, actual_hours = $5, assigned_to = $6, started_at = $7, completed_at = $8, notes = $9, updated_at = $10 WHERE id = $11",
        )
        .bind(new_sequence)
        .bind(&new_task_name)
        .bind(new_status.to_string())
        .bind(new_estimated)
        .bind(new_actual)
        .bind(new_assigned)
        .bind(new_started)
        .bind(new_completed)
        .bind(&new_notes)
        .bind(now)
        .bind(task_id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_task_by_id(task_id).await
    }

    /// Remove task (async)
    pub async fn remove_task_async(&self, task_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM manufacturing_work_order_tasks WHERE id = $1")
            .bind(task_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    /// Start task (async)
    pub async fn start_task_async(&self, task_id: Uuid) -> Result<WorkOrderTask> {
        let now = Utc::now();

        sqlx::query("UPDATE manufacturing_work_order_tasks SET status = 'in_progress', started_at = $1, updated_at = $2 WHERE id = $3")
            .bind(now)
            .bind(now)
            .bind(task_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_task_by_id(task_id).await
    }

    /// Complete task (async)
    pub async fn complete_task_async(
        &self,
        task_id: Uuid,
        actual_hours: Option<Decimal>,
    ) -> Result<WorkOrderTask> {
        let now = Utc::now();

        sqlx::query("UPDATE manufacturing_work_order_tasks SET status = 'completed', actual_hours = $1, completed_at = $2, updated_at = $3 WHERE id = $4")
            .bind(actual_hours)
            .bind(now)
            .bind(now)
            .bind(task_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_task_by_id(task_id).await
    }

    /// Add material (async)
    pub async fn add_material_async(
        &self,
        work_order_id: Uuid,
        material: AddWorkOrderMaterial,
    ) -> Result<WorkOrderMaterial> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let material_id =
            super::work_order_stock::add_material(&mut tx, work_order_id, &material).await?;
        tx.commit().await.map_err(map_db_error)?;
        self.get_material_by_id(material_id).await
    }

    /// Consume material (async)
    pub async fn consume_material_async(
        &self,
        material_id: Uuid,
        quantity: Decimal,
    ) -> Result<WorkOrderMaterial> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        super::work_order_stock::consume_material(&mut tx, material_id, quantity).await?;
        tx.commit().await.map_err(map_db_error)?;
        self.get_material_by_id(material_id).await
    }

    /// Count work orders (async)
    pub async fn count_async(&self, filter: WorkOrderFilter) -> Result<u64> {
        let WorkOrderFilter {
            product_id,
            bom_id,
            status,
            priority,
            assigned_to,
            work_center_id,
            overdue_only,
            limit: _,
            offset: _,
            after_cursor: _,
        } = filter;

        let mut builder =
            QueryBuilder::new("SELECT COUNT(*) FROM manufacturing_work_orders WHERE 1=1");

        if let Some(product_id) = product_id {
            builder.push(" AND product_id = ").push_bind(product_id);
        }
        if let Some(bom_id) = bom_id {
            builder.push(" AND bom_id = ").push_bind(bom_id);
        }
        if let Some(status) = status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(priority) = priority {
            builder.push(" AND priority = ").push_bind(priority.to_string());
        }
        if let Some(assigned_to) = assigned_to {
            builder.push(" AND assigned_to = ").push_bind(assigned_to);
        }
        if let Some(work_center_id) = work_center_id {
            builder.push(" AND work_center_id = ").push_bind(work_center_id);
        }
        if overdue_only.unwrap_or(false) {
            let now = Utc::now();
            builder
                .push(" AND scheduled_end IS NOT NULL AND scheduled_end < ")
                .push_bind(now)
                .push(" AND status NOT IN ('completed', 'cancelled')");
        }

        let count: (i64,) =
            builder.build_query_as().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(count.0 as u64)
    }

    // === Batch Operations ===

    /// Create multiple work orders in a batch (async, non-atomic)
    pub async fn create_batch_async(
        &self,
        inputs: Vec<CreateWorkOrder>,
    ) -> Result<BatchResult<WorkOrder>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_async(input).await {
                Ok(work_order) => result.record_success(work_order),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    /// Create multiple work orders in a batch atomically (async)
    pub async fn create_batch_atomic_async(
        &self,
        inputs: Vec<CreateWorkOrder>,
    ) -> Result<Vec<WorkOrder>> {
        validate_batch_size(&inputs)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let mut ids_out = Vec::with_capacity(inputs.len());
        for input in inputs {
            let id = Uuid::new_v4();
            super::work_order_stock::create(&mut tx, id, &input).await?;
            ids_out.push(id);
        }
        tx.commit().await.map_err(map_db_error)?;
        let mut result = Vec::with_capacity(ids_out.len());
        for id in ids_out {
            result.push(self.get_async(id).await?.ok_or(CommerceError::NotFound)?);
        }
        Ok(result)
    }

    /// Update multiple work orders in a batch (async, non-atomic)
    pub async fn update_batch_async(
        &self,
        updates: Vec<(Uuid, UpdateWorkOrder)>,
    ) -> Result<BatchResult<WorkOrder>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update_async(id, input).await {
                Ok(work_order) => result.record_success(work_order),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Update multiple work orders in a batch atomically (async)
    pub async fn update_batch_atomic_async(
        &self,
        updates: Vec<(Uuid, UpdateWorkOrder)>,
    ) -> Result<Vec<WorkOrder>> {
        validate_batch_size(&updates)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let mut ids_out = Vec::with_capacity(updates.len());
        for (id, input) in updates {
            super::work_order_stock::update(&mut tx, id, &input).await?;
            ids_out.push(id);
        }
        tx.commit().await.map_err(map_db_error)?;
        let mut result = Vec::with_capacity(ids_out.len());
        for id in ids_out {
            result.push(self.get_async(id).await?.ok_or(CommerceError::NotFound)?);
        }
        Ok(result)
    }

    /// Delete multiple work orders in a batch (async, non-atomic)
    pub async fn delete_batch_async(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;
        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            match self.delete_async(id).await {
                Ok(()) => result.record_success(id),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Delete multiple work orders in a batch atomically (async)
    pub async fn delete_batch_atomic_async(&self, ids: Vec<Uuid>) -> Result<()> {
        validate_batch_size(&ids)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let inventory = super::inventory::PgInventoryRepository::new(self.pool.clone());
        let mut ids_out = Vec::with_capacity(ids.len());
        for id in ids {
            super::work_order_stock::transition(
                &mut tx,
                &inventory,
                id,
                WorkOrderStatus::Cancelled,
            )
            .await?;
            ids_out.push(id);
        }
        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Get multiple work orders by IDs (async)
    pub async fn get_batch_async(&self, ids: Vec<Uuid>) -> Result<Vec<WorkOrder>> {
        validate_batch_size(&ids)?;

        let rows = sqlx::query_as::<_, WorkOrderRow>(
            "SELECT * FROM manufacturing_work_orders WHERE id = ANY($1)",
        )
        .bind(&ids)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let row_ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
        let mut tasks_by_id = self.get_tasks_batch_async(&row_ids).await?;
        let mut materials_by_id = self.get_materials_batch_async(&row_ids).await?;
        let mut work_orders = Vec::with_capacity(rows.len());
        for row in rows {
            let tasks = tasks_by_id.remove(&row.id).unwrap_or_default();
            let materials = materials_by_id.remove(&row.id).unwrap_or_default();
            work_orders.push(Self::row_to_work_order(row, tasks, materials)?);
        }

        Ok(work_orders)
    }
}

impl WorkOrderRepository for PgWorkOrderRepository {
    fn create(&self, input: CreateWorkOrder) -> Result<WorkOrder> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<WorkOrder>> {
        super::block_on(self.get_async(id))
    }

    fn get_by_number(&self, work_order_number: &str) -> Result<Option<WorkOrder>> {
        super::block_on(self.get_by_number_async(work_order_number))
    }

    fn update(&self, id: Uuid, input: UpdateWorkOrder) -> Result<WorkOrder> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: WorkOrderFilter) -> Result<Vec<WorkOrder>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        super::block_on(self.delete_async(id))
    }

    fn start(&self, id: Uuid) -> Result<WorkOrder> {
        super::block_on(self.start_async(id))
    }

    fn complete(&self, id: Uuid, quantity_completed: Decimal) -> Result<WorkOrder> {
        super::block_on(self.complete_async(id, quantity_completed))
    }

    fn hold(&self, id: Uuid) -> Result<WorkOrder> {
        super::block_on(self.hold_async(id))
    }

    fn resume(&self, id: Uuid) -> Result<WorkOrder> {
        super::block_on(self.resume_async(id))
    }

    fn cancel(&self, id: Uuid) -> Result<WorkOrder> {
        super::block_on(self.cancel_async(id))
    }

    fn add_task(&self, work_order_id: Uuid, task: CreateWorkOrderTask) -> Result<WorkOrderTask> {
        super::block_on(self.add_task_async(work_order_id, task))
    }

    fn update_task(&self, task_id: Uuid, task: UpdateWorkOrderTask) -> Result<WorkOrderTask> {
        super::block_on(self.update_task_async(task_id, task))
    }

    fn remove_task(&self, task_id: Uuid) -> Result<()> {
        super::block_on(self.remove_task_async(task_id))
    }

    fn get_tasks(&self, work_order_id: Uuid) -> Result<Vec<WorkOrderTask>> {
        super::block_on(self.get_tasks_async(work_order_id))
    }

    fn start_task(&self, task_id: Uuid) -> Result<WorkOrderTask> {
        super::block_on(self.start_task_async(task_id))
    }

    fn complete_task(&self, task_id: Uuid, actual_hours: Option<Decimal>) -> Result<WorkOrderTask> {
        super::block_on(self.complete_task_async(task_id, actual_hours))
    }

    fn add_material(
        &self,
        work_order_id: Uuid,
        material: AddWorkOrderMaterial,
    ) -> Result<WorkOrderMaterial> {
        super::block_on(self.add_material_async(work_order_id, material))
    }

    fn consume_material(&self, material_id: Uuid, quantity: Decimal) -> Result<WorkOrderMaterial> {
        super::block_on(self.consume_material_async(material_id, quantity))
    }

    fn get_materials(&self, work_order_id: Uuid) -> Result<Vec<WorkOrderMaterial>> {
        super::block_on(self.get_materials_async_internal(work_order_id))
    }

    fn count(&self, filter: WorkOrderFilter) -> Result<u64> {
        super::block_on(self.count_async(filter))
    }

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreateWorkOrder>) -> Result<BatchResult<WorkOrder>> {
        super::block_on(self.create_batch_async(inputs))
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateWorkOrder>) -> Result<Vec<WorkOrder>> {
        super::block_on(self.create_batch_atomic_async(inputs))
    }

    fn update_batch(
        &self,
        updates: Vec<(Uuid, UpdateWorkOrder)>,
    ) -> Result<BatchResult<WorkOrder>> {
        super::block_on(self.update_batch_async(updates))
    }

    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateWorkOrder)>) -> Result<Vec<WorkOrder>> {
        super::block_on(self.update_batch_atomic_async(updates))
    }

    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        super::block_on(self.delete_batch_async(ids))
    }

    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()> {
        super::block_on(self.delete_batch_atomic_async(ids))
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<WorkOrder>> {
        super::block_on(self.get_batch_async(ids))
    }
}
