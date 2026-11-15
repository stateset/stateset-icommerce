//! SQLite Work Order repository implementation

use super::{
    build_in_clause, map_db_error, params_refs, parse_datetime, parse_datetime_opt,
    parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt_row, parse_decimal_row,
    parse_decimal_strict, parse_enum, parse_enum_row, parse_uuid, parse_uuid_opt,
    parse_uuid_opt_row, parse_uuid_row, uuid_params, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    AddWorkOrderMaterial, BatchResult, CommerceError, CreateWorkOrder, CreateWorkOrderTask,
    ProductId, Result, TaskStatus, UpdateWorkOrder, UpdateWorkOrderTask, WorkOrder,
    WorkOrderFilter, WorkOrderMaterial, WorkOrderRepository, WorkOrderStatus, WorkOrderTask,
    validate_batch_size,
};
use uuid::Uuid;

/// SQLite implementation of `WorkOrderRepository`
#[derive(Debug)]
pub struct SqliteWorkOrderRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteWorkOrderRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn load_tasks(&self, work_order_id: Uuid) -> Result<Vec<WorkOrderTask>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
                    id: parse_uuid_row(&row.get::<_, String>(0)?, "work_order_task", "id")?,
                    work_order_id: parse_uuid_row(
                        &row.get::<_, String>(1)?,
                        "work_order_task",
                        "work_order_id",
                    )?,
                    sequence: row.get(2)?,
                    task_name: row.get(3)?,
                    status: parse_enum_row(&row.get::<_, String>(4)?, "work_order_task", "status")?,
                    estimated_hours: parse_decimal_opt_row(
                        row.get(5)?,
                        "work_order_task",
                        "estimated_hours",
                    )?,
                    actual_hours: parse_decimal_opt_row(
                        row.get(6)?,
                        "work_order_task",
                        "actual_hours",
                    )?,
                    assigned_to: parse_uuid_opt_row(
                        row.get::<_, Option<String>>(7)?,
                        "work_order_task",
                        "assigned_to",
                    )?,
                    started_at: parse_datetime_opt_row(
                        row.get(8)?,
                        "work_order_task",
                        "started_at",
                    )?,
                    completed_at: parse_datetime_opt_row(
                        row.get(9)?,
                        "work_order_task",
                        "completed_at",
                    )?,
                    notes: row.get(10)?,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>(11)?,
                        "work_order_task",
                        "created_at",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>(12)?,
                        "work_order_task",
                        "updated_at",
                    )?,
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
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
                    id: parse_uuid_row(&row.get::<_, String>(0)?, "work_order_material", "id")?,
                    work_order_id: parse_uuid_row(
                        &row.get::<_, String>(1)?,
                        "work_order_material",
                        "work_order_id",
                    )?,
                    component_id: parse_uuid_opt_row(
                        row.get::<_, Option<String>>(2)?,
                        "work_order_material",
                        "component_id",
                    )?,
                    component_sku: row.get(3)?,
                    component_name: row.get(4)?,
                    reserved_quantity: parse_decimal_row(
                        &row.get::<_, String>(5)?,
                        "work_order_material",
                        "reserved_quantity",
                    )?,
                    consumed_quantity: parse_decimal_row(
                        &row.get::<_, String>(6)?,
                        "work_order_material",
                        "consumed_quantity",
                    )?,
                    inventory_reservation_id: parse_uuid_opt_row(
                        row.get::<_, Option<String>>(7)?,
                        "work_order_material",
                        "inventory_reservation_id",
                    )?,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>(8)?,
                        "work_order_material",
                        "created_at",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>(9)?,
                        "work_order_material",
                        "updated_at",
                    )?,
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
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.query_row(
            "SELECT id, work_order_id, sequence, task_name, status, estimated_hours,
                    actual_hours, assigned_to, started_at, completed_at, notes, created_at, updated_at
             FROM manufacturing_work_order_tasks WHERE id = ?",
            [task_id.to_string()],
            |row| {
                Ok(WorkOrderTask {
                    id: parse_uuid_row(&row.get::<_, String>(0)?, "work_order_task", "id")?,
                    work_order_id: parse_uuid_row(&row.get::<_, String>(1)?, "work_order_task", "work_order_id")?,
                    sequence: row.get(2)?,
                    task_name: row.get(3)?,
                    status: parse_enum_row(&row.get::<_, String>(4)?, "work_order_task", "status")?,
                    estimated_hours: parse_decimal_opt_row(row.get(5)?, "work_order_task", "estimated_hours")?,
                    actual_hours: parse_decimal_opt_row(row.get(6)?, "work_order_task", "actual_hours")?,
                    assigned_to: parse_uuid_opt_row(
                        row.get::<_, Option<String>>(7)?,
                        "work_order_task",
                        "assigned_to",
                    )?,
                    started_at: parse_datetime_opt_row(row.get(8)?, "work_order_task", "started_at")?,
                    completed_at: parse_datetime_opt_row(row.get(9)?, "work_order_task", "completed_at")?,
                    notes: row.get(10)?,
                    created_at: parse_datetime_row(&row.get::<_, String>(11)?, "work_order_task", "created_at")?,
                    updated_at: parse_datetime_row(&row.get::<_, String>(12)?, "work_order_task", "updated_at")?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => CommerceError::NotFound,
            _ => CommerceError::DatabaseError(e.to_string()),
        })
    }

    fn get_material_internal(&self, material_id: Uuid) -> Result<WorkOrderMaterial> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.query_row(
            "SELECT id, work_order_id, component_id, component_sku, component_name,
                    reserved_quantity, consumed_quantity, inventory_reservation_id, created_at, updated_at
             FROM manufacturing_work_order_materials WHERE id = ?",
            [material_id.to_string()],
            |row| {
                Ok(WorkOrderMaterial {
                    id: parse_uuid_row(&row.get::<_, String>(0)?, "work_order_material", "id")?,
                    work_order_id: parse_uuid_row(&row.get::<_, String>(1)?, "work_order_material", "work_order_id")?,
                    component_id: parse_uuid_opt_row(
                        row.get::<_, Option<String>>(2)?,
                        "work_order_material",
                        "component_id",
                    )?,
                    component_sku: row.get(3)?,
                    component_name: row.get(4)?,
                    reserved_quantity: parse_decimal_row(&row.get::<_, String>(5)?, "work_order_material", "reserved_quantity")?,
                    consumed_quantity: parse_decimal_row(&row.get::<_, String>(6)?, "work_order_material", "consumed_quantity")?,
                    inventory_reservation_id: parse_uuid_opt_row(
                        row.get::<_, Option<String>>(7)?,
                        "work_order_material",
                        "inventory_reservation_id",
                    )?,
                    created_at: parse_datetime_row(&row.get::<_, String>(8)?, "work_order_material", "created_at")?,
                    updated_at: parse_datetime_row(&row.get::<_, String>(9)?, "work_order_material", "updated_at")?,
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
        with_immediate_transaction(&self.pool, |tx| {
            super::work_order_stock::create(tx, id, &input)
        })?;
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn get(&self, id: Uuid) -> Result<Option<WorkOrder>> {
        // Query work order in a scoped block to release connection before loading tasks/materials
        let wo_data = {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
                let wo_id = parse_uuid(&id_str, "work_order", "id")?;
                let tasks = self.load_tasks(wo_id)?;
                let materials = self.load_materials(wo_id)?;

                Ok(Some(WorkOrder {
                    stock: {
                        let conn = self
                            .pool
                            .get()
                            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                        super::work_order_stock::stock(&conn, wo_id).map_err(map_db_error)?
                    },
                    id: wo_id,
                    work_order_number,
                    product_id: ProductId::from(parse_uuid(
                        &product_id,
                        "work_order",
                        "product_id",
                    )?),
                    bom_id: parse_uuid_opt(bom_id, "work_order", "bom_id")?,
                    work_center_id,
                    assigned_to: parse_uuid_opt(assigned_to, "work_order", "assigned_to")?,
                    status: parse_enum(&status, "work_order", "status")?,
                    priority: parse_enum(&priority, "work_order", "priority")?,
                    quantity_to_build: parse_decimal_strict(
                        &quantity_to_build,
                        "work_order",
                        "quantity_to_build",
                    )?,
                    quantity_completed: parse_decimal_strict(
                        &quantity_completed,
                        "work_order",
                        "quantity_completed",
                    )?,
                    scheduled_start: parse_datetime_opt(
                        scheduled_start,
                        "work_order",
                        "scheduled_start",
                    )?,
                    scheduled_end: parse_datetime_opt(
                        scheduled_end,
                        "work_order",
                        "scheduled_end",
                    )?,
                    actual_start: parse_datetime_opt(actual_start, "work_order", "actual_start")?,
                    actual_end: parse_datetime_opt(actual_end, "work_order", "actual_end")?,
                    notes,
                    tasks,
                    materials,
                    version: {
                        let conn = self
                            .pool
                            .get()
                            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                        conn.query_row(
                            "SELECT version FROM manufacturing_work_orders WHERE id = ?1",
                            [&id_str],
                            |r| r.get(0),
                        )
                        .map_err(map_db_error)?
                    }, // Default to 1 for backwards compatibility
                    created_at: parse_datetime(&created_at, "work_order", "created_at")?,
                    updated_at: parse_datetime(&updated_at, "work_order", "updated_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    fn get_by_number(&self, work_order_number: &str) -> Result<Option<WorkOrder>> {
        // Query ID in a scoped block to release connection before calling self.get()
        let id_result = {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let result = conn.query_row(
                "SELECT id FROM manufacturing_work_orders WHERE work_order_number = ?",
                [work_order_number],
                |row| row.get::<_, String>(0),
            );

            match result {
                Ok(id_str) => Some(parse_uuid(&id_str, "work_order", "id")?),
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
        with_immediate_transaction(&self.pool, |tx| {
            super::work_order_stock::update(tx, id, &input)
        })?;
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn list(&self, filter: WorkOrderFilter) -> Result<Vec<WorkOrder>> {
        // Collect all IDs in a scoped block to release connection before calling self.get()
        let ids = {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let limit = i64::from(super::effective_limit(filter.limit));
            let offset = i64::from(filter.offset.unwrap_or(0));

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
            if filter.overdue_only.unwrap_or(false) {
                sql.push_str(" AND scheduled_end IS NOT NULL AND scheduled_end < ? AND status NOT IN ('completed', 'cancelled')");
                params.push(Box::new(Utc::now().to_rfc3339()));
            }

            // Keyset cursor: (created_at, id) for stable DESC ordering
            if let Some((cursor_created, cursor_id)) = &filter.after_cursor {
                sql.push_str(" AND (created_at < ? OR (created_at = ? AND id < ?))");
                params.push(Box::new(cursor_created.clone()));
                params.push(Box::new(cursor_created.clone()));
                params.push(Box::new(cursor_id.clone()));
            }

            sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?");
            params.push(Box::new(limit));
            // Offset pagination applies only in non-cursor mode.
            params.push(Box::new(if filter.after_cursor.is_none() { offset } else { 0 }));

            let mut stmt =
                conn.prepare(&sql).map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let param_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(std::convert::AsRef::as_ref).collect();

            let rows = stmt
                .query_map(param_refs.as_slice(), |row| row.get::<_, String>(0))
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let mut id_list = Vec::new();
            for row in rows {
                let id_str = row.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                id_list.push(parse_uuid(&id_str, "work_order", "id")?);
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
        self.cancel(id).map(|_| ())
    }

    fn start(&self, id: Uuid) -> Result<WorkOrder> {
        with_immediate_transaction(&self.pool, |tx| {
            super::work_order_stock::transition(tx, id, WorkOrderStatus::InProgress)
        })?;
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn complete(&self, id: Uuid, quantity_completed: Decimal) -> Result<WorkOrder> {
        with_immediate_transaction(&self.pool, |tx| {
            super::work_order_stock::complete(tx, id, quantity_completed)
        })?;
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn hold(&self, id: Uuid) -> Result<WorkOrder> {
        with_immediate_transaction(&self.pool, |tx| {
            super::work_order_stock::transition(tx, id, WorkOrderStatus::OnHold)
        })?;
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn resume(&self, id: Uuid) -> Result<WorkOrder> {
        with_immediate_transaction(&self.pool, |tx| {
            super::work_order_stock::transition(tx, id, WorkOrderStatus::InProgress)
        })?;
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn cancel(&self, id: Uuid) -> Result<WorkOrder> {
        with_immediate_transaction(&self.pool, |tx| {
            super::work_order_stock::transition(tx, id, WorkOrderStatus::Cancelled)
        })?;
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn add_task(&self, work_order_id: Uuid, task: CreateWorkOrderTask) -> Result<WorkOrderTask> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
        let material_id = with_immediate_transaction(&self.pool, |tx| {
            super::work_order_stock::add_material(tx, work_order_id, &material)
        })?;
        self.get_material_internal(material_id)
    }

    fn consume_material(&self, material_id: Uuid, quantity: Decimal) -> Result<WorkOrderMaterial> {
        with_immediate_transaction(&self.pool, |tx| {
            super::work_order_stock::consume_material(tx, material_id, quantity)
        })?;
        self.get_material_internal(material_id)
    }

    fn get_materials(&self, work_order_id: Uuid) -> Result<Vec<WorkOrderMaterial>> {
        self.load_materials(work_order_id)
    }

    fn count(&self, filter: WorkOrderFilter) -> Result<u64> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
        if filter.overdue_only.unwrap_or(false) {
            sql.push_str(" AND scheduled_end IS NOT NULL AND scheduled_end < ? AND status NOT IN ('completed', 'cancelled')");
            params.push(Box::new(Utc::now().to_rfc3339()));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();

        let count: i64 = conn
            .query_row(&sql, param_refs.as_slice(), |row| row.get(0))
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        Ok(count as u64)
    }

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreateWorkOrder>) -> Result<BatchResult<WorkOrder>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(work_order) => result.record_success(work_order),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateWorkOrder>) -> Result<Vec<WorkOrder>> {
        validate_batch_size(&inputs)?;
        let ids = with_immediate_transaction(&self.pool, |tx| {
            let mut result = Vec::with_capacity(inputs.len());
            for input in &inputs {
                let id = Uuid::new_v4();
                super::work_order_stock::create(tx, id, input)?;
                result.push(id);
            }
            Ok(result)
        })?;
        ids.into_iter().map(|id| self.get(id)?.ok_or(CommerceError::NotFound)).collect()
    }

    fn update_batch(
        &self,
        updates: Vec<(Uuid, UpdateWorkOrder)>,
    ) -> Result<BatchResult<WorkOrder>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update(id, input) {
                Ok(work_order) => result.record_success(work_order),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateWorkOrder)>) -> Result<Vec<WorkOrder>> {
        validate_batch_size(&updates)?;
        let ids = with_immediate_transaction(&self.pool, |tx| {
            let mut result = Vec::with_capacity(updates.len());
            for (id, input) in &updates {
                let id = *id;

                super::work_order_stock::update(tx, id, input)?;
                result.push(id);
            }
            Ok(result)
        })?;
        ids.into_iter().map(|id| self.get(id)?.ok_or(CommerceError::NotFound)).collect()
    }

    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;
        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            match self.delete(id) {
                Ok(()) => result.record_success(id),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()> {
        validate_batch_size(&ids)?;
        with_immediate_transaction(&self.pool, |tx| {
            let mut result = Vec::with_capacity(ids.len());
            for id in &ids {
                let id = *id;
                super::work_order_stock::transition(tx, id, WorkOrderStatus::Cancelled)?;
                result.push(id);
            }
            Ok(result)
        })?;
        Ok(())
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<WorkOrder>> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }

        // Query all work orders in a single query
        let work_order_data = {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let placeholders = build_in_clause(ids.len());
            let sql = format!(
                "SELECT id, work_order_number, product_id, bom_id, work_center_id, assigned_to,
                        status, priority, quantity_to_build, quantity_completed, scheduled_start,
                        scheduled_end, actual_start, actual_end, notes, created_at, updated_at
                 FROM manufacturing_work_orders WHERE id IN ({placeholders})"
            );

            let params = uuid_params(&ids);
            let param_refs = params_refs(&params);

            let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;

            let rows = stmt
                .query_map(param_refs.as_slice(), |row| {
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
                })
                .map_err(map_db_error)?;

            let mut data = Vec::new();
            for row in rows {
                data.push(row.map_err(map_db_error)?);
            }
            data
        };

        // Build work orders, loading tasks and materials for each
        let mut work_orders = Vec::with_capacity(work_order_data.len());
        for (
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
        ) in work_order_data
        {
            let wo_id = parse_uuid(&id_str, "work_order", "id")?;
            let tasks = self.load_tasks(wo_id)?;
            let materials = self.load_materials(wo_id)?;

            work_orders.push(WorkOrder {
                stock: {
                    let conn =
                        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                    super::work_order_stock::stock(&conn, wo_id).map_err(map_db_error)?
                },
                id: wo_id,
                work_order_number,
                product_id: ProductId::from(parse_uuid(&product_id, "work_order", "product_id")?),
                bom_id: parse_uuid_opt(bom_id, "work_order", "bom_id")?,
                work_center_id,
                assigned_to: parse_uuid_opt(assigned_to, "work_order", "assigned_to")?,
                status: parse_enum(&status, "work_order", "status")?,
                priority: parse_enum(&priority, "work_order", "priority")?,
                quantity_to_build: parse_decimal_strict(
                    &quantity_to_build,
                    "work_order",
                    "quantity_to_build",
                )?,
                quantity_completed: parse_decimal_strict(
                    &quantity_completed,
                    "work_order",
                    "quantity_completed",
                )?,
                scheduled_start: parse_datetime_opt(
                    scheduled_start,
                    "work_order",
                    "scheduled_start",
                )?,
                scheduled_end: parse_datetime_opt(scheduled_end, "work_order", "scheduled_end")?,
                actual_start: parse_datetime_opt(actual_start, "work_order", "actual_start")?,
                actual_end: parse_datetime_opt(actual_end, "work_order", "actual_end")?,
                notes,
                tasks,
                materials,
                version: {
                    let conn =
                        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                    conn.query_row(
                        "SELECT version FROM manufacturing_work_orders WHERE id = ?1",
                        [&id_str],
                        |r| r.get(0),
                    )
                    .map_err(map_db_error)?
                },
                created_at: parse_datetime(&created_at, "work_order", "created_at")?,
                updated_at: parse_datetime(&updated_at, "work_order", "updated_at")?,
            });
        }

        Ok(work_orders)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{
        AddWorkOrderMaterial, CreateWorkOrder, CreateWorkOrderTask, TaskStatus, WorkOrderFilter,
        WorkOrderRepository, WorkOrderStatus,
    };

    fn fresh_repo() -> SqliteWorkOrderRepository {
        SqliteDatabase::in_memory().expect("in-memory").work_orders()
    }

    fn make_input(repo: &SqliteWorkOrderRepository, qty: Decimal) -> CreateWorkOrder {
        use stateset_core::{ProductRepository, WarehouseRepository};
        let tag = Uuid::new_v4().to_string();
        let product = super::super::products::SqliteProductRepository::new(repo.pool.clone())
            .create(stateset_core::CreateProduct {
                name: tag.clone(),
                variants: Some(vec![stateset_core::CreateProductVariant {
                    sku: tag.clone(),
                    price: dec!(1),
                    ..Default::default()
                }]),
                ..Default::default()
            })
            .unwrap();
        let warehouse = super::super::warehouse::SqliteWarehouseRepository::new(repo.pool.clone());
        let wh = warehouse
            .create_warehouse(stateset_core::CreateWarehouse {
                code: tag.clone(),
                name: tag.clone(),
                ..Default::default()
            })
            .unwrap();
        let location = warehouse
            .create_location(stateset_core::CreateLocation {
                warehouse_id: wh.id,
                ..Default::default()
            })
            .unwrap();
        CreateWorkOrder {
            product_id: product.id,
            quantity_to_build: qty,
            stock: Some(stateset_core::WorkOrderStock {
                source_location_id: location.id,
                output_location_id: location.id,
                output_sku: tag,
            }),
            ..Default::default()
        }
    }

    fn make_wo(repo: &SqliteWorkOrderRepository, qty: Decimal) -> WorkOrder {
        repo.create(make_input(repo, qty)).expect("create wo")
    }

    fn seed_material(
        repo: &SqliteWorkOrderRepository,
        wo: &WorkOrder,
        sku: &str,
        quantity: Decimal,
    ) {
        use stateset_core::WarehouseRepository;
        super::super::warehouse::SqliteWarehouseRepository::new(repo.pool.clone())
            .adjust_inventory(stateset_core::AdjustLocationInventory {
                location_id: wo.stock.as_ref().unwrap().source_location_id,
                sku: sku.into(),
                lot_id: None,
                quantity,
                reason: "fixture".into(),
                reference_type: None,
                reference_id: None,
                performed_by: None,
            })
            .unwrap();
    }

    #[test]
    fn create_wo_starts_in_planned() {
        let repo = fresh_repo();
        let wo = make_wo(&repo, dec!(10));
        assert_eq!(wo.status, WorkOrderStatus::Planned);
        assert_eq!(wo.quantity_to_build, dec!(10));
        assert!(!wo.work_order_number.is_empty());
    }

    #[test]
    fn create_wo_with_tasks_persists_them() {
        let repo = fresh_repo();
        let wo = repo
            .create(CreateWorkOrder {
                tasks: Some(vec![
                    CreateWorkOrderTask {
                        sequence: Some(1),
                        task_name: "Assembly".into(),
                        estimated_hours: Some(dec!(2)),
                        assigned_to: None,
                        notes: None,
                    },
                    CreateWorkOrderTask {
                        sequence: Some(2),
                        task_name: "QA".into(),
                        estimated_hours: Some(dec!(1)),
                        assigned_to: None,
                        notes: None,
                    },
                ]),
                ..make_input(&repo, dec!(5))
            })
            .expect("create");
        let tasks = repo.get_tasks(wo.id).expect("tasks");
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn get_and_get_by_number_round_trip() {
        let repo = fresh_repo();
        let wo = make_wo(&repo, dec!(1));
        let by_id = repo.get(wo.id).expect("ok").expect("found");
        assert_eq!(by_id.id, wo.id);
        let by_num = repo.get_by_number(&wo.work_order_number).expect("ok").expect("found");
        assert_eq!(by_num.id, wo.id);
        assert!(repo.get_by_number("missing").expect("ok").is_none());
    }

    #[test]
    fn start_transitions_to_in_progress() {
        let repo = fresh_repo();
        let wo = make_wo(&repo, dec!(1));
        let started = repo.start(wo.id).expect("start");
        assert_eq!(started.status, WorkOrderStatus::InProgress);
    }

    #[test]
    fn complete_full_quantity_marks_completed() {
        let repo = fresh_repo();
        let wo = make_wo(&repo, dec!(5));
        repo.start(wo.id).expect("start");
        let done = repo.complete(wo.id, dec!(5)).expect("complete");
        assert_eq!(done.status, WorkOrderStatus::Completed);
        assert_eq!(done.quantity_completed, dec!(5));
    }

    #[test]
    fn complete_partial_quantity_marks_partially_completed() {
        let repo = fresh_repo();
        let wo = make_wo(&repo, dec!(10));
        repo.start(wo.id).expect("start");
        let partial = repo.complete(wo.id, dec!(7)).expect("partial");
        assert_eq!(partial.status, WorkOrderStatus::PartiallyCompleted);
        assert_eq!(partial.quantity_completed, dec!(7));
    }

    #[test]
    fn completion_cannot_exceed_planned_quantity() {
        let repo = fresh_repo();
        let wo = make_wo(&repo, dec!(5));
        repo.start(wo.id).expect("start");
        let partial = repo.complete(wo.id, dec!(3)).expect("partial");
        assert_eq!(partial.quantity_completed, dec!(3));
        assert!(matches!(repo.complete(wo.id, dec!(3)), Err(CommerceError::ValidationError(_))));
        assert_eq!(repo.get(wo.id).expect("get").expect("work order").quantity_completed, dec!(3));
        assert_eq!(
            repo.complete(wo.id, dec!(2)).expect("exact remainder").quantity_completed,
            dec!(5)
        );
        assert!(matches!(repo.complete(wo.id, dec!(1)), Err(CommerceError::Conflict(_))));
    }

    #[test]
    fn cancel_transitions_to_cancelled() {
        let repo = fresh_repo();
        let wo = make_wo(&repo, dec!(1));
        let cancelled = repo.cancel(wo.id).expect("cancel");
        assert_eq!(cancelled.status, WorkOrderStatus::Cancelled);
    }

    #[test]
    fn list_filters_by_status() {
        let repo = fresh_repo();
        let planned = make_wo(&repo, dec!(1));
        let in_progress = make_wo(&repo, dec!(1));
        repo.start(in_progress.id).expect("start");

        let pl = repo
            .list(WorkOrderFilter { status: Some(WorkOrderStatus::Planned), ..Default::default() })
            .expect("planned");
        let ip = repo
            .list(WorkOrderFilter {
                status: Some(WorkOrderStatus::InProgress),
                ..Default::default()
            })
            .expect("in_progress");
        assert!(pl.iter().any(|w| w.id == planned.id));
        assert!(ip.iter().any(|w| w.id == in_progress.id));
    }

    #[test]
    fn add_task_appends_to_work_order() {
        let repo = fresh_repo();
        let wo = make_wo(&repo, dec!(1));
        let task = repo
            .add_task(
                wo.id,
                CreateWorkOrderTask {
                    sequence: Some(1),
                    task_name: "Solder".into(),
                    estimated_hours: Some(dec!(0.5)),
                    assigned_to: None,
                    notes: None,
                },
            )
            .expect("add task");
        assert_eq!(task.task_name, "Solder");
        let tasks = repo.get_tasks(wo.id).expect("tasks");
        assert_eq!(tasks.len(), 1);
    }

    #[test]
    fn start_and_complete_task_transitions() {
        let repo = fresh_repo();
        let wo = make_wo(&repo, dec!(1));
        let task = repo
            .add_task(
                wo.id,
                CreateWorkOrderTask {
                    sequence: Some(1),
                    task_name: "Wash".into(),
                    estimated_hours: Some(dec!(1)),
                    assigned_to: None,
                    notes: None,
                },
            )
            .expect("add");
        let started = repo.start_task(task.id).expect("start");
        assert_eq!(started.status, TaskStatus::InProgress);
        let completed = repo.complete_task(task.id, Some(dec!(1.5))).expect("complete");
        assert_eq!(completed.status, TaskStatus::Completed);
    }

    #[test]
    fn add_and_consume_material() {
        let repo = fresh_repo();
        let wo = make_wo(&repo, dec!(1));
        let mat = repo
            .add_material(
                wo.id,
                AddWorkOrderMaterial {
                    component_id: None,
                    component_sku: "PART-A".into(),
                    component_name: "Resistor".into(),
                    quantity: dec!(10),
                },
            )
            .expect("add mat");
        assert_eq!(mat.component_sku, "PART-A");

        seed_material(&repo, &wo, "PART-A", dec!(10));
        repo.start(wo.id).unwrap();
        let consumed = repo.consume_material(mat.id, dec!(4)).expect("consume");
        assert_eq!(consumed.id, mat.id);
        let remaining_materials = repo.get_materials(wo.id).expect("materials");
        assert_eq!(remaining_materials.len(), 1);
    }

    #[test]
    fn material_consumption_is_bounded_and_serialized() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let db = Arc::new(SqliteDatabase::in_memory().expect("in-memory"));
        let repo = db.work_orders();
        let wo = make_wo(&repo, dec!(1));
        assert!(
            repo.add_material(
                wo.id,
                AddWorkOrderMaterial {
                    component_id: None,
                    component_sku: "BAD".into(),
                    component_name: "Bad".into(),
                    quantity: dec!(-1),
                }
            )
            .is_err()
        );
        let mat = repo
            .add_material(
                wo.id,
                AddWorkOrderMaterial {
                    component_id: None,
                    component_sku: "PART-A".into(),
                    component_name: "Resistor".into(),
                    quantity: dec!(10),
                },
            )
            .expect("add material");
        seed_material(&repo, &wo, "PART-A", dec!(10));
        repo.start(wo.id).unwrap();
        assert!(repo.consume_material(mat.id, dec!(-1)).is_err());
        assert!(repo.consume_material(mat.id, dec!(11)).is_err());

        let barrier = Arc::new(Barrier::new(10));
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let db = Arc::clone(&db);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    db.work_orders().consume_material(mat.id, dec!(1))
                })
            })
            .collect();
        for handle in handles {
            handle.join().expect("thread").expect("consume");
        }
        let stored = db.work_orders().get_materials(wo.id).expect("materials");
        assert_eq!(stored[0].consumed_quantity, dec!(10));
        assert!(db.work_orders().consume_material(mat.id, dec!(1)).is_err());
    }

    #[test]
    fn material_consumption_overflow_returns_validation_error() {
        let repo = fresh_repo();
        let wo = make_wo(&repo, dec!(1));
        let mat = repo
            .add_material(
                wo.id,
                AddWorkOrderMaterial {
                    component_id: None,
                    component_sku: "MAX".into(),
                    component_name: "Maximum quantity".into(),
                    quantity: Decimal::MAX,
                },
            )
            .expect("add");
        seed_material(&repo, &wo, "MAX", Decimal::MAX);
        repo.start(wo.id).unwrap();
        repo.consume_material(mat.id, Decimal::MAX).expect("consume full reservation");
        let err = repo.consume_material(mat.id, dec!(1)).expect_err("reject overflow");
        assert!(
            matches!(err, CommerceError::ValidationError(ref message)
                if message.contains("overflow")),
            "got {err:?}"
        );
        assert_eq!(
            repo.get_materials(wo.id).expect("materials")[0].consumed_quantity,
            Decimal::MAX
        );
    }

    #[test]
    fn create_batch_returns_per_input_results() {
        let repo = fresh_repo();
        let result = repo
            .create_batch(vec![make_input(&repo, dec!(1)), make_input(&repo, dec!(2))])
            .expect("batch");
        assert_eq!(result.success_count, 2);
        assert_eq!(result.failure_count, 0);
    }

    #[test]
    fn get_unknown_id_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get(Uuid::new_v4()).expect("ok").is_none());
    }

    #[test]
    fn get_batch_returns_only_existing() {
        let repo = fresh_repo();
        let w1 = make_wo(&repo, dec!(1));
        let w2 = make_wo(&repo, dec!(2));
        let stranger = Uuid::new_v4();
        let fetched = repo.get_batch(vec![w1.id, w2.id, stranger]).expect("ok");
        assert_eq!(fetched.len(), 2);
    }

    #[test]
    fn concurrent_completions_are_not_lost() {
        // Ten completions of one unit each land simultaneously on a work order.
        // `complete` is a read-modify-write of `quantity_completed`; without
        // serialization the reads race and completions are silently overwritten
        // (lost updates). Every committed completion must be counted.
        use std::sync::{Arc, Barrier};
        use std::thread;

        let db = Arc::new(SqliteDatabase::in_memory().expect("in-memory"));
        let wo = make_wo(&db.work_orders(), dec!(1000));
        db.work_orders().start(wo.id).unwrap();

        let thread_count = 10usize;
        let barrier = Arc::new(Barrier::new(thread_count));
        let mut handles = Vec::new();
        for _ in 0..thread_count {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let id = wo.id;
            handles.push(thread::spawn(move || {
                let repo = db.work_orders();
                barrier.wait();
                repo.complete(id, dec!(1))
            }));
        }
        let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread")).collect();
        // A failure is acceptable only if it is a transient lock (the caller
        // retries); a lost update is never acceptable.
        assert!(
            results
                .iter()
                .all(|r| r.is_ok() || format!("{:?}", r.as_ref().unwrap_err()).contains("locked")),
            "unexpected non-lock failure: {results:?}"
        );

        let fetched = db.work_orders().get(wo.id).expect("get").expect("found");
        // Each completion adds exactly one unit; with the read+write serialized
        // every one lands, so the total equals the thread count (no lost updates).
        assert_eq!(
            fetched.quantity_completed,
            Decimal::from(thread_count as u64),
            "completions were lost to a race: {results:?}"
        );
    }

    #[test]
    fn list_after_cursor_paginates_without_overlap() {
        let repo = fresh_repo();
        for _ in 0..3 {
            make_wo(&repo, dec!(1));
        }
        let all = repo.list(WorkOrderFilter::default()).expect("list all");
        assert_eq!(all.len(), 3);

        let first_page =
            repo.list(WorkOrderFilter { limit: Some(2), ..Default::default() }).expect("page 1");
        assert_eq!(first_page.len(), 2);
        assert_eq!(first_page[0].id, all[0].id);

        let last = &first_page[1];
        let second_page = repo
            .list(WorkOrderFilter {
                after_cursor: Some((last.created_at.to_rfc3339(), last.id.to_string())),
                ..Default::default()
            })
            .expect("page 2");
        assert_eq!(second_page.len(), 1);
        assert_eq!(second_page[0].id, all[2].id);
    }
}
