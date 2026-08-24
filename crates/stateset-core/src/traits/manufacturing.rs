//! Bill-of-materials, work-order, quality, and production-batch repositories.

use super::*;

/// Bill of Materials repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait BomRepository: Send + Sync {
    /// Create a new BOM
    fn create(&self, input: CreateBom) -> Result<BillOfMaterials>;

    /// Get BOM by ID
    fn get(&self, id: Uuid) -> Result<Option<BillOfMaterials>>;

    /// Get BOM by BOM number
    fn get_by_number(&self, bom_number: &str) -> Result<Option<BillOfMaterials>>;

    /// Update a BOM
    fn update(&self, id: Uuid, input: UpdateBom) -> Result<BillOfMaterials>;

    /// List BOMs with filter
    fn list(&self, filter: BomFilter) -> Result<Vec<BillOfMaterials>>;

    /// Delete a BOM (marks as obsolete)
    fn delete(&self, id: Uuid) -> Result<()>;

    /// Add component to BOM
    fn add_component(&self, bom_id: Uuid, component: CreateBomComponent) -> Result<BomComponent>;

    /// Update a BOM component
    fn update_component(
        &self,
        component_id: Uuid,
        component: CreateBomComponent,
    ) -> Result<BomComponent>;

    /// Remove component from BOM
    fn remove_component(&self, component_id: Uuid) -> Result<()>;

    /// Get all components for a BOM
    fn get_components(&self, bom_id: Uuid) -> Result<Vec<BomComponent>>;

    /// Activate a BOM (make it ready for production use)
    fn activate(&self, id: Uuid) -> Result<BillOfMaterials>;

    /// Count BOMs matching filter
    fn count(&self, filter: BomFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple BOMs - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateBom>) -> Result<BatchResult<BillOfMaterials>>;

    /// Create multiple BOMs - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateBom>) -> Result<Vec<BillOfMaterials>>;

    /// Update multiple BOMs - partial success allowed
    fn update_batch(&self, updates: Vec<(Uuid, UpdateBom)>)
    -> Result<BatchResult<BillOfMaterials>>;

    /// Update multiple BOMs - atomic (all-or-nothing)
    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateBom)>) -> Result<Vec<BillOfMaterials>>;

    /// Delete multiple BOMs - partial success allowed
    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple BOMs - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()>;

    /// Get multiple BOMs by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<BillOfMaterials>>;
}

/// Work Order repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait WorkOrderRepository: Send + Sync {
    /// Create a new work order
    fn create(&self, input: CreateWorkOrder) -> Result<WorkOrder>;

    /// Get work order by ID
    fn get(&self, id: Uuid) -> Result<Option<WorkOrder>>;

    /// Get work order by work order number
    fn get_by_number(&self, work_order_number: &str) -> Result<Option<WorkOrder>>;

    /// Update a work order
    fn update(&self, id: Uuid, input: UpdateWorkOrder) -> Result<WorkOrder>;

    /// List work orders with filter
    fn list(&self, filter: WorkOrderFilter) -> Result<Vec<WorkOrder>>;

    /// Delete a work order (cancels if not started)
    fn delete(&self, id: Uuid) -> Result<()>;

    /// Start a work order (transitions from planned to `in_progress`)
    fn start(&self, id: Uuid) -> Result<WorkOrder>;

    /// Complete a work order
    fn complete(&self, id: Uuid, quantity_completed: rust_decimal::Decimal) -> Result<WorkOrder>;

    /// Put work order on hold
    fn hold(&self, id: Uuid) -> Result<WorkOrder>;

    /// Resume a held work order
    fn resume(&self, id: Uuid) -> Result<WorkOrder>;

    /// Cancel a work order
    fn cancel(&self, id: Uuid) -> Result<WorkOrder>;

    // Task operations
    /// Add task to work order
    fn add_task(&self, work_order_id: Uuid, task: CreateWorkOrderTask) -> Result<WorkOrderTask>;

    /// Update a task
    fn update_task(&self, task_id: Uuid, task: UpdateWorkOrderTask) -> Result<WorkOrderTask>;

    /// Remove task from work order
    fn remove_task(&self, task_id: Uuid) -> Result<()>;

    /// Get tasks for work order
    fn get_tasks(&self, work_order_id: Uuid) -> Result<Vec<WorkOrderTask>>;

    /// Start a task
    fn start_task(&self, task_id: Uuid) -> Result<WorkOrderTask>;

    /// Complete a task
    fn complete_task(
        &self,
        task_id: Uuid,
        actual_hours: Option<rust_decimal::Decimal>,
    ) -> Result<WorkOrderTask>;

    // Material operations
    /// Add material to work order
    fn add_material(
        &self,
        work_order_id: Uuid,
        material: AddWorkOrderMaterial,
    ) -> Result<WorkOrderMaterial>;

    /// Consume material
    fn consume_material(
        &self,
        material_id: Uuid,
        quantity: rust_decimal::Decimal,
    ) -> Result<WorkOrderMaterial>;

    /// Get materials for work order
    fn get_materials(&self, work_order_id: Uuid) -> Result<Vec<WorkOrderMaterial>>;

    /// Count work orders matching filter
    fn count(&self, filter: WorkOrderFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple work orders - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateWorkOrder>) -> Result<BatchResult<WorkOrder>>;

    /// Create multiple work orders - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateWorkOrder>) -> Result<Vec<WorkOrder>>;

    /// Update multiple work orders - partial success allowed
    fn update_batch(&self, updates: Vec<(Uuid, UpdateWorkOrder)>)
    -> Result<BatchResult<WorkOrder>>;

    /// Update multiple work orders - atomic (all-or-nothing)
    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateWorkOrder)>) -> Result<Vec<WorkOrder>>;

    /// Delete multiple work orders - partial success allowed
    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple work orders - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()>;

    /// Get multiple work orders by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<WorkOrder>>;
}

// ============================================================================
// Quality Control Repository
// ============================================================================

/// Quality Control repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait QualityRepository: Send + Sync {
    // Inspection operations
    /// Create a new inspection
    fn create_inspection(&self, input: CreateInspection) -> Result<Inspection>;

    /// Get inspection by ID
    fn get_inspection(&self, id: Uuid) -> Result<Option<Inspection>>;

    /// Get inspection by number
    fn get_inspection_by_number(&self, number: &str) -> Result<Option<Inspection>>;

    /// Update an inspection
    fn update_inspection(&self, id: Uuid, input: UpdateInspection) -> Result<Inspection>;

    /// List inspections with filter
    fn list_inspections(&self, filter: InspectionFilter) -> Result<Vec<Inspection>>;

    /// Delete an inspection
    fn delete_inspection(&self, id: Uuid) -> Result<()>;

    /// Start an inspection
    fn start_inspection(&self, id: Uuid) -> Result<Inspection>;

    /// Complete an inspection
    fn complete_inspection(&self, id: Uuid) -> Result<Inspection>;

    /// Record inspection result for an item
    fn record_inspection_result(&self, input: RecordInspectionResult) -> Result<InspectionItem>;

    /// Get inspection items
    fn get_inspection_items(&self, inspection_id: Uuid) -> Result<Vec<InspectionItem>>;

    /// Count inspections
    fn count_inspections(&self, filter: InspectionFilter) -> Result<u64>;

    // NCR operations
    /// Create a non-conformance report
    fn create_ncr(&self, input: CreateNonConformance) -> Result<NonConformance>;

    /// Get NCR by ID
    fn get_ncr(&self, id: Uuid) -> Result<Option<NonConformance>>;

    /// Get NCR by number
    fn get_ncr_by_number(&self, number: &str) -> Result<Option<NonConformance>>;

    /// Update an NCR
    fn update_ncr(&self, id: Uuid, input: UpdateNonConformance) -> Result<NonConformance>;

    /// List NCRs with filter
    fn list_ncrs(&self, filter: NonConformanceFilter) -> Result<Vec<NonConformance>>;

    /// Close an NCR
    fn close_ncr(&self, id: Uuid) -> Result<NonConformance>;

    /// Cancel an NCR
    fn cancel_ncr(&self, id: Uuid) -> Result<NonConformance>;

    /// Count NCRs
    fn count_ncrs(&self, filter: NonConformanceFilter) -> Result<u64>;

    // Quality hold operations
    /// Create a quality hold
    fn create_hold(&self, input: CreateQualityHold) -> Result<QualityHold>;

    /// Get hold by ID
    fn get_hold(&self, id: Uuid) -> Result<Option<QualityHold>>;

    /// List holds with filter
    fn list_holds(&self, filter: QualityHoldFilter) -> Result<Vec<QualityHold>>;

    /// Release a hold
    fn release_hold(&self, id: Uuid, input: ReleaseQualityHold) -> Result<QualityHold>;

    /// Get active holds for SKU
    fn get_active_holds_for_sku(&self, sku: &str) -> Result<Vec<QualityHold>>;

    /// Get active holds for lot
    fn get_active_holds_for_lot(&self, lot_number: &str) -> Result<Vec<QualityHold>>;

    /// Count active holds
    fn count_active_holds(&self) -> Result<u64>;

    // Defect code operations
    /// Create a defect code
    fn create_defect_code(&self, input: CreateDefectCode) -> Result<DefectCode>;

    /// Get defect code by code
    fn get_defect_code(&self, code: &str) -> Result<Option<DefectCode>>;

    /// List defect codes
    fn list_defect_codes(&self, category: Option<&str>) -> Result<Vec<DefectCode>>;

    /// Deactivate a defect code
    fn deactivate_defect_code(&self, id: Uuid) -> Result<()>;
}

/// Production batch repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ProductionBatchRepository: Send + Sync {
    /// Create a new production batch.
    fn create(&self, input: CreateProductionBatch) -> Result<ProductionBatch>;

    /// Get a production batch by ID.
    fn get(&self, id: ProductionBatchId) -> Result<Option<ProductionBatch>>;

    /// Update a production batch (partial).
    fn update(
        &self,
        id: ProductionBatchId,
        input: UpdateProductionBatch,
    ) -> Result<ProductionBatch>;

    /// List production batches with filter.
    fn list(&self, filter: ProductionBatchFilter) -> Result<Vec<ProductionBatch>>;

    /// Delete a production batch.
    fn delete(&self, id: ProductionBatchId) -> Result<()>;

    /// Link work orders to a batch. Returns the updated batch.
    fn add_work_orders(
        &self,
        id: ProductionBatchId,
        work_order_ids: Vec<uuid::Uuid>,
    ) -> Result<ProductionBatch>;

    /// Remove a work order from a batch. Returns the updated batch.
    fn remove_work_order(
        &self,
        id: ProductionBatchId,
        work_order_id: uuid::Uuid,
    ) -> Result<ProductionBatch>;
}
