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

/// PostgreSQL implementation of WorkOrderRepository
#[derive(Clone)]
pub struct PgWorkOrderRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct WorkOrderRow {
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
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_work_order(
        row: WorkOrderRow,
        tasks: Vec<WorkOrderTask>,
        materials: Vec<WorkOrderMaterial>,
    ) -> Result<WorkOrder> {
        let WorkOrderRow {
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
        let work_order_number = WorkOrder::generate_work_order_number();
        let now = Utc::now();
        let priority = input.priority.unwrap_or(WorkOrderPriority::Normal);

        sqlx::query(
            r#"
            INSERT INTO manufacturing_work_orders (id, work_order_number, product_id, bom_id, work_center_id, assigned_to, status, priority, quantity_to_build, quantity_completed, scheduled_start, scheduled_end, notes, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, 'planned', $7, $8, 0, $9, $10, $11, $12, $13)
            "#,
        )
        .bind(id)
        .bind(&work_order_number)
        .bind(input.product_id)
        .bind(input.bom_id)
        .bind(&input.work_center_id)
        .bind(input.assigned_to)
        .bind(priority.to_string())
        .bind(input.quantity_to_build)
        .bind(input.scheduled_start)
        .bind(input.scheduled_end)
        .bind(&input.notes)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        // Create tasks if provided
        let mut tasks = Vec::new();
        if let Some(task_inputs) = input.tasks {
            for task_input in task_inputs {
                let task = self.add_task_async(id, task_input).await?;
                tasks.push(task);
            }
        }

        Ok(WorkOrder {
            id,
            work_order_number,
            product_id: input.product_id,
            bom_id: input.bom_id,
            work_center_id: input.work_center_id,
            assigned_to: input.assigned_to,
            status: WorkOrderStatus::Planned,
            priority,
            quantity_to_build: input.quantity_to_build,
            quantity_completed: Decimal::ZERO,
            scheduled_start: input.scheduled_start,
            scheduled_end: input.scheduled_end,
            actual_start: None,
            actual_end: None,
            notes: input.notes,
            tasks,
            materials: vec![],
            version: 1,
            created_at: now,
            updated_at: now,
        })
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
        let existing = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let new_status = input.status.unwrap_or(existing.status);
        let new_priority = input.priority.unwrap_or(existing.priority);
        let new_assigned_to = input.assigned_to.or(existing.assigned_to);
        let new_notes = input.notes.or(existing.notes);
        let new_work_center_id = input.work_center_id.or(existing.work_center_id);

        sqlx::query(
            "UPDATE manufacturing_work_orders SET status = $1, priority = $2, assigned_to = $3, work_center_id = $4, scheduled_start = $5, scheduled_end = $6, notes = $7, updated_at = $8 WHERE id = $9",
        )
        .bind(new_status.to_string())
        .bind(new_priority.to_string())
        .bind(new_assigned_to)
        .bind(&new_work_center_id)
        .bind(input.scheduled_start.or(existing.scheduled_start))
        .bind(input.scheduled_end.or(existing.scheduled_end))
        .bind(&new_notes)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

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
        } = filter;

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

        builder.push(" ORDER BY created_at DESC");

        if let Some(limit) = limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<WorkOrderRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        let mut work_orders = Vec::new();
        for row in rows {
            let tasks = self.get_tasks_async(row.id).await?;
            let materials = self.get_materials_async_internal(row.id).await?;
            work_orders.push(Self::row_to_work_order(row, tasks, materials)?);
        }

        Ok(work_orders)
    }

    /// Delete work order (async) - cancels
    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        sqlx::query("UPDATE manufacturing_work_orders SET status = 'cancelled', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    /// Start work order (async)
    pub async fn start_async(&self, id: Uuid) -> Result<WorkOrder> {
        let now = Utc::now();

        sqlx::query("UPDATE manufacturing_work_orders SET status = 'in_progress', actual_start = $1, updated_at = $2 WHERE id = $3")
            .bind(now)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Complete work order (async)
    pub async fn complete_async(&self, id: Uuid, quantity_completed: Decimal) -> Result<WorkOrder> {
        if quantity_completed <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Completed quantity must be greater than zero".to_string(),
            ));
        }

        let now = Utc::now();
        let result = sqlx::query(
            "UPDATE manufacturing_work_orders
             SET quantity_completed = quantity_completed + $1,
                 status = CASE
                    WHEN quantity_completed + $1 >= quantity_to_build THEN 'completed'
                    ELSE 'partially_completed'
                 END,
                 actual_end = CASE
                    WHEN quantity_completed + $1 >= quantity_to_build THEN $2
                    ELSE actual_end
                 END,
                 updated_at = $3
             WHERE id = $4",
        )
        .bind(quantity_completed)
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        if result.rows_affected() == 0 {
            return Err(CommerceError::NotFound);
        }

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Hold work order (async)
    pub async fn hold_async(&self, id: Uuid) -> Result<WorkOrder> {
        self.update_async(
            id,
            UpdateWorkOrder { status: Some(WorkOrderStatus::OnHold), ..Default::default() },
        )
        .await
    }

    /// Resume work order (async)
    pub async fn resume_async(&self, id: Uuid) -> Result<WorkOrder> {
        self.update_async(
            id,
            UpdateWorkOrder { status: Some(WorkOrderStatus::InProgress), ..Default::default() },
        )
        .await
    }

    /// Cancel work order (async)
    pub async fn cancel_async(&self, id: Uuid) -> Result<WorkOrder> {
        self.update_async(
            id,
            UpdateWorkOrder { status: Some(WorkOrderStatus::Cancelled), ..Default::default() },
        )
        .await
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
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO manufacturing_work_order_materials (id, work_order_id, component_id, component_sku, component_name, reserved_quantity, consumed_quantity, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, 0, $7, $8)
            "#,
        )
        .bind(id)
        .bind(work_order_id)
        .bind(material.component_id)
        .bind(&material.component_sku)
        .bind(&material.component_name)
        .bind(material.quantity)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(WorkOrderMaterial {
            id,
            work_order_id,
            component_id: material.component_id,
            component_sku: material.component_sku,
            component_name: material.component_name,
            reserved_quantity: material.quantity,
            consumed_quantity: Decimal::ZERO,
            inventory_reservation_id: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Consume material (async)
    pub async fn consume_material_async(
        &self,
        material_id: Uuid,
        quantity: Decimal,
    ) -> Result<WorkOrderMaterial> {
        let existing = self.get_material_by_id(material_id).await?;
        let now = Utc::now();
        let new_consumed = existing.consumed_quantity + quantity;

        sqlx::query("UPDATE manufacturing_work_order_materials SET consumed_quantity = $1, updated_at = $2 WHERE id = $3")
            .bind(new_consumed)
            .bind(now)
            .bind(material_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

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
        let mut work_orders = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = Uuid::new_v4();
            let work_order_number = WorkOrder::generate_work_order_number();
            let now = Utc::now();
            let priority = input.priority.unwrap_or(WorkOrderPriority::Normal);

            sqlx::query(
                r#"
                INSERT INTO manufacturing_work_orders (id, work_order_number, product_id, bom_id, work_center_id, assigned_to, status, priority, quantity_to_build, quantity_completed, scheduled_start, scheduled_end, notes, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, 'planned', $7, $8, 0, $9, $10, $11, $12, $13)
                "#,
            )
            .bind(id)
            .bind(&work_order_number)
            .bind(input.product_id)
            .bind(input.bom_id)
            .bind(&input.work_center_id)
            .bind(input.assigned_to)
            .bind(priority.to_string())
            .bind(input.quantity_to_build)
            .bind(input.scheduled_start)
            .bind(input.scheduled_end)
            .bind(&input.notes)
            .bind(now)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            // Create tasks if provided
            let mut tasks = Vec::new();
            if let Some(task_inputs) = input.tasks {
                for task_input in task_inputs {
                    let task_id = Uuid::new_v4();
                    let sequence = task_input.sequence.unwrap_or(1);

                    sqlx::query(
                        r#"
                        INSERT INTO manufacturing_work_order_tasks (id, work_order_id, sequence, task_name, status, estimated_hours, assigned_to, notes, created_at, updated_at)
                        VALUES ($1, $2, $3, $4, 'pending', $5, $6, $7, $8, $9)
                        "#,
                    )
                    .bind(task_id)
                    .bind(id)
                    .bind(sequence)
                    .bind(&task_input.task_name)
                    .bind(task_input.estimated_hours)
                    .bind(task_input.assigned_to)
                    .bind(&task_input.notes)
                    .bind(now)
                    .bind(now)
                    .execute(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;

                    tasks.push(WorkOrderTask {
                        id: task_id,
                        work_order_id: id,
                        sequence,
                        task_name: task_input.task_name,
                        status: TaskStatus::Pending,
                        estimated_hours: task_input.estimated_hours,
                        actual_hours: None,
                        assigned_to: task_input.assigned_to,
                        started_at: None,
                        completed_at: None,
                        notes: task_input.notes,
                        created_at: now,
                        updated_at: now,
                    });
                }
            }

            work_orders.push(WorkOrder {
                id,
                work_order_number,
                product_id: input.product_id,
                bom_id: input.bom_id,
                work_center_id: input.work_center_id,
                assigned_to: input.assigned_to,
                status: WorkOrderStatus::Planned,
                priority,
                quantity_to_build: input.quantity_to_build,
                quantity_completed: Decimal::ZERO,
                scheduled_start: input.scheduled_start,
                scheduled_end: input.scheduled_end,
                actual_start: None,
                actual_end: None,
                notes: input.notes,
                tasks,
                materials: vec![],
                version: 1,
                created_at: now,
                updated_at: now,
            });
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(work_orders)
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
        let mut work_orders = Vec::with_capacity(updates.len());
        let now = Utc::now();

        for (id, input) in updates {
            // Get existing work order with lock
            let existing_row = sqlx::query_as::<_, WorkOrderRow>(
                "SELECT * FROM manufacturing_work_orders WHERE id = $1 FOR UPDATE",
            )
            .bind(id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;

            let status_str = existing_row.status.clone();
            let priority_str = existing_row.priority.clone();
            let existing_status: WorkOrderStatus = status_str.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid work_order.status '{}': {}",
                    status_str, e
                ))
            })?;
            let existing_priority: WorkOrderPriority = priority_str.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid work_order.priority '{}': {}",
                    priority_str, e
                ))
            })?;
            let new_status = input.status.unwrap_or(existing_status);
            let new_priority = input.priority.unwrap_or(existing_priority);
            let new_assigned_to = input.assigned_to.or(existing_row.assigned_to);
            let new_notes = input.notes.or(existing_row.notes.clone());
            let new_work_center_id = input.work_center_id.or(existing_row.work_center_id.clone());

            sqlx::query(
                "UPDATE manufacturing_work_orders SET status = $1, priority = $2, assigned_to = $3, work_center_id = $4, scheduled_start = $5, scheduled_end = $6, notes = $7, updated_at = $8 WHERE id = $9",
            )
            .bind(new_status.to_string())
            .bind(new_priority.to_string())
            .bind(new_assigned_to)
            .bind(&new_work_center_id)
            .bind(input.scheduled_start.or(existing_row.scheduled_start))
            .bind(input.scheduled_end.or(existing_row.scheduled_end))
            .bind(&new_notes)
            .bind(now)
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            // Fetch updated work order
            let updated_row = sqlx::query_as::<_, WorkOrderRow>(
                "SELECT * FROM manufacturing_work_orders WHERE id = $1",
            )
            .bind(id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            let tasks = sqlx::query_as::<_, WorkOrderTaskRow>(
                "SELECT * FROM manufacturing_work_order_tasks WHERE work_order_id = $1 ORDER BY sequence",
            )
            .bind(id)
            .fetch_all(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            let materials = sqlx::query_as::<_, WorkOrderMaterialRow>(
                "SELECT * FROM manufacturing_work_order_materials WHERE work_order_id = $1",
            )
            .bind(id)
            .fetch_all(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            let mut task_models = Vec::with_capacity(tasks.len());
            for task in tasks {
                task_models.push(Self::row_to_task(task)?);
            }
            let material_models = materials.into_iter().map(Self::row_to_material).collect();
            work_orders.push(Self::row_to_work_order(updated_row, task_models, material_models)?);
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(work_orders)
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
        if ids.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        // Match single-delete semantics: soft-cancel work orders atomically.
        sqlx::query(
            "UPDATE manufacturing_work_orders
             SET status = 'cancelled', updated_at = $1
             WHERE id = ANY($2)",
        )
            .bind(now)
            .bind(&ids)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

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

        let mut work_orders = Vec::with_capacity(rows.len());
        for row in rows {
            let tasks = self.get_tasks_async(row.id).await?;
            let materials = self.get_materials_async_internal(row.id).await?;
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
