//! SQLite Work Order repository implementation

use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use std::str::FromStr;
use stateset_core::{
    AddWorkOrderMaterial, CommerceError, CreateWorkOrder, CreateWorkOrderTask, Result,
    TaskStatus, UpdateWorkOrder, UpdateWorkOrderTask, WorkOrder, WorkOrderFilter, WorkOrderMaterial,
    WorkOrderPriority, WorkOrderRepository, WorkOrderStatus, WorkOrderTask,
};
use uuid::Uuid;

/// SQLite implementation of WorkOrderRepository
pub struct SqliteWorkOrderRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteWorkOrderRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
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

    fn parse_datetime(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now())
    }

    fn parse_optional_datetime(s: Option<String>) -> Option<DateTime<Utc>> {
        s.map(|s| Self::parse_datetime(&s))
    }

    fn load_tasks(&self, work_order_id: Uuid) -> Result<Vec<WorkOrderTask>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, work_order_id, sequence, task_name, status, estimated_hours,
                        actual_hours, assigned_to, started_at, completed_at, notes, created_at, updated_at
                 FROM manufacturing_work_order_tasks WHERE work_order_id = ? ORDER BY sequence",
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([work_order_id.to_string()], |row| {
                Ok(WorkOrderTask {
                    id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                    work_order_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or_default(),
                    sequence: row.get(2)?,
                    task_name: row.get(3)?,
                    status: Self::parse_task_status(&row.get::<_, String>(4)?),
                    estimated_hours: row
                        .get::<_, Option<String>>(5)?
                        .and_then(|s| Decimal::from_str(&s).ok()),
                    actual_hours: row
                        .get::<_, Option<String>>(6)?
                        .and_then(|s| Decimal::from_str(&s).ok()),
                    assigned_to: row
                        .get::<_, Option<String>>(7)?
                        .and_then(|s| Uuid::parse_str(&s).ok()),
                    started_at: Self::parse_optional_datetime(row.get(8)?),
                    completed_at: Self::parse_optional_datetime(row.get(9)?),
                    notes: row.get(10)?,
                    created_at: Self::parse_datetime(&row.get::<_, String>(11)?),
                    updated_at: Self::parse_datetime(&row.get::<_, String>(12)?),
                })
            })
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row.map_err(|e| CommerceError::DatabaseError(e.to_string()))?);
        }

        Ok(tasks)
    }

    fn load_materials(&self, work_order_id: Uuid) -> Result<Vec<WorkOrderMaterial>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, work_order_id, component_id, component_sku, component_name,
                        reserved_quantity, consumed_quantity, inventory_reservation_id, created_at, updated_at
                 FROM manufacturing_work_order_materials WHERE work_order_id = ?",
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([work_order_id.to_string()], |row| {
                Ok(WorkOrderMaterial {
                    id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                    work_order_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or_default(),
                    component_id: row
                        .get::<_, Option<String>>(2)?
                        .and_then(|s| Uuid::parse_str(&s).ok()),
                    component_sku: row.get(3)?,
                    component_name: row.get(4)?,
                    reserved_quantity: Decimal::from_str(&row.get::<_, String>(5)?)
                        .unwrap_or_default(),
                    consumed_quantity: Decimal::from_str(&row.get::<_, String>(6)?)
                        .unwrap_or_default(),
                    inventory_reservation_id: row
                        .get::<_, Option<String>>(7)?
                        .and_then(|s| Uuid::parse_str(&s).ok()),
                    created_at: Self::parse_datetime(&row.get::<_, String>(8)?),
                    updated_at: Self::parse_datetime(&row.get::<_, String>(9)?),
                })
            })
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut materials = Vec::new();
        for row in rows {
            materials.push(row.map_err(|e| CommerceError::DatabaseError(e.to_string()))?);
        }

        Ok(materials)
    }

    fn get_task_internal(&self, task_id: Uuid) -> Result<WorkOrderTask> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.query_row(
            "SELECT id, work_order_id, sequence, task_name, status, estimated_hours,
                    actual_hours, assigned_to, started_at, completed_at, notes, created_at, updated_at
             FROM manufacturing_work_order_tasks WHERE id = ?",
            [task_id.to_string()],
            |row| {
                Ok(WorkOrderTask {
                    id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                    work_order_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or_default(),
                    sequence: row.get(2)?,
                    task_name: row.get(3)?,
                    status: Self::parse_task_status(&row.get::<_, String>(4)?),
                    estimated_hours: row
                        .get::<_, Option<String>>(5)?
                        .and_then(|s| Decimal::from_str(&s).ok()),
                    actual_hours: row
                        .get::<_, Option<String>>(6)?
                        .and_then(|s| Decimal::from_str(&s).ok()),
                    assigned_to: row
                        .get::<_, Option<String>>(7)?
                        .and_then(|s| Uuid::parse_str(&s).ok()),
                    started_at: Self::parse_optional_datetime(row.get(8)?),
                    completed_at: Self::parse_optional_datetime(row.get(9)?),
                    notes: row.get(10)?,
                    created_at: Self::parse_datetime(&row.get::<_, String>(11)?),
                    updated_at: Self::parse_datetime(&row.get::<_, String>(12)?),
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => CommerceError::NotFound,
            _ => CommerceError::DatabaseError(e.to_string()),
        })
    }

    fn get_material_internal(&self, material_id: Uuid) -> Result<WorkOrderMaterial> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.query_row(
            "SELECT id, work_order_id, component_id, component_sku, component_name,
                    reserved_quantity, consumed_quantity, inventory_reservation_id, created_at, updated_at
             FROM manufacturing_work_order_materials WHERE id = ?",
            [material_id.to_string()],
            |row| {
                Ok(WorkOrderMaterial {
                    id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                    work_order_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or_default(),
                    component_id: row
                        .get::<_, Option<String>>(2)?
                        .and_then(|s| Uuid::parse_str(&s).ok()),
                    component_sku: row.get(3)?,
                    component_name: row.get(4)?,
                    reserved_quantity: Decimal::from_str(&row.get::<_, String>(5)?)
                        .unwrap_or_default(),
                    consumed_quantity: Decimal::from_str(&row.get::<_, String>(6)?)
                        .unwrap_or_default(),
                    inventory_reservation_id: row
                        .get::<_, Option<String>>(7)?
                        .and_then(|s| Uuid::parse_str(&s).ok()),
                    created_at: Self::parse_datetime(&row.get::<_, String>(8)?),
                    updated_at: Self::parse_datetime(&row.get::<_, String>(9)?),
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => CommerceError::NotFound,
            _ => CommerceError::DatabaseError(e.to_string()),
        })
    }
}

impl WorkOrderRepository for SqliteWorkOrderRepository {
    fn create(&self, input: CreateWorkOrder) -> Result<WorkOrder> {
        let id = Uuid::new_v4();
        let work_order_number = WorkOrder::generate_work_order_number();
        let now = Utc::now();
        let priority = input.priority.clone().unwrap_or(WorkOrderPriority::Normal);

        // Insert work order in a scoped block to release connection before adding tasks
        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            conn.execute(
                "INSERT INTO manufacturing_work_orders (id, work_order_number, product_id, bom_id, work_center_id,
                 assigned_to, status, priority, quantity_to_build, quantity_completed, scheduled_start, scheduled_end, notes, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, 'planned', ?, ?, '0', ?, ?, ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    work_order_number,
                    input.product_id.to_string(),
                    input.bom_id.map(|u| u.to_string()),
                    input.work_center_id,
                    input.assigned_to.map(|u| u.to_string()),
                    priority.to_string(),
                    input.quantity_to_build.to_string(),
                    input.scheduled_start.map(|dt| dt.to_rfc3339()),
                    input.scheduled_end.map(|dt| dt.to_rfc3339()),
                    input.notes,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        } // Connection released here

        // Create tasks if provided (after releasing the main connection)
        let mut tasks = Vec::new();
        if let Some(task_inputs) = input.tasks {
            for task_input in task_inputs {
                let task = self.add_task(id, task_input)?;
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

    fn get(&self, id: Uuid) -> Result<Option<WorkOrder>> {
        // Query work order in a scoped block to release connection before loading tasks/materials
        let wo_data = {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let result = conn.query_row(
                "SELECT id, work_order_number, product_id, bom_id, work_center_id, assigned_to,
                        status, priority, quantity_to_build, quantity_completed, scheduled_start,
                        scheduled_end, actual_start, actual_end, notes, created_at, updated_at
                 FROM manufacturing_work_orders WHERE id = ?",
                [id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, Option<String>>(10)?,
                        row.get::<_, Option<String>>(11)?,
                        row.get::<_, Option<String>>(12)?,
                        row.get::<_, Option<String>>(13)?,
                        row.get::<_, Option<String>>(14)?,
                        row.get::<_, String>(15)?,
                        row.get::<_, String>(16)?,
                    ))
                },
            );

            match result {
                Ok(data) => Some(data),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(CommerceError::DatabaseError(e.to_string())),
            }
        }; // Connection released here

        match wo_data {
            Some((
                id_str,
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
                created_at,
                updated_at,
            )) => {
                let wo_id = Uuid::parse_str(&id_str).unwrap_or_default();
                let tasks = self.load_tasks(wo_id)?;
                let materials = self.load_materials(wo_id)?;

                Ok(Some(WorkOrder {
                    id: wo_id,
                    work_order_number,
                    product_id: Uuid::parse_str(&product_id).unwrap_or_default(),
                    bom_id: bom_id.and_then(|s| Uuid::parse_str(&s).ok()),
                    work_center_id,
                    assigned_to: assigned_to.and_then(|s| Uuid::parse_str(&s).ok()),
                    status: Self::parse_status(&status),
                    priority: Self::parse_priority(&priority),
                    quantity_to_build: Decimal::from_str(&quantity_to_build).unwrap_or_default(),
                    quantity_completed: Decimal::from_str(&quantity_completed).unwrap_or_default(),
                    scheduled_start: Self::parse_optional_datetime(scheduled_start),
                    scheduled_end: Self::parse_optional_datetime(scheduled_end),
                    actual_start: Self::parse_optional_datetime(actual_start),
                    actual_end: Self::parse_optional_datetime(actual_end),
                    notes,
                    tasks,
                    materials,
                    version: 1, // Default to 1 for backwards compatibility
                    created_at: Self::parse_datetime(&created_at),
                    updated_at: Self::parse_datetime(&updated_at),
                }))
            }
            None => Ok(None),
        }
    }

    fn get_by_number(&self, work_order_number: &str) -> Result<Option<WorkOrder>> {
        // Query ID in a scoped block to release connection before calling self.get()
        let id_result = {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let result = conn.query_row(
                "SELECT id FROM manufacturing_work_orders WHERE work_order_number = ?",
                [work_order_number],
                |row| row.get::<_, String>(0),
            );

            match result {
                Ok(id_str) => Some(Uuid::parse_str(&id_str).unwrap_or_default()),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(CommerceError::DatabaseError(e.to_string())),
            }
        }; // Connection released here

        match id_result {
            Some(id) => self.get(id),
            None => Ok(None),
        }
    }

    fn update(&self, id: Uuid, input: UpdateWorkOrder) -> Result<WorkOrder> {
        // Get existing work order first (releases connection after)
        let existing = self.get(id)?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let new_status = input.status.unwrap_or(existing.status);
        let new_priority = input.priority.unwrap_or(existing.priority);
        let new_assigned_to = input.assigned_to.or(existing.assigned_to);
        let new_notes = input.notes.or(existing.notes);
        let new_work_center_id = input.work_center_id.or(existing.work_center_id);
        let new_scheduled_start = input.scheduled_start.or(existing.scheduled_start);
        let new_scheduled_end = input.scheduled_end.or(existing.scheduled_end);

        // Do the update in a scoped block
        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            conn.execute(
                "UPDATE manufacturing_work_orders SET status = ?, priority = ?, assigned_to = ?,
                 work_center_id = ?, scheduled_start = ?, scheduled_end = ?, notes = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![
                    new_status.to_string(),
                    new_priority.to_string(),
                    new_assigned_to.map(|u| u.to_string()),
                    new_work_center_id,
                    new_scheduled_start.map(|dt| dt.to_rfc3339()),
                    new_scheduled_end.map(|dt| dt.to_rfc3339()),
                    new_notes,
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        } // Connection released here

        // Fetch and return the updated work order
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn list(&self, filter: WorkOrderFilter) -> Result<Vec<WorkOrder>> {
        // Collect all IDs in a scoped block to release connection before calling self.get()
        let ids = {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let limit = filter.limit.unwrap_or(100) as i64;
            let offset = filter.offset.unwrap_or(0) as i64;

            let mut sql = "SELECT id FROM manufacturing_work_orders WHERE 1=1".to_string();
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

            if let Some(product_id) = filter.product_id {
                sql.push_str(" AND product_id = ?");
                params.push(Box::new(product_id.to_string()));
            }

            if let Some(bom_id) = filter.bom_id {
                sql.push_str(" AND bom_id = ?");
                params.push(Box::new(bom_id.to_string()));
            }

            if let Some(status) = filter.status {
                sql.push_str(" AND status = ?");
                params.push(Box::new(status.to_string()));
            }

            if let Some(priority) = filter.priority {
                sql.push_str(" AND priority = ?");
                params.push(Box::new(priority.to_string()));
            }

            if let Some(assigned_to) = filter.assigned_to {
                sql.push_str(" AND assigned_to = ?");
                params.push(Box::new(assigned_to.to_string()));
            }

            if let Some(work_center_id) = filter.work_center_id {
                sql.push_str(" AND work_center_id = ?");
                params.push(Box::new(work_center_id));
            }

            sql.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");
            params.push(Box::new(limit));
            params.push(Box::new(offset));

            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

            let rows = stmt
                .query_map(param_refs.as_slice(), |row| row.get::<_, String>(0))
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let mut id_list = Vec::new();
            for row in rows {
                let id_str = row.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                id_list.push(Uuid::parse_str(&id_str).unwrap_or_default());
            }
            id_list
        }; // Connection released here

        // Now fetch each work order (each call gets its own connection)
        let mut work_orders = Vec::new();
        for id in ids {
            if let Some(wo) = self.get(id)? {
                work_orders.push(wo);
            }
        }

        Ok(work_orders)
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Mark as cancelled instead of hard delete
        conn.execute(
            "UPDATE manufacturing_work_orders SET status = 'cancelled', updated_at = ? WHERE id = ?",
            rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    fn start(&self, id: Uuid) -> Result<WorkOrder> {
        let now = Utc::now();

        // Do the update in a scoped block
        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            conn.execute(
                "UPDATE manufacturing_work_orders SET status = 'in_progress', actual_start = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        } // Connection released here

        // Fetch and return the updated work order
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn complete(&self, id: Uuid, quantity_completed: Decimal) -> Result<WorkOrder> {
        // Get existing work order first (releases connection after)
        let existing = self.get(id)?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let new_quantity = existing.quantity_completed + quantity_completed;
        let status = if new_quantity >= existing.quantity_to_build {
            "completed"
        } else {
            "partially_completed"
        };

        // Do the update in a scoped block
        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            conn.execute(
                "UPDATE manufacturing_work_orders SET status = ?, quantity_completed = ?, actual_end = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![
                    status,
                    new_quantity.to_string(),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        } // Connection released here

        // Fetch and return the updated work order
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn hold(&self, id: Uuid) -> Result<WorkOrder> {
        self.update(
            id,
            UpdateWorkOrder {
                status: Some(WorkOrderStatus::OnHold),
                ..Default::default()
            },
        )
    }

    fn resume(&self, id: Uuid) -> Result<WorkOrder> {
        self.update(
            id,
            UpdateWorkOrder {
                status: Some(WorkOrderStatus::InProgress),
                ..Default::default()
            },
        )
    }

    fn cancel(&self, id: Uuid) -> Result<WorkOrder> {
        self.update(
            id,
            UpdateWorkOrder {
                status: Some(WorkOrderStatus::Cancelled),
                ..Default::default()
            },
        )
    }

    fn add_task(&self, work_order_id: Uuid, task: CreateWorkOrderTask) -> Result<WorkOrderTask> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();
        let sequence = task.sequence.unwrap_or(1);

        conn.execute(
            "INSERT INTO manufacturing_work_order_tasks (id, work_order_id, sequence, task_name, status, estimated_hours, assigned_to, notes, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'pending', ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                work_order_id.to_string(),
                sequence,
                task.task_name,
                task.estimated_hours.map(|h| h.to_string()),
                task.assigned_to.map(|u| u.to_string()),
                task.notes,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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

    fn update_task(&self, task_id: Uuid, task: UpdateWorkOrderTask) -> Result<WorkOrderTask> {
        // Get existing task first (releases connection after)
        let existing = self.get_task_internal(task_id)?;
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

        // Do the update in a scoped block
        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            conn.execute(
                "UPDATE manufacturing_work_order_tasks SET sequence = ?, task_name = ?, status = ?,
                 estimated_hours = ?, actual_hours = ?, assigned_to = ?, started_at = ?,
                 completed_at = ?, notes = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![
                    new_sequence,
                    new_task_name,
                    new_status.to_string(),
                    new_estimated.map(|h| h.to_string()),
                    new_actual.map(|h| h.to_string()),
                    new_assigned.map(|u| u.to_string()),
                    new_started.map(|dt| dt.to_rfc3339()),
                    new_completed.map(|dt| dt.to_rfc3339()),
                    new_notes,
                    now.to_rfc3339(),
                    task_id.to_string(),
                ],
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        } // Connection released here

        self.get_task_internal(task_id)
    }

    fn remove_task(&self, task_id: Uuid) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "DELETE FROM manufacturing_work_order_tasks WHERE id = ?",
            [task_id.to_string()],
        )
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    fn get_tasks(&self, work_order_id: Uuid) -> Result<Vec<WorkOrderTask>> {
        self.load_tasks(work_order_id)
    }

    fn start_task(&self, task_id: Uuid) -> Result<WorkOrderTask> {
        let now = Utc::now();

        // Do the update in a scoped block
        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            conn.execute(
                "UPDATE manufacturing_work_order_tasks SET status = 'in_progress', started_at = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![now.to_rfc3339(), now.to_rfc3339(), task_id.to_string()],
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        } // Connection released here

        self.get_task_internal(task_id)
    }

    fn complete_task(&self, task_id: Uuid, actual_hours: Option<Decimal>) -> Result<WorkOrderTask> {
        let now = Utc::now();

        // Do the update in a scoped block
        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            conn.execute(
                "UPDATE manufacturing_work_order_tasks SET status = 'completed', actual_hours = ?, completed_at = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![
                    actual_hours.map(|h| h.to_string()),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    task_id.to_string(),
                ],
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        } // Connection released here

        self.get_task_internal(task_id)
    }

    fn add_material(
        &self,
        work_order_id: Uuid,
        material: AddWorkOrderMaterial,
    ) -> Result<WorkOrderMaterial> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO manufacturing_work_order_materials (id, work_order_id, component_id, component_sku, component_name, reserved_quantity, consumed_quantity, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, '0', ?, ?)",
            rusqlite::params![
                id.to_string(),
                work_order_id.to_string(),
                material.component_id.map(|u| u.to_string()),
                material.component_sku,
                material.component_name,
                material.quantity.to_string(),
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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

    fn consume_material(&self, material_id: Uuid, quantity: Decimal) -> Result<WorkOrderMaterial> {
        // Get existing material first (releases connection after)
        let existing = self.get_material_internal(material_id)?;
        let now = Utc::now();

        let new_consumed = existing.consumed_quantity + quantity;

        // Do the update in a scoped block
        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            conn.execute(
                "UPDATE manufacturing_work_order_materials SET consumed_quantity = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![
                    new_consumed.to_string(),
                    now.to_rfc3339(),
                    material_id.to_string(),
                ],
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        } // Connection released here

        self.get_material_internal(material_id)
    }

    fn get_materials(&self, work_order_id: Uuid) -> Result<Vec<WorkOrderMaterial>> {
        self.load_materials(work_order_id)
    }

    fn count(&self, filter: WorkOrderFilter) -> Result<u64> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = "SELECT COUNT(*) FROM manufacturing_work_orders WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(product_id) = filter.product_id {
            sql.push_str(" AND product_id = ?");
            params.push(Box::new(product_id.to_string()));
        }

        if let Some(bom_id) = filter.bom_id {
            sql.push_str(" AND bom_id = ?");
            params.push(Box::new(bom_id.to_string()));
        }

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }

        if let Some(priority) = filter.priority {
            sql.push_str(" AND priority = ?");
            params.push(Box::new(priority.to_string()));
        }

        if let Some(assigned_to) = filter.assigned_to {
            sql.push_str(" AND assigned_to = ?");
            params.push(Box::new(assigned_to.to_string()));
        }

        if let Some(work_center_id) = filter.work_center_id {
            sql.push_str(" AND work_center_id = ?");
            params.push(Box::new(work_center_id));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let count: i64 = conn
            .query_row(&sql, param_refs.as_slice(), |row| row.get(0))
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        Ok(count as u64)
    }
}
