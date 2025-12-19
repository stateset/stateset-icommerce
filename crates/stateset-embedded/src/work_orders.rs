//! Work Order operations for manufacturing

use rust_decimal::Decimal;
use stateset_core::{
    AddWorkOrderMaterial, CreateWorkOrder, CreateWorkOrderTask, Result,
    UpdateWorkOrder, UpdateWorkOrderTask, WorkOrder, WorkOrderFilter, WorkOrderMaterial,
    WorkOrderTask,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

/// Work Order operations.
///
/// Access via `commerce.work_orders()`.
///
/// # Example
///
/// ```rust,no_run
/// use stateset_embedded::{Commerce, CreateWorkOrder, CreateWorkOrderTask};
/// use rust_decimal_macros::dec;
/// use uuid::Uuid;
///
/// let commerce = Commerce::new("./store.db")?;
///
/// // Create a work order
/// let wo = commerce.work_orders().create(CreateWorkOrder {
///     product_id: Uuid::new_v4(),
///     quantity_to_build: dec!(100),
///     tasks: Some(vec![
///         CreateWorkOrderTask {
///             task_name: "Assembly".into(),
///             sequence: Some(1),
///             estimated_hours: Some(dec!(2)),
///             ..Default::default()
///         },
///     ]),
///     ..Default::default()
/// })?;
///
/// // Start the work order
/// let wo = commerce.work_orders().start(wo.id)?;
///
/// // Complete with quantity
/// let wo = commerce.work_orders().complete(wo.id, dec!(100))?;
/// # Ok::<(), stateset_embedded::CommerceError>(())
/// ```
pub struct WorkOrders {
    db: Arc<dyn Database>,
}

impl WorkOrders {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Create a new work order.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateWorkOrder};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let wo = commerce.work_orders().create(CreateWorkOrder {
    ///     product_id: Uuid::new_v4(),
    ///     quantity_to_build: dec!(50),
    ///     notes: Some("Rush order".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateWorkOrder) -> Result<WorkOrder> {
        self.db.work_orders().create(input)
    }

    /// Get a work order by ID.
    pub fn get(&self, id: Uuid) -> Result<Option<WorkOrder>> {
        self.db.work_orders().get(id)
    }

    /// Get a work order by its work order number.
    pub fn get_by_number(&self, work_order_number: &str) -> Result<Option<WorkOrder>> {
        self.db.work_orders().get_by_number(work_order_number)
    }

    /// Update a work order.
    pub fn update(&self, id: Uuid, input: UpdateWorkOrder) -> Result<WorkOrder> {
        self.db.work_orders().update(id, input)
    }

    /// List work orders with optional filter.
    pub fn list(&self, filter: WorkOrderFilter) -> Result<Vec<WorkOrder>> {
        self.db.work_orders().list(filter)
    }

    /// Delete a work order (cancels if not started).
    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.db.work_orders().delete(id)
    }

    /// Start a work order.
    ///
    /// Transitions the work order from Planned to InProgress and records the actual start time.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::Commerce;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    /// let wo_id = Uuid::new_v4(); // Existing work order ID
    ///
    /// let wo = commerce.work_orders().start(wo_id)?;
    /// assert_eq!(wo.status.to_string(), "in_progress");
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn start(&self, id: Uuid) -> Result<WorkOrder> {
        self.db.work_orders().start(id)
    }

    /// Complete a work order with the quantity produced.
    ///
    /// If the quantity meets or exceeds the target, the order is marked as Completed.
    /// Otherwise, it's marked as PartiallyCompleted.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::Commerce;
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    /// let wo_id = Uuid::new_v4(); // Existing work order ID
    ///
    /// // Complete with full quantity
    /// let wo = commerce.work_orders().complete(wo_id, dec!(100))?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn complete(&self, id: Uuid, quantity_completed: Decimal) -> Result<WorkOrder> {
        self.db.work_orders().complete(id, quantity_completed)
    }

    /// Put a work order on hold.
    pub fn hold(&self, id: Uuid) -> Result<WorkOrder> {
        self.db.work_orders().hold(id)
    }

    /// Resume a held work order.
    pub fn resume(&self, id: Uuid) -> Result<WorkOrder> {
        self.db.work_orders().resume(id)
    }

    /// Cancel a work order.
    pub fn cancel(&self, id: Uuid) -> Result<WorkOrder> {
        self.db.work_orders().cancel(id)
    }

    // Task operations

    /// Add a task to a work order.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateWorkOrderTask};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    /// let wo_id = Uuid::new_v4(); // Existing work order ID
    ///
    /// let task = commerce.work_orders().add_task(wo_id, CreateWorkOrderTask {
    ///     task_name: "Quality Check".into(),
    ///     sequence: Some(10),
    ///     estimated_hours: Some(dec!(0.5)),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn add_task(&self, work_order_id: Uuid, task: CreateWorkOrderTask) -> Result<WorkOrderTask> {
        self.db.work_orders().add_task(work_order_id, task)
    }

    /// Update a task.
    pub fn update_task(&self, task_id: Uuid, task: UpdateWorkOrderTask) -> Result<WorkOrderTask> {
        self.db.work_orders().update_task(task_id, task)
    }

    /// Remove a task from a work order.
    pub fn remove_task(&self, task_id: Uuid) -> Result<()> {
        self.db.work_orders().remove_task(task_id)
    }

    /// Get all tasks for a work order.
    pub fn get_tasks(&self, work_order_id: Uuid) -> Result<Vec<WorkOrderTask>> {
        self.db.work_orders().get_tasks(work_order_id)
    }

    /// Start a task.
    pub fn start_task(&self, task_id: Uuid) -> Result<WorkOrderTask> {
        self.db.work_orders().start_task(task_id)
    }

    /// Complete a task with optional actual hours.
    pub fn complete_task(&self, task_id: Uuid, actual_hours: Option<Decimal>) -> Result<WorkOrderTask> {
        self.db.work_orders().complete_task(task_id, actual_hours)
    }

    // Material operations

    /// Add material to a work order.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, AddWorkOrderMaterial};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    /// let wo_id = Uuid::new_v4(); // Existing work order ID
    ///
    /// let material = commerce.work_orders().add_material(wo_id, AddWorkOrderMaterial {
    ///     component_sku: "SCREW-M3".into(),
    ///     component_name: "M3 Screw".into(),
    ///     quantity: dec!(200),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn add_material(&self, work_order_id: Uuid, material: AddWorkOrderMaterial) -> Result<WorkOrderMaterial> {
        self.db.work_orders().add_material(work_order_id, material)
    }

    /// Consume material during production.
    ///
    /// Records that a certain quantity of material has been used.
    pub fn consume_material(&self, material_id: Uuid, quantity: Decimal) -> Result<WorkOrderMaterial> {
        self.db.work_orders().consume_material(material_id, quantity)
    }

    /// Get all materials for a work order.
    pub fn get_materials(&self, work_order_id: Uuid) -> Result<Vec<WorkOrderMaterial>> {
        self.db.work_orders().get_materials(work_order_id)
    }

    /// Count work orders matching filter.
    pub fn count(&self, filter: WorkOrderFilter) -> Result<u64> {
        self.db.work_orders().count(filter)
    }

    /// Get work orders for a specific product.
    pub fn for_product(&self, product_id: Uuid) -> Result<Vec<WorkOrder>> {
        self.list(WorkOrderFilter {
            product_id: Some(product_id),
            ..Default::default()
        })
    }

    /// Get work orders using a specific BOM.
    pub fn for_bom(&self, bom_id: Uuid) -> Result<Vec<WorkOrder>> {
        self.list(WorkOrderFilter {
            bom_id: Some(bom_id),
            ..Default::default()
        })
    }
}
