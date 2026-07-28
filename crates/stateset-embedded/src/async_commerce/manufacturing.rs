//! Manufacturing accessors: warranties, BOMs, work orders.

use super::*;

/// Async warranty operations.
pub struct AsyncWarranties {
    db: Arc<PostgresDatabase>,
}

impl AsyncWarranties {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new warranty.
    pub async fn create(&self, input: CreateWarranty) -> Result<Warranty> {
        self.db.warranties().create_async(input).await
    }

    /// Get warranty by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Warranty>> {
        self.db.warranties().get_async(id.into()).await
    }

    /// Get warranty by warranty number.
    pub async fn get_by_number(&self, warranty_number: &str) -> Result<Option<Warranty>> {
        self.db.warranties().get_by_number_async(warranty_number).await
    }

    /// Get warranty by serial number.
    pub async fn get_by_serial(&self, serial_number: &str) -> Result<Option<Warranty>> {
        self.db.warranties().get_by_serial_async(serial_number).await
    }

    /// Update a warranty.
    pub async fn update(&self, id: Uuid, input: UpdateWarranty) -> Result<Warranty> {
        self.db.warranties().update_async(id.into(), input).await
    }

    /// List warranties.
    pub async fn list(&self, filter: WarrantyFilter) -> Result<Vec<Warranty>> {
        self.db.warranties().list_async(filter).await
    }

    /// Get warranties for a customer.
    pub async fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Warranty>> {
        self.db.warranties().for_customer_async(customer_id.into()).await
    }

    /// Get warranties for an order.
    pub async fn for_order(&self, order_id: Uuid) -> Result<Vec<Warranty>> {
        self.db.warranties().for_order_async(order_id.into()).await
    }

    /// Void a warranty.
    pub async fn void(&self, id: Uuid) -> Result<Warranty> {
        self.db.warranties().void_async(id.into()).await
    }

    /// Expire a warranty.
    pub async fn expire(&self, id: Uuid) -> Result<Warranty> {
        self.db.warranties().expire_async(id.into()).await
    }

    /// Transfer warranty to new owner.
    pub async fn transfer(&self, id: Uuid, new_customer_id: Uuid) -> Result<Warranty> {
        self.db.warranties().transfer_async(id.into(), new_customer_id.into()).await
    }

    /// Create a warranty claim.
    pub async fn create_claim(&self, input: CreateWarrantyClaim) -> Result<WarrantyClaim> {
        self.db.warranties().create_claim_async(input).await
    }

    /// Get claim by ID.
    pub async fn get_claim(&self, id: Uuid) -> Result<Option<WarrantyClaim>> {
        self.db.warranties().get_claim_async(id).await
    }

    /// Get claim by claim number.
    pub async fn get_claim_by_number(&self, claim_number: &str) -> Result<Option<WarrantyClaim>> {
        self.db.warranties().get_claim_by_number_async(claim_number).await
    }

    /// Update a claim.
    pub async fn update_claim(
        &self,
        id: Uuid,
        input: UpdateWarrantyClaim,
    ) -> Result<WarrantyClaim> {
        self.db.warranties().update_claim_async(id, input).await
    }

    /// List claims.
    pub async fn list_claims(&self, filter: WarrantyClaimFilter) -> Result<Vec<WarrantyClaim>> {
        self.db.warranties().list_claims_async(filter).await
    }

    /// Get claims for a warranty.
    pub async fn get_claims(&self, warranty_id: Uuid) -> Result<Vec<WarrantyClaim>> {
        self.db.warranties().get_claims_async(warranty_id.into()).await
    }

    /// Approve a claim.
    pub async fn approve_claim(&self, id: Uuid) -> Result<WarrantyClaim> {
        self.db.warranties().approve_claim_async(id).await
    }

    /// Deny a claim.
    pub async fn deny_claim(&self, id: Uuid, reason: &str) -> Result<WarrantyClaim> {
        self.db.warranties().deny_claim_async(id, reason).await
    }

    /// Complete a claim.
    pub async fn complete_claim(
        &self,
        id: Uuid,
        resolution: ClaimResolution,
    ) -> Result<WarrantyClaim> {
        self.db.warranties().complete_claim_async(id, resolution).await
    }

    /// Cancel a claim.
    pub async fn cancel_claim(&self, id: Uuid) -> Result<WarrantyClaim> {
        self.db.warranties().cancel_claim_async(id).await
    }

    /// Count warranties.
    pub async fn count(&self, filter: WarrantyFilter) -> Result<u64> {
        self.db.warranties().count_async(filter).await
    }

    /// Count claims.
    pub async fn count_claims(&self, filter: WarrantyClaimFilter) -> Result<u64> {
        self.db.warranties().count_claims_async(filter).await
    }
}

// ============================================================================
// Async BOM
// ============================================================================

/// Async Bill of Materials operations.
pub struct AsyncBom {
    db: Arc<PostgresDatabase>,
}

impl AsyncBom {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new BOM.
    pub async fn create(&self, input: CreateBom) -> Result<BillOfMaterials> {
        self.db.bom().create_async(input).await
    }

    /// Get BOM by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<BillOfMaterials>> {
        self.db.bom().get_async(id).await
    }

    /// Get BOM by BOM number.
    pub async fn get_by_number(&self, bom_number: &str) -> Result<Option<BillOfMaterials>> {
        self.db.bom().get_by_number_async(bom_number).await
    }

    /// Update a BOM.
    pub async fn update(&self, id: Uuid, input: UpdateBom) -> Result<BillOfMaterials> {
        self.db.bom().update_async(id, input).await
    }

    /// List BOMs.
    pub async fn list(&self, filter: BomFilter) -> Result<Vec<BillOfMaterials>> {
        self.db.bom().list_async(filter).await
    }

    /// Delete a BOM.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.bom().delete_async(id).await
    }

    /// Add component to BOM.
    pub async fn add_component(
        &self,
        bom_id: Uuid,
        component: CreateBomComponent,
    ) -> Result<BomComponent> {
        self.db.bom().add_component_async(bom_id, component).await
    }

    /// Update a component.
    pub async fn update_component(
        &self,
        component_id: Uuid,
        component: CreateBomComponent,
    ) -> Result<BomComponent> {
        self.db.bom().update_component_async(component_id, component).await
    }

    /// Remove component from BOM.
    pub async fn remove_component(&self, component_id: Uuid) -> Result<()> {
        self.db.bom().remove_component_async(component_id).await
    }

    /// Activate a BOM.
    pub async fn activate(&self, id: Uuid) -> Result<BillOfMaterials> {
        self.db.bom().activate_async(id).await
    }

    /// Count BOMs.
    pub async fn count(&self, filter: BomFilter) -> Result<u64> {
        self.db.bom().count_async(filter).await
    }
}

// ============================================================================
// Async Work Orders
// ============================================================================

/// Async work order operations.
pub struct AsyncWorkOrders {
    db: Arc<PostgresDatabase>,
}

impl AsyncWorkOrders {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new work order.
    pub async fn create(&self, input: CreateWorkOrder) -> Result<WorkOrder> {
        self.db.work_orders().create_async(input).await
    }

    /// Get work order by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<WorkOrder>> {
        self.db.work_orders().get_async(id).await
    }

    /// Get work order by work order number.
    pub async fn get_by_number(&self, work_order_number: &str) -> Result<Option<WorkOrder>> {
        self.db.work_orders().get_by_number_async(work_order_number).await
    }

    /// Update a work order.
    pub async fn update(&self, id: Uuid, input: UpdateWorkOrder) -> Result<WorkOrder> {
        self.db.work_orders().update_async(id, input).await
    }

    /// List work orders.
    pub async fn list(&self, filter: WorkOrderFilter) -> Result<Vec<WorkOrder>> {
        self.db.work_orders().list_async(filter).await
    }

    /// Delete a work order.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.work_orders().delete_async(id).await
    }

    /// Start a work order.
    pub async fn start(&self, id: Uuid) -> Result<WorkOrder> {
        self.db.work_orders().start_async(id).await
    }

    /// Complete a work order.
    pub async fn complete(&self, id: Uuid, quantity_completed: Decimal) -> Result<WorkOrder> {
        self.db.work_orders().complete_async(id, quantity_completed).await
    }

    /// Put work order on hold.
    pub async fn hold(&self, id: Uuid) -> Result<WorkOrder> {
        self.db.work_orders().hold_async(id).await
    }

    /// Resume a held work order.
    pub async fn resume(&self, id: Uuid) -> Result<WorkOrder> {
        self.db.work_orders().resume_async(id).await
    }

    /// Cancel a work order.
    pub async fn cancel(&self, id: Uuid) -> Result<WorkOrder> {
        self.db.work_orders().cancel_async(id).await
    }

    /// Add task to work order.
    pub async fn add_task(
        &self,
        work_order_id: Uuid,
        task: CreateWorkOrderTask,
    ) -> Result<WorkOrderTask> {
        self.db.work_orders().add_task_async(work_order_id, task).await
    }

    /// Update a task.
    pub async fn update_task(
        &self,
        task_id: Uuid,
        task: UpdateWorkOrderTask,
    ) -> Result<WorkOrderTask> {
        self.db.work_orders().update_task_async(task_id, task).await
    }

    /// Remove task from work order.
    pub async fn remove_task(&self, task_id: Uuid) -> Result<()> {
        self.db.work_orders().remove_task_async(task_id).await
    }

    /// Start a task.
    pub async fn start_task(&self, task_id: Uuid) -> Result<WorkOrderTask> {
        self.db.work_orders().start_task_async(task_id).await
    }

    /// Complete a task.
    pub async fn complete_task(
        &self,
        task_id: Uuid,
        actual_hours: Option<Decimal>,
    ) -> Result<WorkOrderTask> {
        self.db.work_orders().complete_task_async(task_id, actual_hours).await
    }

    /// Add material to work order.
    pub async fn add_material(
        &self,
        work_order_id: Uuid,
        material: AddWorkOrderMaterial,
    ) -> Result<WorkOrderMaterial> {
        self.db.work_orders().add_material_async(work_order_id, material).await
    }

    /// Consume material.
    pub async fn consume_material(
        &self,
        material_id: Uuid,
        quantity: Decimal,
    ) -> Result<WorkOrderMaterial> {
        self.db.work_orders().consume_material_async(material_id, quantity).await
    }

    /// Count work orders.
    pub async fn count(&self, filter: WorkOrderFilter) -> Result<u64> {
        self.db.work_orders().count_async(filter).await
    }
}

// ============================================================================
// Async Purchase Orders
// ============================================================================
