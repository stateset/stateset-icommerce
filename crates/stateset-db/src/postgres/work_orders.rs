//! PostgreSQL Work Order repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use stateset_core::{
    AddWorkOrderMaterial, CommerceError, CreateWorkOrder, CreateWorkOrderTask, Result,
    TaskStatus, UpdateWorkOrder, UpdateWorkOrderTask, WorkOrder, WorkOrderFilter,
    WorkOrderMaterial, WorkOrderPriority, WorkOrderRepository, WorkOrderStatus, WorkOrderTask,
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

    fn parse_status(s: &str) -> WorkOrderStatus {
        match s {
            "in_progress" => WorkOrderStatus::InProgress,
            "completed" => WorkOrderStatus::Completed,
            "partially_completed" => WorkOrderStatus::PartiallyCompleted,
            "cancelled" => WorkOrderStatus::Cancelled,
            "on_hold" => WorkOrderStatus::OnHold,
            _ => WorkOrderStatus::Planned,
        }
    }

    fn parse_priority(s: &str) -> WorkOrderPriority {
        match s {
            "low" => WorkOrderPriority::Low,
            "high" => WorkOrderPriority::High,
            "urgent" => WorkOrderPriority::Urgent,
            _ => WorkOrderPriority::Normal,
        }
    }

    fn parse_task_status(s: &str) -> TaskStatus {
        match s {
            "in_progress" => TaskStatus::InProgress,
            "completed" => TaskStatus::Completed,
            "skipped" => TaskStatus::Skipped,
            "cancelled" => TaskStatus::Cancelled,
            _ => TaskStatus::Pending,
        }
    }

    fn row_to_work_order(row: WorkOrderRow, tasks: Vec<WorkOrderTask>, materials: Vec<WorkOrderMaterial>) -> WorkOrder {
        WorkOrder {
            id: row.id,
            work_order_number: row.work_order_number,
            product_id: row.product_id,
            bom_id: row.bom_id,
            work_center_id: row.work_center_id,
            assigned_to: row.assigned_to,
            status: Self::parse_status(&row.status),
            priority: Self::parse_priority(&row.priority),
            quantity_to_build: row.quantity_to_build,
            quantity_completed: row.quantity_completed,
            scheduled_start: row.scheduled_start,
            scheduled_end: row.scheduled_end,
            actual_start: row.actual_start,
            actual_end: row.actual_end,
            notes: row.notes,
            tasks,
            materials,
            version: row.version,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_task(row: WorkOrderTaskRow) -> WorkOrderTask {
        WorkOrderTask {
            id: row.id,
            work_order_id: row.work_order_id,
            sequence: row.sequence,
            task_name: row.task_name,
            status: Self::parse_task_status(&row.status),
            estimated_hours: row.estimated_hours,
            actual_hours: row.actual_hours,
            assigned_to: row.assigned_to,
            started_at: row.started_at,
            completed_at: row.completed_at,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
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

        Ok(rows.into_iter().map(Self::row_to_task).collect())
    }

    async fn get_materials_async_internal(&self, work_order_id: Uuid) -> Result<Vec<WorkOrderMaterial>> {
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

        Ok(Self::row_to_task(row))
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
                Ok(Some(Self::row_to_work_order(row, tasks, materials)))
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
                Ok(Some(Self::row_to_work_order(row, tasks, materials)))
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
        let limit = filter.limit.unwrap_or(100) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;

        let rows = sqlx::query_as::<_, WorkOrderRow>(
            "SELECT * FROM manufacturing_work_orders ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut work_orders = Vec::new();
        for row in rows {
            let tasks = self.get_tasks_async(row.id).await?;
            let materials = self.get_materials_async_internal(row.id).await?;
            work_orders.push(Self::row_to_work_order(row, tasks, materials));
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
        let existing = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let new_quantity = existing.quantity_completed + quantity_completed;
        let status = if new_quantity >= existing.quantity_to_build {
            "completed"
        } else {
            "partially_completed"
        };

        sqlx::query("UPDATE manufacturing_work_orders SET status = $1, quantity_completed = $2, actual_end = $3, updated_at = $4 WHERE id = $5")
            .bind(status)
            .bind(new_quantity)
            .bind(now)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Hold work order (async)
    pub async fn hold_async(&self, id: Uuid) -> Result<WorkOrder> {
        self.update_async(id, UpdateWorkOrder {
            status: Some(WorkOrderStatus::OnHold),
            ..Default::default()
        }).await
    }

    /// Resume work order (async)
    pub async fn resume_async(&self, id: Uuid) -> Result<WorkOrder> {
        self.update_async(id, UpdateWorkOrder {
            status: Some(WorkOrderStatus::InProgress),
            ..Default::default()
        }).await
    }

    /// Cancel work order (async)
    pub async fn cancel_async(&self, id: Uuid) -> Result<WorkOrder> {
        self.update_async(id, UpdateWorkOrder {
            status: Some(WorkOrderStatus::Cancelled),
            ..Default::default()
        }).await
    }

    /// Add task (async)
    pub async fn add_task_async(&self, work_order_id: Uuid, task: CreateWorkOrderTask) -> Result<WorkOrderTask> {
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
    pub async fn update_task_async(&self, task_id: Uuid, task: UpdateWorkOrderTask) -> Result<WorkOrderTask> {
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
    pub async fn complete_task_async(&self, task_id: Uuid, actual_hours: Option<Decimal>) -> Result<WorkOrderTask> {
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
    pub async fn add_material_async(&self, work_order_id: Uuid, material: AddWorkOrderMaterial) -> Result<WorkOrderMaterial> {
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
    pub async fn consume_material_async(&self, material_id: Uuid, quantity: Decimal) -> Result<WorkOrderMaterial> {
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
    pub async fn count_async(&self, _filter: WorkOrderFilter) -> Result<u64> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM manufacturing_work_orders")
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(count.0 as u64)
    }
}

impl WorkOrderRepository for PgWorkOrderRepository {
    fn create(&self, input: CreateWorkOrder) -> Result<WorkOrder> {
        tokio::runtime::Handle::current().block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<WorkOrder>> {
        tokio::runtime::Handle::current().block_on(self.get_async(id))
    }

    fn get_by_number(&self, work_order_number: &str) -> Result<Option<WorkOrder>> {
        tokio::runtime::Handle::current().block_on(self.get_by_number_async(work_order_number))
    }

    fn update(&self, id: Uuid, input: UpdateWorkOrder) -> Result<WorkOrder> {
        tokio::runtime::Handle::current().block_on(self.update_async(id, input))
    }

    fn list(&self, filter: WorkOrderFilter) -> Result<Vec<WorkOrder>> {
        tokio::runtime::Handle::current().block_on(self.list_async(filter))
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.delete_async(id))
    }

    fn start(&self, id: Uuid) -> Result<WorkOrder> {
        tokio::runtime::Handle::current().block_on(self.start_async(id))
    }

    fn complete(&self, id: Uuid, quantity_completed: Decimal) -> Result<WorkOrder> {
        tokio::runtime::Handle::current().block_on(self.complete_async(id, quantity_completed))
    }

    fn hold(&self, id: Uuid) -> Result<WorkOrder> {
        tokio::runtime::Handle::current().block_on(self.hold_async(id))
    }

    fn resume(&self, id: Uuid) -> Result<WorkOrder> {
        tokio::runtime::Handle::current().block_on(self.resume_async(id))
    }

    fn cancel(&self, id: Uuid) -> Result<WorkOrder> {
        tokio::runtime::Handle::current().block_on(self.cancel_async(id))
    }

    fn add_task(&self, work_order_id: Uuid, task: CreateWorkOrderTask) -> Result<WorkOrderTask> {
        tokio::runtime::Handle::current().block_on(self.add_task_async(work_order_id, task))
    }

    fn update_task(&self, task_id: Uuid, task: UpdateWorkOrderTask) -> Result<WorkOrderTask> {
        tokio::runtime::Handle::current().block_on(self.update_task_async(task_id, task))
    }

    fn remove_task(&self, task_id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.remove_task_async(task_id))
    }

    fn get_tasks(&self, work_order_id: Uuid) -> Result<Vec<WorkOrderTask>> {
        tokio::runtime::Handle::current().block_on(self.get_tasks_async(work_order_id))
    }

    fn start_task(&self, task_id: Uuid) -> Result<WorkOrderTask> {
        tokio::runtime::Handle::current().block_on(self.start_task_async(task_id))
    }

    fn complete_task(&self, task_id: Uuid, actual_hours: Option<Decimal>) -> Result<WorkOrderTask> {
        tokio::runtime::Handle::current().block_on(self.complete_task_async(task_id, actual_hours))
    }

    fn add_material(&self, work_order_id: Uuid, material: AddWorkOrderMaterial) -> Result<WorkOrderMaterial> {
        tokio::runtime::Handle::current().block_on(self.add_material_async(work_order_id, material))
    }

    fn consume_material(&self, material_id: Uuid, quantity: Decimal) -> Result<WorkOrderMaterial> {
        tokio::runtime::Handle::current().block_on(self.consume_material_async(material_id, quantity))
    }

    fn get_materials(&self, work_order_id: Uuid) -> Result<Vec<WorkOrderMaterial>> {
        tokio::runtime::Handle::current().block_on(self.get_materials_async_internal(work_order_id))
    }

    fn count(&self, filter: WorkOrderFilter) -> Result<u64> {
        tokio::runtime::Handle::current().block_on(self.count_async(filter))
    }
}
