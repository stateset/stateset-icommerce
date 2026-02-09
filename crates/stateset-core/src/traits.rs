//! Repository traits for data access abstraction
//!
//! These traits define the interface for data persistence.
//! Implementations can be SQLite, PostgreSQL, in-memory, etc.

use crate::errors::{BatchResult, Result};
use crate::models::*;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Order repository trait
pub trait OrderRepository {
    /// Create a new order
    fn create(&self, input: CreateOrder) -> Result<Order>;

    /// Get order by ID
    fn get(&self, id: Uuid) -> Result<Option<Order>>;

    /// Get order by order number
    fn get_by_number(&self, order_number: &str) -> Result<Option<Order>>;

    /// Update an order
    fn update(&self, id: Uuid, input: UpdateOrder) -> Result<Order>;

    /// List orders with filter
    fn list(&self, filter: OrderFilter) -> Result<Vec<Order>>;

    /// Delete an order
    fn delete(&self, id: Uuid) -> Result<()>;

    /// Add item to order
    fn add_item(&self, order_id: Uuid, item: CreateOrderItem) -> Result<OrderItem>;

    /// Remove item from order
    fn remove_item(&self, order_id: Uuid, item_id: Uuid) -> Result<()>;

    /// Count orders matching filter
    fn count(&self, filter: OrderFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple orders - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateOrder>) -> Result<BatchResult<Order>>;

    /// Create multiple orders - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateOrder>) -> Result<Vec<Order>>;

    /// Update multiple orders - partial success allowed
    fn update_batch(&self, updates: Vec<(Uuid, UpdateOrder)>) -> Result<BatchResult<Order>>;

    /// Update multiple orders - atomic (all-or-nothing)
    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateOrder)>) -> Result<Vec<Order>>;

    /// Delete multiple orders - partial success allowed
    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple orders - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()>;

    /// Get multiple orders by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Order>>;
}

/// Inventory repository trait
pub trait InventoryRepository {
    /// Create a new inventory item
    fn create_item(&self, input: CreateInventoryItem) -> Result<InventoryItem>;

    /// Get inventory item by ID
    fn get_item(&self, id: i64) -> Result<Option<InventoryItem>>;

    /// Get inventory item by SKU
    fn get_item_by_sku(&self, sku: &str) -> Result<Option<InventoryItem>>;

    /// Get stock level for SKU (aggregated across locations)
    fn get_stock(&self, sku: &str) -> Result<Option<StockLevel>>;

    /// Get balance at specific location
    fn get_balance(&self, item_id: i64, location_id: i32) -> Result<Option<InventoryBalance>>;

    /// Adjust inventory quantity
    fn adjust(&self, input: AdjustInventory) -> Result<InventoryTransaction>;

    /// Reserve inventory
    fn reserve(&self, input: ReserveInventory) -> Result<InventoryReservation>;

    /// Get a reservation by ID
    fn get_reservation(&self, reservation_id: Uuid) -> Result<Option<InventoryReservation>>;

    /// Release reservation
    fn release_reservation(&self, reservation_id: Uuid) -> Result<()>;

    /// Confirm reservation (convert to allocation)
    fn confirm_reservation(&self, reservation_id: Uuid) -> Result<()>;

    /// List reservations by reference (e.g., order id)
    fn list_reservations_by_reference(
        &self,
        reference_type: &str,
        reference_id: &str,
    ) -> Result<Vec<InventoryReservation>>;

    /// List inventory items with filter
    fn list(&self, filter: InventoryFilter) -> Result<Vec<InventoryItem>>;

    /// Get items below reorder point
    fn get_reorder_needed(&self) -> Result<Vec<StockLevel>>;

    /// Record transaction
    fn record_transaction(&self, transaction: InventoryTransaction)
        -> Result<InventoryTransaction>;

    /// Get transaction history
    fn get_transactions(&self, item_id: i64, limit: u32) -> Result<Vec<InventoryTransaction>>;

    // === Batch Operations ===

    /// Create multiple inventory items - partial success allowed
    fn create_item_batch(
        &self,
        inputs: Vec<CreateInventoryItem>,
    ) -> Result<BatchResult<InventoryItem>>;

    /// Create multiple inventory items - atomic (all-or-nothing)
    fn create_item_batch_atomic(
        &self,
        inputs: Vec<CreateInventoryItem>,
    ) -> Result<Vec<InventoryItem>>;

    /// Adjust multiple inventory quantities - partial success allowed
    fn adjust_batch(
        &self,
        adjustments: Vec<AdjustInventory>,
    ) -> Result<BatchResult<InventoryTransaction>>;

    /// Adjust multiple inventory quantities - atomic (all-or-nothing)
    fn adjust_batch_atomic(
        &self,
        adjustments: Vec<AdjustInventory>,
    ) -> Result<Vec<InventoryTransaction>>;

    /// Get multiple inventory items by ID
    fn get_item_batch(&self, ids: Vec<i64>) -> Result<Vec<InventoryItem>>;

    /// Get stock levels for multiple SKUs
    fn get_stock_batch(&self, skus: Vec<String>) -> Result<Vec<StockLevel>>;
}

/// Customer repository trait
pub trait CustomerRepository {
    /// Create a new customer
    fn create(&self, input: CreateCustomer) -> Result<Customer>;

    /// Get customer by ID
    fn get(&self, id: Uuid) -> Result<Option<Customer>>;

    /// Get customer by email
    fn get_by_email(&self, email: &str) -> Result<Option<Customer>>;

    /// Update a customer
    fn update(&self, id: Uuid, input: UpdateCustomer) -> Result<Customer>;

    /// List customers with filter
    fn list(&self, filter: CustomerFilter) -> Result<Vec<Customer>>;

    /// Delete a customer (soft delete)
    fn delete(&self, id: Uuid) -> Result<()>;

    /// Add address for customer
    fn add_address(&self, input: CreateCustomerAddress) -> Result<CustomerAddress>;

    /// Get customer addresses
    fn get_addresses(&self, customer_id: Uuid) -> Result<Vec<CustomerAddress>>;

    /// Update address
    fn update_address(
        &self,
        address_id: Uuid,
        input: CreateCustomerAddress,
    ) -> Result<CustomerAddress>;

    /// Delete address
    fn delete_address(&self, address_id: Uuid) -> Result<()>;

    /// Set default address
    fn set_default_address(
        &self,
        customer_id: Uuid,
        address_id: Uuid,
        address_type: AddressType,
    ) -> Result<()>;

    /// Count customers matching filter
    fn count(&self, filter: CustomerFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple customers - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateCustomer>) -> Result<BatchResult<Customer>>;

    /// Create multiple customers - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateCustomer>) -> Result<Vec<Customer>>;

    /// Update multiple customers - partial success allowed
    fn update_batch(&self, updates: Vec<(Uuid, UpdateCustomer)>) -> Result<BatchResult<Customer>>;

    /// Update multiple customers - atomic (all-or-nothing)
    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateCustomer)>) -> Result<Vec<Customer>>;

    /// Delete multiple customers - partial success allowed
    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple customers - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()>;

    /// Get multiple customers by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Customer>>;
}

/// Product repository trait
pub trait ProductRepository {
    /// Create a new product
    fn create(&self, input: CreateProduct) -> Result<Product>;

    /// Get product by ID
    fn get(&self, id: Uuid) -> Result<Option<Product>>;

    /// Get product by slug
    fn get_by_slug(&self, slug: &str) -> Result<Option<Product>>;

    /// Update a product
    fn update(&self, id: Uuid, input: UpdateProduct) -> Result<Product>;

    /// List products with filter
    fn list(&self, filter: ProductFilter) -> Result<Vec<Product>>;

    /// Delete a product (archive)
    fn delete(&self, id: Uuid) -> Result<()>;

    /// Add variant to product
    fn add_variant(
        &self,
        product_id: Uuid,
        variant: CreateProductVariant,
    ) -> Result<ProductVariant>;

    /// Get variant by ID
    fn get_variant(&self, id: Uuid) -> Result<Option<ProductVariant>>;

    /// Get variant by SKU
    fn get_variant_by_sku(&self, sku: &str) -> Result<Option<ProductVariant>>;

    /// Update variant
    fn update_variant(&self, id: Uuid, variant: CreateProductVariant) -> Result<ProductVariant>;

    /// Delete variant
    fn delete_variant(&self, id: Uuid) -> Result<()>;

    /// Get all variants for product
    fn get_variants(&self, product_id: Uuid) -> Result<Vec<ProductVariant>>;

    /// Count products matching filter
    fn count(&self, filter: ProductFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple products - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateProduct>) -> Result<BatchResult<Product>>;

    /// Create multiple products - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateProduct>) -> Result<Vec<Product>>;

    /// Update multiple products - partial success allowed
    fn update_batch(&self, updates: Vec<(Uuid, UpdateProduct)>) -> Result<BatchResult<Product>>;

    /// Update multiple products - atomic (all-or-nothing)
    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateProduct)>) -> Result<Vec<Product>>;

    /// Delete multiple products - partial success allowed
    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple products - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()>;

    /// Get multiple products by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Product>>;
}

/// Return repository trait
pub trait ReturnRepository {
    /// Create a new return
    fn create(&self, input: CreateReturn) -> Result<Return>;

    /// Get return by ID
    fn get(&self, id: Uuid) -> Result<Option<Return>>;

    /// Update a return
    fn update(&self, id: Uuid, input: UpdateReturn) -> Result<Return>;

    /// List returns with filter
    fn list(&self, filter: ReturnFilter) -> Result<Vec<Return>>;

    /// Approve a return
    fn approve(&self, id: Uuid) -> Result<Return>;

    /// Reject a return
    fn reject(&self, id: Uuid, reason: &str) -> Result<Return>;

    /// Complete a return
    fn complete(&self, id: Uuid) -> Result<Return>;

    /// Cancel a return
    fn cancel(&self, id: Uuid) -> Result<Return>;

    /// Count returns matching filter
    fn count(&self, filter: ReturnFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple returns - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateReturn>) -> Result<BatchResult<Return>>;

    /// Create multiple returns - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateReturn>) -> Result<Vec<Return>>;

    /// Update multiple returns - partial success allowed
    fn update_batch(&self, updates: Vec<(Uuid, UpdateReturn)>) -> Result<BatchResult<Return>>;

    /// Update multiple returns - atomic (all-or-nothing)
    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateReturn)>) -> Result<Vec<Return>>;

    /// Delete multiple returns - partial success allowed
    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple returns - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()>;

    /// Get multiple returns by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Return>>;
}

/// Event handler trait for domain events
pub trait EventHandler {
    /// Handle a commerce event
    fn handle(&self, event: &crate::events::CommerceEvent) -> Result<()>;
}

/// Bill of Materials repository trait
pub trait BomRepository {
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
pub trait WorkOrderRepository {
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

    /// Start a work order (transitions from planned to in_progress)
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

/// Shipment repository trait
pub trait ShipmentRepository {
    /// Create a new shipment
    fn create(&self, input: CreateShipment) -> Result<Shipment>;

    /// Get shipment by ID
    fn get(&self, id: Uuid) -> Result<Option<Shipment>>;

    /// Get shipment by shipment number
    fn get_by_number(&self, shipment_number: &str) -> Result<Option<Shipment>>;

    /// Get shipment by tracking number
    fn get_by_tracking(&self, tracking_number: &str) -> Result<Option<Shipment>>;

    /// Update a shipment
    fn update(&self, id: Uuid, input: UpdateShipment) -> Result<Shipment>;

    /// List shipments with filter
    fn list(&self, filter: ShipmentFilter) -> Result<Vec<Shipment>>;

    /// Get shipments for an order
    fn for_order(&self, order_id: Uuid) -> Result<Vec<Shipment>>;

    /// Delete a shipment (cancel if not shipped)
    fn delete(&self, id: Uuid) -> Result<()>;

    // Status transitions
    /// Mark shipment as processing
    fn mark_processing(&self, id: Uuid) -> Result<Shipment>;

    /// Mark shipment as ready to ship
    fn mark_ready(&self, id: Uuid) -> Result<Shipment>;

    /// Mark shipment as shipped with tracking number
    fn ship(&self, id: Uuid, tracking_number: Option<String>) -> Result<Shipment>;

    /// Mark shipment as in transit
    fn mark_in_transit(&self, id: Uuid) -> Result<Shipment>;

    /// Mark shipment as out for delivery
    fn mark_out_for_delivery(&self, id: Uuid) -> Result<Shipment>;

    /// Mark shipment as delivered
    fn mark_delivered(&self, id: Uuid) -> Result<Shipment>;

    /// Mark shipment as failed delivery
    fn mark_failed(&self, id: Uuid) -> Result<Shipment>;

    /// Put shipment on hold
    fn hold(&self, id: Uuid) -> Result<Shipment>;

    /// Cancel shipment
    fn cancel(&self, id: Uuid) -> Result<Shipment>;

    // Item operations
    /// Add item to shipment
    fn add_item(&self, shipment_id: Uuid, item: CreateShipmentItem) -> Result<ShipmentItem>;

    /// Remove item from shipment
    fn remove_item(&self, item_id: Uuid) -> Result<()>;

    /// Get items in shipment
    fn get_items(&self, shipment_id: Uuid) -> Result<Vec<ShipmentItem>>;

    // Event/tracking operations
    /// Add tracking event
    fn add_event(&self, shipment_id: Uuid, event: AddShipmentEvent) -> Result<ShipmentEvent>;

    /// Get tracking events for shipment
    fn get_events(&self, shipment_id: Uuid) -> Result<Vec<ShipmentEvent>>;

    /// Count shipments matching filter
    fn count(&self, filter: ShipmentFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple shipments - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateShipment>) -> Result<BatchResult<Shipment>>;

    /// Create multiple shipments - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateShipment>) -> Result<Vec<Shipment>>;

    /// Update multiple shipments - partial success allowed
    fn update_batch(&self, updates: Vec<(Uuid, UpdateShipment)>) -> Result<BatchResult<Shipment>>;

    /// Update multiple shipments - atomic (all-or-nothing)
    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateShipment)>) -> Result<Vec<Shipment>>;

    /// Delete multiple shipments - partial success allowed
    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple shipments - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()>;

    /// Get multiple shipments by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Shipment>>;
}

/// Payment repository trait
pub trait PaymentRepository {
    /// Create a new payment
    fn create(&self, input: CreatePayment) -> Result<Payment>;

    /// Get payment by ID
    fn get(&self, id: Uuid) -> Result<Option<Payment>>;

    /// Get payment by payment number
    fn get_by_number(&self, payment_number: &str) -> Result<Option<Payment>>;

    /// Get payment by external ID (e.g., Stripe payment intent)
    fn get_by_external_id(&self, external_id: &str) -> Result<Option<Payment>>;

    /// Update a payment
    fn update(&self, id: Uuid, input: UpdatePayment) -> Result<Payment>;

    /// List payments with filter
    fn list(&self, filter: PaymentFilter) -> Result<Vec<Payment>>;

    /// Get payments for an order
    fn for_order(&self, order_id: Uuid) -> Result<Vec<Payment>>;

    /// Get payments for an invoice
    fn for_invoice(&self, invoice_id: Uuid) -> Result<Vec<Payment>>;

    // Status transitions
    /// Mark payment as processing
    fn mark_processing(&self, id: Uuid) -> Result<Payment>;

    /// Mark payment as completed (paid)
    fn mark_completed(&self, id: Uuid) -> Result<Payment>;

    /// Mark payment as failed
    fn mark_failed(&self, id: Uuid, reason: &str, code: Option<&str>) -> Result<Payment>;

    /// Cancel payment
    fn cancel(&self, id: Uuid) -> Result<Payment>;

    // Refund operations
    /// Create a refund for a payment
    fn create_refund(&self, input: CreateRefund) -> Result<Refund>;

    /// Get refund by ID
    fn get_refund(&self, id: Uuid) -> Result<Option<Refund>>;

    /// Get refunds for a payment
    fn get_refunds(&self, payment_id: Uuid) -> Result<Vec<Refund>>;

    /// Process refund (mark as completed)
    fn complete_refund(&self, id: Uuid) -> Result<Refund>;

    /// Fail refund
    fn fail_refund(&self, id: Uuid, reason: &str) -> Result<Refund>;

    // Payment method operations
    /// Create a payment method for a customer
    fn create_payment_method(&self, input: CreatePaymentMethod) -> Result<PaymentMethod>;

    /// Get payment methods for a customer
    fn get_payment_methods(&self, customer_id: Uuid) -> Result<Vec<PaymentMethod>>;

    /// Delete a payment method
    fn delete_payment_method(&self, id: Uuid) -> Result<()>;

    /// Set default payment method
    fn set_default_payment_method(&self, customer_id: Uuid, method_id: Uuid) -> Result<()>;

    /// Count payments matching filter
    fn count(&self, filter: PaymentFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple payments - partial success allowed
    fn create_batch(&self, inputs: Vec<CreatePayment>) -> Result<BatchResult<Payment>>;

    /// Create multiple payments - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreatePayment>) -> Result<Vec<Payment>>;

    /// Update multiple payments - partial success allowed
    fn update_batch(&self, updates: Vec<(Uuid, UpdatePayment)>) -> Result<BatchResult<Payment>>;

    /// Update multiple payments - atomic (all-or-nothing)
    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdatePayment)>) -> Result<Vec<Payment>>;

    /// Delete multiple payments - partial success allowed
    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple payments - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()>;

    /// Get multiple payments by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Payment>>;
}

/// Warranty repository trait
pub trait WarrantyRepository {
    /// Create a new warranty
    fn create(&self, input: CreateWarranty) -> Result<Warranty>;

    /// Get warranty by ID
    fn get(&self, id: Uuid) -> Result<Option<Warranty>>;

    /// Get warranty by warranty number
    fn get_by_number(&self, warranty_number: &str) -> Result<Option<Warranty>>;

    /// Get warranty by serial number
    fn get_by_serial(&self, serial_number: &str) -> Result<Option<Warranty>>;

    /// Update a warranty
    fn update(&self, id: Uuid, input: UpdateWarranty) -> Result<Warranty>;

    /// List warranties with filter
    fn list(&self, filter: WarrantyFilter) -> Result<Vec<Warranty>>;

    /// Get warranties for a customer
    fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Warranty>>;

    /// Get warranties for an order
    fn for_order(&self, order_id: Uuid) -> Result<Vec<Warranty>>;

    // Status transitions
    /// Void a warranty
    fn void(&self, id: Uuid) -> Result<Warranty>;

    /// Expire a warranty
    fn expire(&self, id: Uuid) -> Result<Warranty>;

    /// Transfer warranty to new owner
    fn transfer(&self, id: Uuid, new_customer_id: Uuid) -> Result<Warranty>;

    // Claim operations
    /// Create a warranty claim
    fn create_claim(&self, input: CreateWarrantyClaim) -> Result<WarrantyClaim>;

    /// Get claim by ID
    fn get_claim(&self, id: Uuid) -> Result<Option<WarrantyClaim>>;

    /// Get claim by claim number
    fn get_claim_by_number(&self, claim_number: &str) -> Result<Option<WarrantyClaim>>;

    /// Update a claim
    fn update_claim(&self, id: Uuid, input: UpdateWarrantyClaim) -> Result<WarrantyClaim>;

    /// List claims with filter
    fn list_claims(&self, filter: WarrantyClaimFilter) -> Result<Vec<WarrantyClaim>>;

    /// Get claims for a warranty
    fn get_claims(&self, warranty_id: Uuid) -> Result<Vec<WarrantyClaim>>;

    // Claim status transitions
    /// Approve a claim
    fn approve_claim(&self, id: Uuid) -> Result<WarrantyClaim>;

    /// Deny a claim
    fn deny_claim(&self, id: Uuid, reason: &str) -> Result<WarrantyClaim>;

    /// Complete a claim
    fn complete_claim(&self, id: Uuid, resolution: ClaimResolution) -> Result<WarrantyClaim>;

    /// Cancel a claim
    fn cancel_claim(&self, id: Uuid) -> Result<WarrantyClaim>;

    /// Count warranties matching filter
    fn count(&self, filter: WarrantyFilter) -> Result<u64>;

    /// Count claims matching filter
    fn count_claims(&self, filter: WarrantyClaimFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple warranties - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateWarranty>) -> Result<BatchResult<Warranty>>;

    /// Create multiple warranties - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateWarranty>) -> Result<Vec<Warranty>>;

    /// Update multiple warranties - partial success allowed
    fn update_batch(&self, updates: Vec<(Uuid, UpdateWarranty)>) -> Result<BatchResult<Warranty>>;

    /// Update multiple warranties - atomic (all-or-nothing)
    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateWarranty)>) -> Result<Vec<Warranty>>;

    /// Delete multiple warranties - partial success allowed
    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple warranties - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()>;

    /// Get multiple warranties by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Warranty>>;
}

/// Purchase Order repository trait
pub trait PurchaseOrderRepository {
    // Supplier operations
    /// Create a new supplier
    fn create_supplier(&self, input: CreateSupplier) -> Result<Supplier>;

    /// Get supplier by ID
    fn get_supplier(&self, id: Uuid) -> Result<Option<Supplier>>;

    /// Get supplier by code
    fn get_supplier_by_code(&self, code: &str) -> Result<Option<Supplier>>;

    /// Update a supplier
    fn update_supplier(&self, id: Uuid, input: UpdateSupplier) -> Result<Supplier>;

    /// List suppliers with filter
    fn list_suppliers(&self, filter: SupplierFilter) -> Result<Vec<Supplier>>;

    /// Delete supplier (deactivate)
    fn delete_supplier(&self, id: Uuid) -> Result<()>;

    // Purchase Order operations
    /// Create a new purchase order
    fn create(&self, input: CreatePurchaseOrder) -> Result<PurchaseOrder>;

    /// Get purchase order by ID
    fn get(&self, id: Uuid) -> Result<Option<PurchaseOrder>>;

    /// Get purchase order by PO number
    fn get_by_number(&self, po_number: &str) -> Result<Option<PurchaseOrder>>;

    /// Update a purchase order
    fn update(&self, id: Uuid, input: UpdatePurchaseOrder) -> Result<PurchaseOrder>;

    /// List purchase orders with filter
    fn list(&self, filter: PurchaseOrderFilter) -> Result<Vec<PurchaseOrder>>;

    /// Get purchase orders for a supplier
    fn for_supplier(&self, supplier_id: Uuid) -> Result<Vec<PurchaseOrder>>;

    /// Delete a purchase order (only if draft)
    fn delete(&self, id: Uuid) -> Result<()>;

    // Status transitions
    /// Submit for approval
    fn submit_for_approval(&self, id: Uuid) -> Result<PurchaseOrder>;

    /// Approve purchase order
    fn approve(&self, id: Uuid, approved_by: &str) -> Result<PurchaseOrder>;

    /// Send to supplier
    fn send(&self, id: Uuid) -> Result<PurchaseOrder>;

    /// Mark as acknowledged by supplier
    fn acknowledge(&self, id: Uuid, supplier_reference: Option<&str>) -> Result<PurchaseOrder>;

    /// Put on hold
    fn hold(&self, id: Uuid) -> Result<PurchaseOrder>;

    /// Cancel purchase order
    fn cancel(&self, id: Uuid) -> Result<PurchaseOrder>;

    /// Receive items on a purchase order
    fn receive(&self, id: Uuid, items: ReceivePurchaseOrderItems) -> Result<PurchaseOrder>;

    /// Complete/close purchase order
    fn complete(&self, id: Uuid) -> Result<PurchaseOrder>;

    // Item operations
    /// Add item to purchase order
    fn add_item(&self, po_id: Uuid, item: CreatePurchaseOrderItem) -> Result<PurchaseOrderItem>;

    /// Update a PO item
    fn update_item(
        &self,
        item_id: Uuid,
        item: CreatePurchaseOrderItem,
    ) -> Result<PurchaseOrderItem>;

    /// Remove item from purchase order
    fn remove_item(&self, item_id: Uuid) -> Result<()>;

    /// Get items for purchase order
    fn get_items(&self, po_id: Uuid) -> Result<Vec<PurchaseOrderItem>>;

    /// Count purchase orders matching filter
    fn count(&self, filter: PurchaseOrderFilter) -> Result<u64>;

    /// Count suppliers matching filter
    fn count_suppliers(&self, filter: SupplierFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple purchase orders - partial success allowed
    fn create_batch(&self, inputs: Vec<CreatePurchaseOrder>) -> Result<BatchResult<PurchaseOrder>>;

    /// Create multiple purchase orders - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreatePurchaseOrder>) -> Result<Vec<PurchaseOrder>>;

    /// Update multiple purchase orders - partial success allowed
    fn update_batch(
        &self,
        updates: Vec<(Uuid, UpdatePurchaseOrder)>,
    ) -> Result<BatchResult<PurchaseOrder>>;

    /// Update multiple purchase orders - atomic (all-or-nothing)
    fn update_batch_atomic(
        &self,
        updates: Vec<(Uuid, UpdatePurchaseOrder)>,
    ) -> Result<Vec<PurchaseOrder>>;

    /// Delete multiple purchase orders - partial success allowed
    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple purchase orders - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()>;

    /// Get multiple purchase orders by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<PurchaseOrder>>;
}

/// Invoice repository trait
pub trait InvoiceRepository {
    /// Create a new invoice
    fn create(&self, input: CreateInvoice) -> Result<Invoice>;

    /// Get invoice by ID
    fn get(&self, id: Uuid) -> Result<Option<Invoice>>;

    /// Get invoice by invoice number
    fn get_by_number(&self, invoice_number: &str) -> Result<Option<Invoice>>;

    /// Update an invoice
    fn update(&self, id: Uuid, input: UpdateInvoice) -> Result<Invoice>;

    /// List invoices with filter
    fn list(&self, filter: InvoiceFilter) -> Result<Vec<Invoice>>;

    /// Get invoices for a customer
    fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Invoice>>;

    /// Get invoices for an order
    fn for_order(&self, order_id: Uuid) -> Result<Vec<Invoice>>;

    /// Delete an invoice (only if draft)
    fn delete(&self, id: Uuid) -> Result<()>;

    // Status transitions
    /// Send invoice to customer
    fn send(&self, id: Uuid) -> Result<Invoice>;

    /// Mark invoice as viewed
    fn mark_viewed(&self, id: Uuid) -> Result<Invoice>;

    /// Record a payment on the invoice
    fn record_payment(&self, id: Uuid, payment: RecordInvoicePayment) -> Result<Invoice>;

    /// Void an invoice
    fn void(&self, id: Uuid) -> Result<Invoice>;

    /// Write off an invoice as uncollectible
    fn write_off(&self, id: Uuid) -> Result<Invoice>;

    /// Mark invoice as disputed
    fn dispute(&self, id: Uuid) -> Result<Invoice>;

    // Item operations
    /// Add item to invoice
    fn add_item(&self, invoice_id: Uuid, item: CreateInvoiceItem) -> Result<InvoiceItem>;

    /// Update an invoice item
    fn update_item(&self, item_id: Uuid, item: CreateInvoiceItem) -> Result<InvoiceItem>;

    /// Remove item from invoice
    fn remove_item(&self, item_id: Uuid) -> Result<()>;

    /// Get items for invoice
    fn get_items(&self, invoice_id: Uuid) -> Result<Vec<InvoiceItem>>;

    /// Recalculate invoice totals
    fn recalculate(&self, id: Uuid) -> Result<Invoice>;

    /// Get overdue invoices
    fn get_overdue(&self) -> Result<Vec<Invoice>>;

    /// Count invoices matching filter
    fn count(&self, filter: InvoiceFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple invoices - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateInvoice>) -> Result<BatchResult<Invoice>>;

    /// Create multiple invoices - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateInvoice>) -> Result<Vec<Invoice>>;

    /// Update multiple invoices - partial success allowed
    fn update_batch(&self, updates: Vec<(Uuid, UpdateInvoice)>) -> Result<BatchResult<Invoice>>;

    /// Update multiple invoices - atomic (all-or-nothing)
    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateInvoice)>) -> Result<Vec<Invoice>>;

    /// Delete multiple invoices - partial success allowed
    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple invoices - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()>;

    /// Get multiple invoices by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Invoice>>;
}

/// Cart/Checkout repository trait
pub trait CartRepository {
    /// Create a new cart/checkout session
    fn create(&self, input: CreateCart) -> Result<Cart>;

    /// Get cart by ID
    fn get(&self, id: Uuid) -> Result<Option<Cart>>;

    /// Get cart by cart number
    fn get_by_number(&self, cart_number: &str) -> Result<Option<Cart>>;

    /// Update a cart
    fn update(&self, id: Uuid, input: UpdateCart) -> Result<Cart>;

    /// List carts with filter
    fn list(&self, filter: CartFilter) -> Result<Vec<Cart>>;

    /// Get carts for a customer
    fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Cart>>;

    /// Delete a cart (or mark as cancelled)
    fn delete(&self, id: Uuid) -> Result<()>;

    // Item operations
    /// Add item to cart
    fn add_item(&self, cart_id: Uuid, item: AddCartItem) -> Result<CartItem>;

    /// Update a cart item (quantity, etc)
    fn update_item(&self, item_id: Uuid, input: UpdateCartItem) -> Result<CartItem>;

    /// Remove item from cart
    fn remove_item(&self, item_id: Uuid) -> Result<()>;

    /// Get items for a cart
    fn get_items(&self, cart_id: Uuid) -> Result<Vec<CartItem>>;

    /// Clear all items from cart
    fn clear_items(&self, cart_id: Uuid) -> Result<()>;

    // Address operations
    /// Set shipping address
    fn set_shipping_address(&self, id: Uuid, address: CartAddress) -> Result<Cart>;

    /// Set billing address
    fn set_billing_address(&self, id: Uuid, address: CartAddress) -> Result<Cart>;

    // Shipping operations
    /// Set shipping method
    fn set_shipping(&self, id: Uuid, shipping: SetCartShipping) -> Result<Cart>;

    /// Get available shipping rates for cart
    fn get_shipping_rates(&self, id: Uuid) -> Result<Vec<ShippingRate>>;

    // Payment operations
    /// Set payment method/token
    fn set_payment(&self, id: Uuid, payment: SetCartPayment) -> Result<Cart>;

    /// Set x402 payment method (stablecoin)
    fn set_x402_payment(&self, id: Uuid, payment: SetCartX402Payment) -> Result<Cart>;

    /// Complete checkout with x402 payment
    /// Returns PaymentRequired if no intent exists, IntentCreated if awaiting signature,
    /// AwaitingSettlement if signed but not settled, or Completed if settled
    fn complete_with_x402(&self, id: Uuid, payee_address: &str) -> Result<X402CheckoutResult>;

    // Discount operations
    /// Apply coupon/discount code
    fn apply_discount(&self, id: Uuid, coupon_code: &str) -> Result<Cart>;

    /// Remove discount
    fn remove_discount(&self, id: Uuid) -> Result<Cart>;

    // Status transitions
    /// Mark cart as ready for payment (validates all requirements met)
    fn mark_ready_for_payment(&self, id: Uuid) -> Result<Cart>;

    /// Begin checkout/payment process
    fn begin_checkout(&self, id: Uuid) -> Result<Cart>;

    /// Complete checkout (creates order, returns checkout result)
    fn complete(&self, id: Uuid) -> Result<CheckoutResult>;

    /// Cancel a cart
    fn cancel(&self, id: Uuid) -> Result<Cart>;

    /// Mark cart as abandoned
    fn abandon(&self, id: Uuid) -> Result<Cart>;

    /// Expire a cart
    fn expire(&self, id: Uuid) -> Result<Cart>;

    // Inventory operations
    /// Reserve inventory for cart items
    fn reserve_inventory(&self, id: Uuid) -> Result<Cart>;

    /// Release inventory reservations
    fn release_inventory(&self, id: Uuid) -> Result<Cart>;

    // Totals
    /// Recalculate cart totals
    fn recalculate(&self, id: Uuid) -> Result<Cart>;

    /// Set tax amount
    fn set_tax(&self, id: Uuid, tax_amount: rust_decimal::Decimal) -> Result<Cart>;

    // Queries
    /// Get abandoned carts (for recovery campaigns)
    fn get_abandoned(&self) -> Result<Vec<Cart>>;

    /// Get expired carts
    fn get_expired(&self) -> Result<Vec<Cart>>;

    /// Count carts matching filter
    fn count(&self, filter: CartFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple carts - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateCart>) -> Result<BatchResult<Cart>>;

    /// Create multiple carts - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateCart>) -> Result<Vec<Cart>>;

    /// Update multiple carts - partial success allowed
    fn update_batch(&self, updates: Vec<(Uuid, UpdateCart)>) -> Result<BatchResult<Cart>>;

    /// Update multiple carts - atomic (all-or-nothing)
    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateCart)>) -> Result<Vec<Cart>>;

    /// Delete multiple carts - partial success allowed
    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple carts - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()>;

    /// Get multiple carts by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Cart>>;
}

/// Analytics repository trait
pub trait AnalyticsRepository {
    // Sales analytics
    /// Get sales summary for a time period
    fn get_sales_summary(&self, query: AnalyticsQuery) -> Result<SalesSummary>;

    /// Get revenue broken down by time periods
    fn get_revenue_by_period(&self, query: AnalyticsQuery) -> Result<Vec<RevenueByPeriod>>;

    /// Get top selling products
    fn get_top_products(&self, query: AnalyticsQuery) -> Result<Vec<TopProduct>>;

    /// Get product performance with period comparison
    fn get_product_performance(&self, query: AnalyticsQuery) -> Result<Vec<ProductPerformance>>;

    // Customer analytics
    /// Get customer metrics
    fn get_customer_metrics(&self, query: AnalyticsQuery) -> Result<CustomerMetrics>;

    /// Get top customers by spend
    fn get_top_customers(&self, query: AnalyticsQuery) -> Result<Vec<TopCustomer>>;

    // Inventory analytics
    /// Get inventory health summary
    fn get_inventory_health(&self) -> Result<InventoryHealth>;

    /// Get low stock items
    fn get_low_stock_items(
        &self,
        threshold: Option<rust_decimal::Decimal>,
    ) -> Result<Vec<LowStockItem>>;

    /// Get inventory movement summary
    fn get_inventory_movement(&self, query: AnalyticsQuery) -> Result<Vec<InventoryMovement>>;

    // Order analytics
    /// Get order status breakdown
    fn get_order_status_breakdown(&self, query: AnalyticsQuery) -> Result<OrderStatusBreakdown>;

    /// Get fulfillment metrics
    fn get_fulfillment_metrics(&self, query: AnalyticsQuery) -> Result<FulfillmentMetrics>;

    // Return analytics
    /// Get return metrics
    fn get_return_metrics(&self, query: AnalyticsQuery) -> Result<ReturnMetrics>;

    // Forecasting
    /// Get demand forecast for SKUs
    fn get_demand_forecast(
        &self,
        skus: Option<Vec<String>>,
        days_ahead: u32,
    ) -> Result<Vec<DemandForecast>>;

    /// Get revenue forecast
    fn get_revenue_forecast(
        &self,
        periods_ahead: u32,
        granularity: TimeGranularity,
    ) -> Result<Vec<RevenueForecast>>;

    // === Batch Operations ===

    /// Get multiple sales summaries for different queries
    fn get_sales_summary_batch(&self, queries: Vec<AnalyticsQuery>) -> Result<Vec<SalesSummary>>;
}

/// Currency and exchange rate repository trait
pub trait CurrencyRepository {
    /// Get current exchange rate between two currencies
    fn get_rate(&self, from: Currency, to: Currency) -> Result<Option<ExchangeRate>>;

    /// Get all exchange rates for a base currency
    fn get_rates_for(&self, base: Currency) -> Result<Vec<ExchangeRate>>;

    /// List all exchange rates with optional filter
    fn list_rates(&self, filter: ExchangeRateFilter) -> Result<Vec<ExchangeRate>>;

    /// Set an exchange rate
    fn set_rate(&self, input: SetExchangeRate) -> Result<ExchangeRate>;

    /// Set multiple exchange rates at once
    fn set_rates(&self, rates: Vec<SetExchangeRate>) -> Result<Vec<ExchangeRate>>;

    /// Delete an exchange rate
    fn delete_rate(&self, id: Uuid) -> Result<()>;

    /// Convert money between currencies
    fn convert(&self, input: ConvertCurrency) -> Result<ConversionResult>;

    /// Get store currency settings
    fn get_settings(&self) -> Result<StoreCurrencySettings>;

    /// Update store currency settings
    fn update_settings(&self, settings: StoreCurrencySettings) -> Result<StoreCurrencySettings>;

    // === Batch Operations ===

    /// Set multiple exchange rates - atomic (all-or-nothing)
    /// Note: set_rates already exists as a partial-success batch operation
    fn set_rates_atomic(&self, rates: Vec<SetExchangeRate>) -> Result<Vec<ExchangeRate>>;

    /// Delete multiple exchange rates - partial success allowed
    fn delete_rates_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple exchange rates - atomic (all-or-nothing)
    fn delete_rates_atomic(&self, ids: Vec<Uuid>) -> Result<()>;

    /// Get multiple exchange rates by currency pairs
    fn get_rates_batch(&self, pairs: Vec<(Currency, Currency)>) -> Result<Vec<ExchangeRate>>;
}

/// Tax repository trait
pub trait TaxRepository {
    /// Create a tax jurisdiction
    fn create_jurisdiction(&self, input: CreateTaxJurisdiction) -> Result<TaxJurisdiction>;
    /// Get a tax jurisdiction by ID
    fn get_jurisdiction(&self, id: Uuid) -> Result<Option<TaxJurisdiction>>;
    /// Get a tax jurisdiction by code
    fn get_jurisdiction_by_code(&self, code: &str) -> Result<Option<TaxJurisdiction>>;
    /// List tax jurisdictions matching a filter
    fn list_jurisdictions(&self, filter: TaxJurisdictionFilter) -> Result<Vec<TaxJurisdiction>>;

    /// Create a tax rate
    fn create_rate(&self, input: CreateTaxRate) -> Result<TaxRate>;
    /// Get a tax rate by ID
    fn get_rate(&self, id: Uuid) -> Result<Option<TaxRate>>;
    /// List tax rates matching a filter
    fn list_rates(&self, filter: TaxRateFilter) -> Result<Vec<TaxRate>>;
    /// Get applicable tax rates for an address and category on a date
    fn get_rates_for_address(
        &self,
        address: &TaxAddress,
        category: ProductTaxCategory,
        date: chrono::NaiveDate,
    ) -> Result<Vec<TaxRate>>;

    /// Create a tax exemption
    fn create_exemption(&self, input: CreateTaxExemption) -> Result<TaxExemption>;
    /// Get a tax exemption by ID
    fn get_exemption(&self, id: Uuid) -> Result<Option<TaxExemption>>;
    /// Get all exemptions for a customer
    fn get_customer_exemptions(&self, customer_id: Uuid) -> Result<Vec<TaxExemption>>;

    /// Get tax settings
    fn get_settings(&self) -> Result<TaxSettings>;
    /// Update tax settings
    fn update_settings(&self, settings: TaxSettings) -> Result<TaxSettings>;

    /// Calculate tax for a request
    fn calculate_tax(&self, request: TaxCalculationRequest) -> Result<TaxCalculationResult>;
    /// Persist a tax calculation for audit/reporting
    fn save_calculation(
        &self,
        result: &TaxCalculationResult,
        order_id: Option<Uuid>,
        cart_id: Option<Uuid>,
        customer_id: Option<Uuid>,
        address: &TaxAddress,
        currency: &str,
    ) -> Result<()>;
}

/// Promotions repository trait
pub trait PromotionRepository {
    /// Create a promotion
    fn create(&self, input: CreatePromotion) -> Result<Promotion>;
    /// Get a promotion by ID
    fn get(&self, id: Uuid) -> Result<Option<Promotion>>;
    /// Get a promotion by code
    fn get_by_code(&self, code: &str) -> Result<Option<Promotion>>;
    /// List promotions matching a filter
    fn list(&self, filter: PromotionFilter) -> Result<Vec<Promotion>>;
    /// Update a promotion
    fn update(&self, id: Uuid, input: UpdatePromotion) -> Result<Promotion>;
    /// Delete a promotion
    fn delete(&self, id: Uuid) -> Result<()>;
    /// Activate a promotion
    fn activate(&self, id: Uuid) -> Result<Promotion>;
    /// Deactivate a promotion
    fn deactivate(&self, id: Uuid) -> Result<Promotion>;

    /// Create a coupon code
    fn create_coupon(&self, input: CreateCouponCode) -> Result<CouponCode>;
    /// Get a coupon by ID
    fn get_coupon(&self, id: Uuid) -> Result<Option<CouponCode>>;
    /// Get a coupon by code
    fn get_coupon_by_code(&self, code: &str) -> Result<Option<CouponCode>>;
    /// List coupons matching a filter
    fn list_coupons(&self, filter: CouponFilter) -> Result<Vec<CouponCode>>;

    /// Apply promotions to a cart or order snapshot
    fn apply_promotions(&self, request: ApplyPromotionsRequest) -> Result<ApplyPromotionsResult>;
    /// Record a promotion usage event
    #[allow(clippy::too_many_arguments)]
    fn record_usage(
        &self,
        promotion_id: Uuid,
        coupon_id: Option<Uuid>,
        customer_id: Option<Uuid>,
        order_id: Option<Uuid>,
        cart_id: Option<Uuid>,
        discount_amount: rust_decimal::Decimal,
        currency: &str,
    ) -> Result<PromotionUsage>;
}

/// Subscriptions repository trait
pub trait SubscriptionRepository {
    /// Create a subscription plan
    fn create_plan(&self, input: CreateSubscriptionPlan) -> Result<SubscriptionPlan>;
    /// Get a subscription plan by ID
    fn get_plan(&self, id: Uuid) -> Result<Option<SubscriptionPlan>>;
    /// Get a subscription plan by code
    fn get_plan_by_code(&self, code: &str) -> Result<Option<SubscriptionPlan>>;
    /// List subscription plans matching a filter
    fn list_plans(&self, filter: SubscriptionPlanFilter) -> Result<Vec<SubscriptionPlan>>;
    /// Update a subscription plan
    fn update_plan(&self, id: Uuid, input: UpdateSubscriptionPlan) -> Result<SubscriptionPlan>;
    /// Activate a subscription plan
    fn activate_plan(&self, id: Uuid) -> Result<SubscriptionPlan>;
    /// Archive a subscription plan
    fn archive_plan(&self, id: Uuid) -> Result<SubscriptionPlan>;

    /// Create a subscription
    fn create_subscription(&self, input: CreateSubscription) -> Result<Subscription>;
    /// Get a subscription by ID
    fn get_subscription(&self, id: Uuid) -> Result<Option<Subscription>>;
    /// Get a subscription by number
    fn get_subscription_by_number(&self, number: &str) -> Result<Option<Subscription>>;
    /// List subscriptions matching a filter
    fn list_subscriptions(&self, filter: SubscriptionFilter) -> Result<Vec<Subscription>>;
    /// Update a subscription
    fn update_subscription(&self, id: Uuid, input: UpdateSubscription) -> Result<Subscription>;
    /// Cancel a subscription
    fn cancel_subscription(&self, id: Uuid, input: CancelSubscription) -> Result<Subscription>;
    /// Pause a subscription
    fn pause_subscription(&self, id: Uuid, input: PauseSubscription) -> Result<Subscription>;
    /// Resume a paused subscription
    fn resume_subscription(&self, id: Uuid) -> Result<Subscription>;

    /// Create a billing cycle
    fn create_billing_cycle(&self, input: CreateBillingCycle) -> Result<BillingCycle>;
    /// Get a billing cycle by ID
    fn get_billing_cycle(&self, id: Uuid) -> Result<Option<BillingCycle>>;
    /// List billing cycles matching a filter
    fn list_billing_cycles(&self, filter: BillingCycleFilter) -> Result<Vec<BillingCycle>>;
    /// Update the status of a billing cycle
    fn update_billing_cycle_status(
        &self,
        id: Uuid,
        status: BillingCycleStatus,
    ) -> Result<BillingCycle>;
    /// Skip a billing cycle
    fn skip_billing_cycle(&self, id: Uuid, input: SkipBillingCycle) -> Result<Subscription>;

    /// Record a subscription event
    fn record_event(
        &self,
        subscription_id: Uuid,
        event_type: SubscriptionEventType,
        notes: Option<String>,
    ) -> Result<SubscriptionEvent>;
    /// Get all events for a subscription
    fn get_subscription_events(&self, subscription_id: Uuid) -> Result<Vec<SubscriptionEvent>>;
}

/// Optional: Transaction support trait
pub trait Transactional {
    /// Begin a transaction
    fn begin_transaction(&self) -> Result<()>;

    /// Commit the current transaction
    fn commit(&self) -> Result<()>;

    /// Rollback the current transaction
    fn rollback(&self) -> Result<()>;
}

// ============================================================================
// Quality Control Repository
// ============================================================================

/// Quality Control repository trait
pub trait QualityRepository {
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

// ============================================================================
// Lot Repository
// ============================================================================

/// Lot/Batch tracking repository trait
pub trait LotRepository {
    /// Create a new lot
    fn create(&self, input: CreateLot) -> Result<Lot>;

    /// Get lot by ID
    fn get(&self, id: Uuid) -> Result<Option<Lot>>;

    /// Get lot by lot number
    fn get_by_number(&self, lot_number: &str) -> Result<Option<Lot>>;

    /// Update a lot
    fn update(&self, id: Uuid, input: UpdateLot) -> Result<Lot>;

    /// List lots with filter
    fn list(&self, filter: LotFilter) -> Result<Vec<Lot>>;

    /// Delete a lot (only if no transactions)
    fn delete(&self, id: Uuid) -> Result<()>;

    /// Adjust lot quantity
    fn adjust(&self, input: AdjustLot) -> Result<LotTransaction>;

    /// Consume from a lot
    fn consume(&self, input: ConsumeLot) -> Result<LotTransaction>;

    /// Reserve quantity from a lot
    fn reserve(&self, input: ReserveLot) -> Result<Uuid>;

    /// Release a reservation
    fn release_reservation(&self, reservation_id: Uuid) -> Result<()>;

    /// Confirm a reservation (convert to consumption)
    fn confirm_reservation(&self, reservation_id: Uuid) -> Result<LotTransaction>;

    /// Transfer lot between locations
    fn transfer(&self, input: TransferLot) -> Result<LotTransaction>;

    /// Split a lot into two
    fn split(&self, input: SplitLot) -> Result<Lot>;

    /// Merge multiple lots into one
    fn merge(&self, input: MergeLots) -> Result<Lot>;

    /// Quarantine a lot
    fn quarantine(&self, id: Uuid, reason: &str) -> Result<Lot>;

    /// Release from quarantine
    fn release_quarantine(&self, id: Uuid) -> Result<Lot>;

    /// Get lot transactions
    fn get_transactions(&self, lot_id: Uuid, limit: u32) -> Result<Vec<LotTransaction>>;

    /// Get lot quantity at a location (None if no location record exists)
    fn get_quantity_at_location(
        &self,
        lot_id: Uuid,
        location_id: i32,
    ) -> Result<Option<rust_decimal::Decimal>>;

    /// Get all locations for a lot
    fn get_lot_locations(&self, lot_id: Uuid) -> Result<Vec<LotLocation>>;

    // Certificate operations
    /// Add certificate to lot
    fn add_certificate(&self, input: AddLotCertificate) -> Result<LotCertificate>;

    /// Get certificates for lot
    fn get_certificates(&self, lot_id: Uuid) -> Result<Vec<LotCertificate>>;

    /// Delete certificate
    fn delete_certificate(&self, certificate_id: Uuid) -> Result<()>;

    // Queries
    /// Get expiring lots
    fn get_expiring_lots(&self, days: i32) -> Result<Vec<Lot>>;

    /// Get expired lots
    fn get_expired_lots(&self) -> Result<Vec<Lot>>;

    /// Get lots with available quantity for SKU
    fn get_available_lots_for_sku(&self, sku: &str) -> Result<Vec<Lot>>;

    /// Trace lot (upstream and downstream)
    fn trace(&self, lot_id: Uuid) -> Result<TraceabilityResult>;

    /// Count lots
    fn count(&self, filter: LotFilter) -> Result<u64>;

    // Batch operations
    /// Create multiple lots
    fn create_batch(&self, inputs: Vec<CreateLot>) -> Result<BatchResult<Lot>>;

    /// Get multiple lots by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Lot>>;
}

// ============================================================================
// Serial Number Repository
// ============================================================================

/// Serial number management repository trait
pub trait SerialRepository {
    /// Create a serial number
    fn create(&self, input: CreateSerialNumber) -> Result<SerialNumber>;

    /// Create multiple serial numbers in bulk
    fn create_bulk(&self, input: CreateSerialNumbersBulk) -> Result<Vec<SerialNumber>>;

    /// Get serial by ID
    fn get(&self, id: Uuid) -> Result<Option<SerialNumber>>;

    /// Get serial by serial number string
    fn get_by_serial(&self, serial: &str) -> Result<Option<SerialNumber>>;

    /// Update a serial number
    fn update(&self, id: Uuid, input: UpdateSerialNumber) -> Result<SerialNumber>;

    /// List serials with filter
    fn list(&self, filter: SerialFilter) -> Result<Vec<SerialNumber>>;

    /// Delete a serial (only if never used)
    fn delete(&self, id: Uuid) -> Result<()>;

    /// Change serial status with history tracking
    fn change_status(&self, input: ChangeSerialStatus) -> Result<SerialNumber>;

    /// Reserve a serial
    fn reserve(&self, input: ReserveSerialNumber) -> Result<SerialReservation>;

    /// Release reservation
    fn release_reservation(&self, reservation_id: Uuid) -> Result<()>;

    /// Confirm reservation
    fn confirm_reservation(&self, reservation_id: Uuid) -> Result<()>;

    /// Move serial to new location
    fn move_serial(&self, input: MoveSerial) -> Result<SerialNumber>;

    /// Transfer ownership
    fn transfer_ownership(&self, input: TransferSerialOwnership) -> Result<SerialNumber>;

    /// Mark as sold
    fn mark_sold(
        &self,
        id: Uuid,
        customer_id: Uuid,
        order_id: Option<Uuid>,
    ) -> Result<SerialNumber>;

    /// Mark as shipped
    fn mark_shipped(&self, id: Uuid, shipment_id: Uuid) -> Result<SerialNumber>;

    /// Mark as returned
    fn mark_returned(&self, id: Uuid, return_id: Uuid) -> Result<SerialNumber>;

    /// Activate serial (e.g., for warranty)
    fn activate(&self, id: Uuid) -> Result<SerialNumber>;

    /// Quarantine serial
    fn quarantine(&self, id: Uuid, reason: &str) -> Result<SerialNumber>;

    /// Release from quarantine
    fn release_quarantine(&self, id: Uuid) -> Result<SerialNumber>;

    /// Scrap serial
    fn scrap(&self, id: Uuid, reason: &str) -> Result<SerialNumber>;

    // History operations
    /// Get serial history
    fn get_history(
        &self,
        serial_id: Uuid,
        filter: SerialHistoryFilter,
    ) -> Result<Vec<SerialHistory>>;

    /// Get full serial lookup with related data
    fn lookup(&self, serial: &str) -> Result<Option<SerialLookupResult>>;

    /// Validate serial number
    fn validate(&self, serial: &str) -> Result<SerialValidation>;

    // Queries
    /// Get available serials for SKU
    fn get_available_for_sku(&self, sku: &str, limit: u32) -> Result<Vec<SerialNumber>>;

    /// Get serials for lot
    fn get_for_lot(&self, lot_id: Uuid) -> Result<Vec<SerialNumber>>;

    /// Get serials for customer
    fn get_for_customer(&self, customer_id: Uuid) -> Result<Vec<SerialNumber>>;

    /// Count serials
    fn count(&self, filter: SerialFilter) -> Result<u64>;

    // Batch operations
    /// Create multiple serials
    fn create_batch(&self, inputs: Vec<CreateSerialNumber>) -> Result<BatchResult<SerialNumber>>;

    /// Get multiple serials by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<SerialNumber>>;

    /// Get multiple serials by serial string
    fn get_batch_by_serial(&self, serials: Vec<String>) -> Result<Vec<SerialNumber>>;
}

// ============================================================================
// Warehouse Repository
// ============================================================================

/// Warehouse management repository trait
pub trait WarehouseRepository {
    // Warehouse operations
    /// Create a new warehouse
    fn create_warehouse(&self, input: CreateWarehouse) -> Result<Warehouse>;

    /// Get warehouse by ID
    fn get_warehouse(&self, id: i32) -> Result<Option<Warehouse>>;

    /// Get warehouse by code
    fn get_warehouse_by_code(&self, code: &str) -> Result<Option<Warehouse>>;

    /// Update a warehouse
    fn update_warehouse(&self, id: i32, input: UpdateWarehouse) -> Result<Warehouse>;

    /// List warehouses with filter
    fn list_warehouses(&self, filter: WarehouseFilter) -> Result<Vec<Warehouse>>;

    /// Delete a warehouse (only if empty)
    fn delete_warehouse(&self, id: i32) -> Result<()>;

    /// Count warehouses
    fn count_warehouses(&self, filter: WarehouseFilter) -> Result<u64>;

    // Zone operations
    /// Create a zone
    fn create_zone(&self, input: CreateZone) -> Result<Zone>;

    /// Get zone by ID
    fn get_zone(&self, id: i32) -> Result<Option<Zone>>;

    /// Get zones for warehouse
    fn get_zones(&self, warehouse_id: i32) -> Result<Vec<Zone>>;

    /// Update a zone
    fn update_zone(&self, id: i32, input: UpdateZone) -> Result<Zone>;

    /// Delete a zone
    fn delete_zone(&self, id: i32) -> Result<()>;

    // Location operations
    /// Create a location
    fn create_location(&self, input: CreateLocation) -> Result<Location>;

    /// Get location by ID
    fn get_location(&self, id: i32) -> Result<Option<Location>>;

    /// Get location by code
    fn get_location_by_code(&self, warehouse_id: i32, code: &str) -> Result<Option<Location>>;

    /// Update a location
    fn update_location(&self, id: i32, input: UpdateLocation) -> Result<Location>;

    /// List locations with filter
    fn list_locations(&self, filter: LocationFilter) -> Result<Vec<Location>>;

    /// Delete a location (only if empty)
    fn delete_location(&self, id: i32) -> Result<()>;

    /// Count locations
    fn count_locations(&self, filter: LocationFilter) -> Result<u64>;

    /// Get locations for warehouse
    fn get_locations_for_warehouse(&self, warehouse_id: i32) -> Result<Vec<Location>>;

    /// Get pickable locations for SKU
    fn get_pickable_locations(&self, warehouse_id: i32, sku: &str) -> Result<Vec<Location>>;

    /// Get receivable locations
    fn get_receivable_locations(&self, warehouse_id: i32) -> Result<Vec<Location>>;

    // Location inventory operations
    /// Get inventory at location
    fn get_location_inventory(&self, location_id: i32) -> Result<Vec<LocationInventory>>;

    /// Get inventory for SKU across locations
    fn get_inventory_for_sku(&self, warehouse_id: i32, sku: &str)
        -> Result<Vec<LocationInventory>>;

    /// Adjust inventory at location
    fn adjust_inventory(&self, input: AdjustLocationInventory) -> Result<LocationInventory>;

    /// Move inventory between locations
    fn move_inventory(&self, input: MoveInventory) -> Result<LocationMovement>;

    /// Get location inventory by filter
    fn list_location_inventory(
        &self,
        filter: LocationInventoryFilter,
    ) -> Result<Vec<LocationInventory>>;

    // Movement operations
    /// Get inventory movements
    fn get_movements(&self, filter: MovementFilter) -> Result<Vec<LocationMovement>>;

    /// Count movements
    fn count_movements(&self, filter: MovementFilter) -> Result<u64>;

    // Batch operations
    /// Create multiple locations
    fn create_locations_batch(&self, inputs: Vec<CreateLocation>) -> Result<BatchResult<Location>>;

    /// Get multiple locations by ID
    fn get_locations_batch(&self, ids: Vec<i32>) -> Result<Vec<Location>>;
}

// ============================================================================
// Receiving Repository
// ============================================================================

/// Receiving/Goods receipt repository trait
pub trait ReceivingRepository {
    // Receipt operations
    /// Create a new receipt
    fn create_receipt(&self, input: CreateReceipt) -> Result<Receipt>;

    /// Get receipt by ID
    fn get_receipt(&self, id: Uuid) -> Result<Option<Receipt>>;

    /// Get receipt by receipt number
    fn get_receipt_by_number(&self, number: &str) -> Result<Option<Receipt>>;

    /// Update a receipt
    fn update_receipt(&self, id: Uuid, input: UpdateReceipt) -> Result<Receipt>;

    /// List receipts with filter
    fn list_receipts(&self, filter: ReceiptFilter) -> Result<Vec<Receipt>>;

    /// Delete a receipt (only if not started)
    fn delete_receipt(&self, id: Uuid) -> Result<()>;

    /// Start receiving (transition to in_progress)
    fn start_receiving(&self, id: Uuid) -> Result<Receipt>;

    /// Receive items on a receipt
    fn receive_items(&self, input: ReceiveItems) -> Result<Receipt>;

    /// Complete receiving (all items received)
    fn complete_receiving(&self, id: Uuid) -> Result<Receipt>;

    /// Cancel a receipt
    fn cancel_receipt(&self, id: Uuid) -> Result<Receipt>;

    /// Get receipt items
    fn get_receipt_items(&self, receipt_id: Uuid) -> Result<Vec<ReceiptItem>>;

    /// Count receipts
    fn count_receipts(&self, filter: ReceiptFilter) -> Result<u64>;

    // Put-away operations
    /// Create a put-away task
    fn create_put_away(&self, input: CreatePutAway) -> Result<PutAway>;

    /// Get put-away by ID
    fn get_put_away(&self, id: Uuid) -> Result<Option<PutAway>>;

    /// List put-aways with filter
    fn list_put_aways(&self, filter: PutAwayFilter) -> Result<Vec<PutAway>>;

    /// Assign put-away to user
    fn assign_put_away(&self, id: Uuid, assigned_to: &str) -> Result<PutAway>;

    /// Start put-away
    fn start_put_away(&self, id: Uuid) -> Result<PutAway>;

    /// Complete put-away
    fn complete_put_away(&self, input: CompletePutAway) -> Result<PutAway>;

    /// Cancel put-away
    fn cancel_put_away(&self, id: Uuid) -> Result<PutAway>;

    /// Get pending put-aways for receipt
    fn get_pending_put_aways(&self, receipt_id: Uuid) -> Result<Vec<PutAway>>;

    /// Count put-aways
    fn count_put_aways(&self, filter: PutAwayFilter) -> Result<u64>;

    // Integration with PO
    /// Create receipt from purchase order
    fn create_receipt_from_po(&self, po_id: Uuid, warehouse_id: i32) -> Result<Receipt>;

    // Batch operations
    /// Create multiple receipts
    fn create_receipts_batch(&self, inputs: Vec<CreateReceipt>) -> Result<BatchResult<Receipt>>;

    /// Get multiple receipts by ID
    fn get_receipts_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Receipt>>;
}

// ============================================================================
// Fulfillment Repository
// ============================================================================

/// Fulfillment (pick/pack/ship) repository trait
pub trait FulfillmentRepository {
    // Wave operations
    /// Create a wave from orders
    fn create_wave(&self, input: CreateWave) -> Result<Wave>;

    /// Get wave by ID
    fn get_wave(&self, id: Uuid) -> Result<Option<Wave>>;

    /// Get wave by number
    fn get_wave_by_number(&self, number: &str) -> Result<Option<Wave>>;

    /// List waves with filter
    fn list_waves(&self, filter: WaveFilter) -> Result<Vec<Wave>>;

    /// Release wave for picking
    fn release_wave(&self, id: Uuid) -> Result<Wave>;

    /// Complete a wave
    fn complete_wave(&self, id: Uuid) -> Result<Wave>;

    /// Cancel a wave
    fn cancel_wave(&self, id: Uuid) -> Result<Wave>;

    /// Get orders in a wave
    fn get_wave_orders(&self, wave_id: Uuid) -> Result<Vec<Uuid>>;

    /// Count waves
    fn count_waves(&self, filter: WaveFilter) -> Result<u64>;

    // Pick operations
    /// Create a pick task
    fn create_pick(&self, input: CreatePickTask) -> Result<PickTask>;

    /// Get pick task by ID
    fn get_pick(&self, id: Uuid) -> Result<Option<PickTask>>;

    /// List pick tasks with filter
    fn list_picks(&self, filter: PickTaskFilter) -> Result<Vec<PickTask>>;

    /// Assign pick to user
    fn assign_pick(&self, id: Uuid, assigned_to: &str) -> Result<PickTask>;

    /// Start a pick
    fn start_pick(&self, id: Uuid) -> Result<PickTask>;

    /// Complete a pick
    fn complete_pick(&self, input: CompletePick) -> Result<PickTask>;

    /// Report short pick
    fn report_short(
        &self,
        id: Uuid,
        short_qty: rust_decimal::Decimal,
        reason: &str,
    ) -> Result<PickTask>;

    /// Cancel a pick
    fn cancel_pick(&self, id: Uuid) -> Result<PickTask>;

    /// Get picks for order
    fn get_picks_for_order(&self, order_id: Uuid) -> Result<Vec<PickTask>>;

    /// Get picks for wave
    fn get_picks_for_wave(&self, wave_id: Uuid) -> Result<Vec<PickTask>>;

    /// Count picks
    fn count_picks(&self, filter: PickTaskFilter) -> Result<u64>;

    // Pack operations
    /// Create a pack task
    fn create_pack(&self, input: CreatePackTask) -> Result<PackTask>;

    /// Get pack task by ID
    fn get_pack(&self, id: Uuid) -> Result<Option<PackTask>>;

    /// List pack tasks with filter
    fn list_packs(&self, filter: PackTaskFilter) -> Result<Vec<PackTask>>;

    /// Assign pack to user
    fn assign_pack(&self, id: Uuid, assigned_to: &str) -> Result<PackTask>;

    /// Start packing
    fn start_pack(&self, id: Uuid) -> Result<PackTask>;

    /// Complete packing
    fn complete_pack(&self, id: Uuid) -> Result<PackTask>;

    /// Add carton to pack task
    fn add_carton(&self, input: AddCarton) -> Result<Carton>;

    /// Add item to carton
    fn add_carton_item(&self, input: AddCartonItem) -> Result<CartonItem>;

    /// Get cartons for pack task
    fn get_cartons(&self, pack_task_id: Uuid) -> Result<Vec<Carton>>;

    /// Get items in carton
    fn get_carton_items(&self, carton_id: Uuid) -> Result<Vec<CartonItem>>;

    /// Mark carton label printed
    fn mark_label_printed(&self, carton_id: Uuid) -> Result<Carton>;

    /// Cancel pack task
    fn cancel_pack(&self, id: Uuid) -> Result<PackTask>;

    /// Count packs
    fn count_packs(&self, filter: PackTaskFilter) -> Result<u64>;

    // Ship operations
    /// Create a ship task
    fn create_ship(&self, input: CreateShipTask) -> Result<ShipTask>;

    /// Get ship task by ID
    fn get_ship(&self, id: Uuid) -> Result<Option<ShipTask>>;

    /// List ship tasks with filter
    fn list_ships(&self, filter: ShipTaskFilter) -> Result<Vec<ShipTask>>;

    /// Assign ship to user
    fn assign_ship(&self, id: Uuid, assigned_to: &str) -> Result<ShipTask>;

    /// Print shipping label
    fn print_label(&self, id: Uuid, label_url: &str) -> Result<ShipTask>;

    /// Complete shipping
    fn complete_ship(&self, input: CompleteShip) -> Result<ShipTask>;

    /// Cancel ship task
    fn cancel_ship(&self, id: Uuid) -> Result<ShipTask>;

    /// Count ships
    fn count_ships(&self, filter: ShipTaskFilter) -> Result<u64>;

    // Workflow helpers
    /// Create picks for an order
    fn create_picks_for_order(&self, order_id: Uuid, warehouse_id: i32) -> Result<Vec<PickTask>>;

    /// Check if order is ready to pack
    fn is_order_ready_to_pack(&self, order_id: Uuid) -> Result<bool>;

    /// Check if order is ready to ship
    fn is_order_ready_to_ship(&self, order_id: Uuid) -> Result<bool>;

    // Batch operations
    /// Create multiple waves
    fn create_waves_batch(&self, inputs: Vec<CreateWave>) -> Result<BatchResult<Wave>>;

    /// Get multiple picks by ID
    fn get_picks_batch(&self, ids: Vec<Uuid>) -> Result<Vec<PickTask>>;
}

// ============================================================================
// Accounts Payable Repository
// ============================================================================

/// Accounts Payable repository trait
pub trait AccountsPayableRepository {
    // Bill operations
    /// Create a new bill
    fn create_bill(&self, input: CreateBill) -> Result<Bill>;

    /// Get bill by ID
    fn get_bill(&self, id: Uuid) -> Result<Option<Bill>>;

    /// Get bill by number
    fn get_bill_by_number(&self, number: &str) -> Result<Option<Bill>>;

    /// Update a bill
    fn update_bill(&self, id: Uuid, input: UpdateBill) -> Result<Bill>;

    /// List bills with filter
    fn list_bills(&self, filter: BillFilter) -> Result<Vec<Bill>>;

    /// Delete a bill (only if draft)
    fn delete_bill(&self, id: Uuid) -> Result<()>;

    /// Approve a bill
    fn approve_bill(&self, id: Uuid) -> Result<Bill>;

    /// Cancel a bill
    fn cancel_bill(&self, id: Uuid) -> Result<Bill>;

    /// Mark bill as disputed
    fn dispute_bill(&self, id: Uuid) -> Result<Bill>;

    /// Get bill items
    fn get_bill_items(&self, bill_id: Uuid) -> Result<Vec<BillItem>>;

    /// Add item to bill
    fn add_bill_item(&self, bill_id: Uuid, item: CreateBillItem) -> Result<BillItem>;

    /// Remove item from bill
    fn remove_bill_item(&self, item_id: Uuid) -> Result<()>;

    /// Count bills
    fn count_bills(&self, filter: BillFilter) -> Result<u64>;

    /// Get overdue bills
    fn get_overdue_bills(&self) -> Result<Vec<Bill>>;

    /// Get bills due soon (within days)
    fn get_bills_due_soon(&self, days: i32) -> Result<Vec<Bill>>;

    // Payment operations
    /// Create a payment
    fn create_payment(&self, input: CreateBillPayment) -> Result<BillPayment>;

    /// Get payment by ID
    fn get_payment(&self, id: Uuid) -> Result<Option<BillPayment>>;

    /// Get payment by number
    fn get_payment_by_number(&self, number: &str) -> Result<Option<BillPayment>>;

    /// List payments with filter
    fn list_payments(&self, filter: BillPaymentFilter) -> Result<Vec<BillPayment>>;

    /// Void a payment
    fn void_payment(&self, id: Uuid) -> Result<BillPayment>;

    /// Mark payment as cleared
    fn clear_payment(&self, id: Uuid) -> Result<BillPayment>;

    /// Get payment allocations
    fn get_payment_allocations(&self, payment_id: Uuid) -> Result<Vec<PaymentAllocation>>;

    /// Get payments for bill
    fn get_payments_for_bill(&self, bill_id: Uuid) -> Result<Vec<BillPayment>>;

    /// Count payments
    fn count_payments(&self, filter: BillPaymentFilter) -> Result<u64>;

    // Payment run operations
    /// Create a payment run
    fn create_payment_run(&self, input: CreatePaymentRun) -> Result<PaymentRun>;

    /// Get payment run by ID
    fn get_payment_run(&self, id: Uuid) -> Result<Option<PaymentRun>>;

    /// List payment runs with filter
    fn list_payment_runs(&self, filter: PaymentRunFilter) -> Result<Vec<PaymentRun>>;

    /// Approve payment run
    fn approve_payment_run(&self, id: Uuid, approved_by: &str) -> Result<PaymentRun>;

    /// Process payment run
    fn process_payment_run(&self, id: Uuid) -> Result<PaymentRun>;

    /// Cancel payment run
    fn cancel_payment_run(&self, id: Uuid) -> Result<PaymentRun>;

    /// Get bills in payment run
    fn get_payment_run_bills(&self, run_id: Uuid) -> Result<Vec<Bill>>;

    // Analytics
    /// Get AP aging summary
    fn get_aging_summary(&self) -> Result<ApAgingSummary>;

    /// Get AP summary by supplier (None if supplier is not found)
    fn get_supplier_summary(&self, supplier_id: Uuid) -> Result<Option<SupplierApSummary>>;

    /// Get total AP outstanding
    fn get_total_outstanding(&self) -> Result<rust_decimal::Decimal>;

    // Batch operations
    /// Create multiple bills
    fn create_bills_batch(&self, inputs: Vec<CreateBill>) -> Result<BatchResult<Bill>>;

    /// Get multiple bills by ID
    fn get_bills_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Bill>>;
}

/// Cost Accounting repository trait
pub trait CostAccountingRepository {
    // Item cost operations
    /// Get item cost by SKU
    fn get_item_cost(&self, sku: &str) -> Result<Option<ItemCost>>;

    /// Set/update item cost
    fn set_item_cost(&self, input: SetItemCost) -> Result<ItemCost>;

    /// List item costs
    fn list_item_costs(&self, filter: ItemCostFilter) -> Result<Vec<ItemCost>>;

    /// Update average cost (called when receiving inventory)
    fn update_average_cost(
        &self,
        sku: &str,
        quantity: rust_decimal::Decimal,
        unit_cost: rust_decimal::Decimal,
    ) -> Result<ItemCost>;

    /// Update last cost
    fn update_last_cost(&self, sku: &str, unit_cost: rust_decimal::Decimal) -> Result<ItemCost>;

    // Cost layer operations (for FIFO/LIFO)
    /// Create a cost layer
    fn create_cost_layer(&self, input: CreateCostLayer) -> Result<CostLayer>;

    /// Get cost layer by ID
    fn get_cost_layer(&self, id: Uuid) -> Result<Option<CostLayer>>;

    /// List cost layers
    fn list_cost_layers(&self, filter: CostLayerFilter) -> Result<Vec<CostLayer>>;

    /// Issue from cost layers (FIFO)
    fn issue_fifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>>;

    /// Issue from cost layers (LIFO)
    fn issue_lifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>>;

    /// Get remaining quantity in layers for SKU
    fn get_layers_remaining(&self, sku: &str) -> Result<rust_decimal::Decimal>;

    // Cost transaction operations
    /// Record a cost transaction
    #[allow(clippy::too_many_arguments)]
    fn record_cost_transaction(
        &self,
        sku: &str,
        transaction_type: CostTransactionType,
        quantity: rust_decimal::Decimal,
        unit_cost: rust_decimal::Decimal,
        layer_id: Option<Uuid>,
        reference_type: Option<&str>,
        reference_id: Option<Uuid>,
        notes: Option<&str>,
    ) -> Result<CostTransaction>;

    /// List cost transactions
    fn list_cost_transactions(&self, filter: CostTransactionFilter)
        -> Result<Vec<CostTransaction>>;

    // Cost variance operations
    /// Record a cost variance
    fn record_variance(&self, input: RecordCostVariance) -> Result<CostVariance>;

    /// List cost variances
    fn list_variances(&self, filter: CostVarianceFilter) -> Result<Vec<CostVariance>>;

    /// Get variance summary for period
    fn get_variance_summary(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<rust_decimal::Decimal>;

    // Cost adjustment operations
    /// Create a cost adjustment
    fn create_adjustment(&self, input: CreateCostAdjustment) -> Result<CostAdjustment>;

    /// Get adjustment by ID
    fn get_adjustment(&self, id: Uuid) -> Result<Option<CostAdjustment>>;

    /// List adjustments
    fn list_adjustments(&self, filter: CostAdjustmentFilter) -> Result<Vec<CostAdjustment>>;

    /// Approve adjustment
    fn approve_adjustment(&self, id: Uuid, approved_by: &str) -> Result<CostAdjustment>;

    /// Apply adjustment (update item cost)
    fn apply_adjustment(&self, id: Uuid) -> Result<CostAdjustment>;

    /// Reject adjustment
    fn reject_adjustment(&self, id: Uuid) -> Result<CostAdjustment>;

    // Rollup operations
    /// Calculate cost rollup for manufactured item
    fn calculate_rollup(&self, sku: &str, bom_id: Option<Uuid>) -> Result<CostRollup>;

    /// Get latest rollup for SKU
    fn get_rollup(&self, sku: &str) -> Result<Option<CostRollup>>;

    // Valuation operations
    /// Get inventory valuation
    fn get_inventory_valuation(&self, cost_method: CostMethod) -> Result<InventoryValuation>;

    /// Get SKU cost summary
    fn get_sku_cost_summary(&self, sku: &str) -> Result<Option<SkuCostSummary>>;

    /// Get total inventory value
    fn get_total_inventory_value(&self) -> Result<rust_decimal::Decimal>;
}

/// Credit Management repository trait
pub trait CreditRepository {
    // Credit account operations
    /// Create a credit account for a customer
    fn create_credit_account(&self, input: CreateCreditAccount) -> Result<CreditAccount>;

    /// Get credit account by ID
    fn get_credit_account(&self, id: Uuid) -> Result<Option<CreditAccount>>;

    /// Get credit account by customer ID
    fn get_credit_account_by_customer(&self, customer_id: Uuid) -> Result<Option<CreditAccount>>;

    /// Update credit account
    fn update_credit_account(&self, id: Uuid, input: UpdateCreditAccount) -> Result<CreditAccount>;

    /// List credit accounts
    fn list_credit_accounts(&self, filter: CreditAccountFilter) -> Result<Vec<CreditAccount>>;

    /// Adjust credit limit
    fn adjust_credit_limit(
        &self,
        customer_id: Uuid,
        new_limit: rust_decimal::Decimal,
        reason: &str,
    ) -> Result<CreditAccount>;

    /// Suspend credit account
    fn suspend_credit_account(&self, customer_id: Uuid, reason: &str) -> Result<CreditAccount>;

    /// Reactivate credit account
    fn reactivate_credit_account(&self, customer_id: Uuid) -> Result<CreditAccount>;

    // Credit check operations
    /// Check credit for an order
    fn check_credit(
        &self,
        customer_id: Uuid,
        order_amount: rust_decimal::Decimal,
    ) -> Result<CreditCheckResult>;

    /// Reserve credit for an order
    fn reserve_credit(
        &self,
        customer_id: Uuid,
        order_id: Uuid,
        amount: rust_decimal::Decimal,
    ) -> Result<CreditAccount>;

    /// Release credit reservation
    fn release_credit_reservation(
        &self,
        customer_id: Uuid,
        order_id: Uuid,
    ) -> Result<CreditAccount>;

    /// Charge credit (convert reservation to balance)
    fn charge_credit(
        &self,
        customer_id: Uuid,
        order_id: Uuid,
        amount: rust_decimal::Decimal,
    ) -> Result<CreditAccount>;

    // Credit hold operations
    /// Place a credit hold
    fn place_hold(&self, input: PlaceCreditHold) -> Result<CreditHold>;

    /// Get credit hold by ID
    fn get_hold(&self, id: Uuid) -> Result<Option<CreditHold>>;

    /// List credit holds
    fn list_holds(&self, filter: CreditHoldFilter) -> Result<Vec<CreditHold>>;

    /// Release a credit hold
    fn release_hold(&self, input: ReleaseCreditHold) -> Result<CreditHold>;

    /// Get active holds for customer
    fn get_active_holds(&self, customer_id: Uuid) -> Result<Vec<CreditHold>>;

    /// Get active holds for order
    fn get_holds_for_order(&self, order_id: Uuid) -> Result<Vec<CreditHold>>;

    // Credit application operations
    /// Submit a credit application
    fn submit_application(&self, input: SubmitCreditApplication) -> Result<CreditApplication>;

    /// Get credit application by ID
    fn get_application(&self, id: Uuid) -> Result<Option<CreditApplication>>;

    /// List credit applications
    fn list_applications(&self, filter: CreditApplicationFilter) -> Result<Vec<CreditApplication>>;

    /// Review credit application
    fn review_application(&self, input: ReviewCreditApplication) -> Result<CreditApplication>;

    /// Withdraw credit application
    fn withdraw_application(&self, id: Uuid) -> Result<CreditApplication>;

    // Transaction operations
    /// Record a credit transaction
    fn record_transaction(&self, input: RecordCreditTransaction) -> Result<CreditTransaction>;

    /// List credit transactions
    fn list_transactions(&self, filter: CreditTransactionFilter) -> Result<Vec<CreditTransaction>>;

    /// Apply payment to balance
    fn apply_payment(
        &self,
        customer_id: Uuid,
        amount: rust_decimal::Decimal,
        reference_id: Option<Uuid>,
    ) -> Result<CreditAccount>;

    // Analytics
    /// Get customer credit summary
    fn get_customer_summary(&self, customer_id: Uuid) -> Result<Option<CustomerCreditSummary>>;

    /// Get credit aging buckets
    fn get_aging_report(&self) -> Result<Vec<(Uuid, CreditAgingBucket)>>;

    /// Get customers over credit limit
    fn get_over_limit_customers(&self) -> Result<Vec<CreditAccount>>;
}

/// Backorder repository trait
pub trait BackorderRepository {
    // Backorder operations
    /// Create a backorder
    fn create_backorder(&self, input: CreateBackorder) -> Result<Backorder>;

    /// Get backorder by ID
    fn get_backorder(&self, id: Uuid) -> Result<Option<Backorder>>;

    /// Get backorder by number
    fn get_backorder_by_number(&self, number: &str) -> Result<Option<Backorder>>;

    /// Update backorder
    fn update_backorder(&self, id: Uuid, input: UpdateBackorder) -> Result<Backorder>;

    /// List backorders
    fn list_backorders(&self, filter: BackorderFilter) -> Result<Vec<Backorder>>;

    /// Cancel backorder
    fn cancel_backorder(&self, id: Uuid) -> Result<Backorder>;

    /// Get backorders for order
    fn get_backorders_for_order(&self, order_id: Uuid) -> Result<Vec<Backorder>>;

    /// Get backorders for customer
    fn get_backorders_for_customer(&self, customer_id: Uuid) -> Result<Vec<Backorder>>;

    /// Get backorders for SKU
    fn get_backorders_for_sku(&self, sku: &str) -> Result<Vec<Backorder>>;

    // Fulfillment operations
    /// Fulfill backorder (partial or full)
    fn fulfill_backorder(&self, input: FulfillBackorder) -> Result<Backorder>;

    /// Get fulfillment history for backorder
    fn get_fulfillment_history(&self, backorder_id: Uuid) -> Result<Vec<BackorderFulfillment>>;

    // Allocation operations
    /// Allocate inventory to backorder
    fn allocate_backorder(&self, input: AllocateBackorder) -> Result<BackorderAllocation>;

    /// Get allocations for backorder
    fn get_allocations(&self, backorder_id: Uuid) -> Result<Vec<BackorderAllocation>>;

    /// Release allocation
    fn release_allocation(&self, allocation_id: Uuid) -> Result<BackorderAllocation>;

    /// Confirm allocation
    fn confirm_allocation(&self, allocation_id: Uuid) -> Result<BackorderAllocation>;

    /// Expire old allocations
    fn expire_allocations(&self) -> Result<u32>;

    // Auto-allocation
    /// Auto-allocate available inventory to pending backorders
    fn auto_allocate_inventory(&self, sku: &str) -> Result<Vec<BackorderAllocation>>;

    // Analytics
    /// Get backorder summary
    fn get_summary(&self) -> Result<BackorderSummary>;

    /// Get SKU backorder summary
    fn get_sku_summary(&self, sku: &str) -> Result<Option<SkuBackorderSummary>>;

    /// Get overdue backorders
    fn get_overdue_backorders(&self) -> Result<Vec<Backorder>>;

    /// Count pending backorders
    fn count_pending(&self) -> Result<u64>;
}

// ============================================================================
// Accounts Receivable Repository
// ============================================================================

/// Accounts Receivable repository trait
pub trait AccountsReceivableRepository {
    // Aging reports
    /// Get AR aging summary across all customers
    fn get_aging_summary(&self) -> Result<ArAgingSummary>;

    /// Get aging by customer (None if customer is not found)
    fn get_customer_aging(&self, customer_id: Uuid) -> Result<Option<CustomerArAging>>;

    /// Get all customers with aging (AR aging report)
    fn get_aging_report(&self, filter: ArAgingFilter) -> Result<Vec<CustomerArAging>>;

    // Collection management
    /// Log collection activity
    fn log_collection_activity(
        &self,
        input: CreateCollectionActivity,
    ) -> Result<CollectionActivity>;

    /// Get collection activities
    fn list_collection_activities(
        &self,
        filter: CollectionActivityFilter,
    ) -> Result<Vec<CollectionActivity>>;

    /// Update invoice collection status
    fn update_collection_status(&self, invoice_id: Uuid, status: CollectionStatus) -> Result<()>;

    /// Get invoices due for dunning (based on aging)
    fn get_invoices_due_for_dunning(&self) -> Result<Vec<Invoice>>;

    /// Send dunning letter (records activity, updates status)
    fn send_dunning_letter(
        &self,
        invoice_id: Uuid,
        letter_type: DunningLetterType,
        sent_by: Option<&str>,
    ) -> Result<CollectionActivity>;

    // Write-offs
    /// Create a write-off
    fn create_write_off(&self, input: CreateWriteOff) -> Result<WriteOff>;

    /// Get write-off by ID
    fn get_write_off(&self, id: Uuid) -> Result<Option<WriteOff>>;

    /// List write-offs
    fn list_write_offs(&self, filter: WriteOffFilter) -> Result<Vec<WriteOff>>;

    /// Reverse a write-off
    fn reverse_write_off(&self, id: Uuid) -> Result<WriteOff>;

    // Credit memos
    /// Create a credit memo
    fn create_credit_memo(&self, input: CreateCreditMemo) -> Result<CreditMemo>;

    /// Get credit memo by ID
    fn get_credit_memo(&self, id: Uuid) -> Result<Option<CreditMemo>>;

    /// Get credit memo by number
    fn get_credit_memo_by_number(&self, number: &str) -> Result<Option<CreditMemo>>;

    /// List credit memos
    fn list_credit_memos(&self, filter: CreditMemoFilter) -> Result<Vec<CreditMemo>>;

    /// Apply credit memo to invoice
    fn apply_credit_memo(&self, input: ApplyCreditMemo) -> Result<CreditMemo>;

    /// Void credit memo
    fn void_credit_memo(&self, id: Uuid) -> Result<CreditMemo>;

    /// Get unapplied credit memos for customer
    fn get_unapplied_credits(&self, customer_id: Uuid) -> Result<Vec<CreditMemo>>;

    // Payment application
    /// Apply payment to invoices
    fn apply_payment_to_invoices(
        &self,
        input: ApplyPaymentToInvoices,
    ) -> Result<Vec<ArPaymentApplication>>;

    /// Get payment applications
    fn get_payment_applications(&self, payment_id: Uuid) -> Result<Vec<ArPaymentApplication>>;

    /// Unapply payment from invoice
    fn unapply_payment(&self, application_id: Uuid) -> Result<()>;

    // Customer summaries and statements
    /// Get customer AR summary (None if customer is not found)
    fn get_customer_summary(&self, customer_id: Uuid) -> Result<Option<CustomerArSummary>>;

    /// Generate customer statement
    fn generate_statement(&self, request: GenerateStatementRequest) -> Result<CustomerStatement>;

    // Analytics
    /// Get total AR outstanding
    fn get_total_outstanding(&self) -> Result<rust_decimal::Decimal>;

    /// Get Days Sales Outstanding (DSO)
    fn get_dso(&self, days: i32) -> Result<rust_decimal::Decimal>;

    /// Get average days to pay by customer
    fn get_average_days_to_pay(&self, customer_id: Uuid) -> Result<Option<i32>>;

    // Batch operations
    fn get_customers_batch(&self, ids: Vec<Uuid>) -> Result<Vec<CustomerArSummary>>;
}

// ============================================================================
// General Ledger Repository
// ============================================================================

use chrono::NaiveDate;

/// General Ledger repository trait
pub trait GeneralLedgerRepository {
    // Chart of Accounts
    /// Create a GL account
    fn create_account(&self, input: CreateGlAccount) -> Result<GlAccount>;

    /// Get account by ID
    fn get_account(&self, id: Uuid) -> Result<Option<GlAccount>>;

    /// Get account by account number
    fn get_account_by_number(&self, account_number: &str) -> Result<Option<GlAccount>>;

    /// Update account
    fn update_account(&self, id: Uuid, input: UpdateGlAccount) -> Result<GlAccount>;

    /// List accounts (Chart of Accounts)
    fn list_accounts(&self, filter: GlAccountFilter) -> Result<Vec<GlAccount>>;

    /// Get account hierarchy (parent-child)
    fn get_account_hierarchy(&self) -> Result<Vec<GlAccount>>;

    /// Delete account (only if no transactions)
    fn delete_account(&self, id: Uuid) -> Result<()>;

    /// Initialize default Chart of Accounts
    fn initialize_chart_of_accounts(&self) -> Result<Vec<GlAccount>>;

    // GL Periods
    /// Create a GL period
    fn create_period(&self, input: CreateGlPeriod) -> Result<GlPeriod>;

    /// Get period by ID
    fn get_period(&self, id: Uuid) -> Result<Option<GlPeriod>>;

    /// Get current open period
    fn get_current_period(&self) -> Result<Option<GlPeriod>>;

    /// Get period for a date
    fn get_period_for_date(&self, date: NaiveDate) -> Result<Option<GlPeriod>>;

    /// List periods
    fn list_periods(&self, filter: GlPeriodFilter) -> Result<Vec<GlPeriod>>;

    /// Open a period
    fn open_period(&self, id: Uuid) -> Result<GlPeriod>;

    /// Close a period
    fn close_period(&self, id: Uuid, closed_by: &str) -> Result<GlPeriod>;

    /// Lock a period (prevents any changes)
    fn lock_period(&self, id: Uuid, locked_by: &str) -> Result<GlPeriod>;

    /// Reopen a closed period (not locked)
    fn reopen_period(&self, id: Uuid) -> Result<GlPeriod>;

    // Journal Entries
    /// Create a journal entry
    fn create_journal_entry(&self, input: CreateJournalEntry) -> Result<JournalEntry>;

    /// Get journal entry by ID
    fn get_journal_entry(&self, id: Uuid) -> Result<Option<JournalEntry>>;

    /// Get journal entry by number
    fn get_journal_entry_by_number(&self, number: &str) -> Result<Option<JournalEntry>>;

    /// List journal entries
    fn list_journal_entries(&self, filter: JournalEntryFilter) -> Result<Vec<JournalEntry>>;

    /// Post a journal entry (update account balances)
    fn post_journal_entry(&self, id: Uuid, posted_by: &str) -> Result<JournalEntry>;

    /// Void a journal entry
    fn void_journal_entry(&self, id: Uuid) -> Result<JournalEntry>;

    /// Reverse a journal entry (creates reversing entry)
    fn reverse_journal_entry(&self, id: Uuid, reversal_date: NaiveDate) -> Result<JournalEntry>;

    /// Get journal entry lines
    fn get_journal_entry_lines(&self, journal_entry_id: Uuid) -> Result<Vec<JournalEntryLine>>;

    // Auto-posting
    /// Get active auto-posting config
    fn get_auto_posting_config(&self) -> Result<Option<AutoPostingConfig>>;

    /// Create/update auto-posting config
    fn set_auto_posting_config(&self, input: CreateAutoPostingConfig) -> Result<AutoPostingConfig>;

    /// Auto-post invoice creation (DR AR / CR Revenue)
    fn auto_post_invoice(&self, invoice_id: Uuid) -> Result<JournalEntry>;

    /// Auto-post payment received (DR Cash / CR AR)
    fn auto_post_payment_received(&self, payment_id: Uuid) -> Result<JournalEntry>;

    /// Auto-post bill creation (DR Expense / CR AP)
    fn auto_post_bill(&self, bill_id: Uuid) -> Result<JournalEntry>;

    /// Auto-post bill payment (DR AP / CR Cash)
    fn auto_post_bill_payment(&self, payment_id: Uuid) -> Result<JournalEntry>;

    /// Auto-post inventory cost transaction (DR/CR Inventory/COGS)
    fn auto_post_inventory_cost(&self, cost_transaction_id: Uuid) -> Result<JournalEntry>;

    /// Auto-post write-off (DR Bad Debt / CR AR)
    fn auto_post_write_off(&self, write_off_id: Uuid) -> Result<JournalEntry>;

    // Financial Reports
    /// Generate trial balance
    fn get_trial_balance(&self, as_of_date: NaiveDate) -> Result<TrialBalance>;

    /// Generate balance sheet
    fn get_balance_sheet(&self, as_of_date: NaiveDate) -> Result<BalanceSheet>;

    /// Generate income statement
    fn get_income_statement(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<IncomeStatement>;

    /// Get account balance (None if account is not found)
    fn get_account_balance(
        &self,
        account_id: Uuid,
        as_of_date: Option<NaiveDate>,
    ) -> Result<Option<rust_decimal::Decimal>>;

    /// Get account transaction history
    fn get_account_transactions(
        &self,
        account_id: Uuid,
        filter: JournalEntryFilter,
    ) -> Result<Vec<JournalEntryLine>>;

    // Period close process
    /// Run period close (creates closing entries)
    fn run_period_close(&self, period_id: Uuid, closed_by: &str) -> Result<JournalEntry>;

    // Batch operations
    fn create_accounts_batch(&self, inputs: Vec<CreateGlAccount>)
        -> Result<BatchResult<GlAccount>>;
    fn get_accounts_batch(&self, ids: Vec<Uuid>) -> Result<Vec<GlAccount>>;
}

// ============================================================================
// Vector Search Repository
// ============================================================================

/// Vector search repository trait for semantic similarity search
pub trait VectorRepository {
    /// Store embedding for an entity
    fn store_embedding(
        &self,
        entity_type: EntityType,
        entity_id: &str,
        embedding: &[f32],
        text_hash: &str,
        model: &str,
    ) -> Result<()>;

    /// Search similar products by embedding vector
    fn search_products(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Product>>>;

    /// Search similar customers by embedding vector
    fn search_customers(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Customer>>>;

    /// Search similar orders by embedding vector
    fn search_orders(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Order>>>;

    /// Search similar inventory items by embedding vector
    fn search_inventory(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<InventoryItem>>>;

    /// Delete embedding for an entity
    fn delete_embedding(&self, entity_type: EntityType, entity_id: &str) -> Result<()>;

    /// Check if entity has an embedding stored
    fn has_embedding(&self, entity_type: EntityType, entity_id: &str) -> Result<bool>;

    /// Get embedding metadata for an entity
    fn get_embedding_metadata(
        &self,
        entity_type: EntityType,
        entity_id: &str,
    ) -> Result<Option<EmbeddingMetadata>>;

    /// Get embedding statistics
    fn get_stats(&self) -> Result<EmbeddingStats>;

    /// Delete all embeddings for an entity type
    fn clear_embeddings(&self, entity_type: EntityType) -> Result<u64>;
}

// ============================================================================
// X402 Payment Intent Repository
// ============================================================================

/// X402 Payment Intent repository trait for off-chain payment signing
pub trait X402PaymentIntentRepository {
    /// Create a new x402 payment intent
    fn create(&self, input: CreateX402PaymentIntent) -> Result<X402PaymentIntent>;

    /// Get payment intent by ID
    fn get(&self, id: Uuid) -> Result<Option<X402PaymentIntent>>;

    /// Get payment intent by idempotency key
    fn get_by_idempotency_key(&self, key: &str) -> Result<Option<X402PaymentIntent>>;

    /// Sign a payment intent (records signature and public key)
    fn sign(&self, id: Uuid, input: SignX402PaymentIntent) -> Result<X402PaymentIntent>;

    /// Mark intent as sequenced (submitted to sequencer)
    fn mark_sequenced(
        &self,
        id: Uuid,
        sequence_number: u64,
        batch_id: Uuid,
    ) -> Result<X402PaymentIntent>;

    /// Mark intent as settled (confirmed on-chain)
    fn mark_settled(&self, id: Uuid, tx_hash: &str, block_number: u64)
        -> Result<X402PaymentIntent>;

    /// Mark intent as failed
    fn mark_failed(&self, id: Uuid, reason: &str) -> Result<X402PaymentIntent>;

    /// Mark intent as expired
    fn mark_expired(&self, id: Uuid) -> Result<X402PaymentIntent>;

    /// Cancel a payment intent (only if not yet sequenced)
    fn cancel(&self, id: Uuid) -> Result<X402PaymentIntent>;

    /// Get payment intents for a cart
    fn for_cart(&self, cart_id: Uuid) -> Result<Vec<X402PaymentIntent>>;

    /// Get payment intents for an order
    fn for_order(&self, order_id: Uuid) -> Result<Vec<X402PaymentIntent>>;

    /// Get the next nonce for a payer address
    fn get_next_nonce(&self, payer_address: &str) -> Result<u64>;

    /// List payment intents with filter
    fn list(&self, filter: X402PaymentIntentFilter) -> Result<Vec<X402PaymentIntent>>;

    /// Count payment intents matching filter
    fn count(&self, filter: X402PaymentIntentFilter) -> Result<u64>;

    /// Expire all intents that have passed their valid_until timestamp
    fn expire_stale_intents(&self) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple payment intents - partial success allowed
    fn create_batch(
        &self,
        inputs: Vec<CreateX402PaymentIntent>,
    ) -> Result<BatchResult<X402PaymentIntent>>;

    /// Create multiple payment intents - atomic (all-or-nothing)
    fn create_batch_atomic(
        &self,
        inputs: Vec<CreateX402PaymentIntent>,
    ) -> Result<Vec<X402PaymentIntent>>;

    /// Get multiple payment intents by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<X402PaymentIntent>>;
}

// ============================================================================
// X402 Credit Repository (Metered Billing)
// ============================================================================

/// X402 credit ledger repository for prepaid balances and metered usage.
pub trait X402CreditRepository {
    /// Get a credit account for payer/asset/network
    fn get_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<Option<X402CreditAccount>>;

    /// Get or create a credit account (balance default = 0)
    fn get_or_create_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<X402CreditAccount>;

    /// Get current balance for payer/asset/network
    fn get_balance(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<u64>;

    /// Apply a credit or debit adjustment
    fn adjust_balance(&self, input: X402CreditAdjustment) -> Result<X402CreditTransaction>;

    /// List credit transactions with optional filter
    fn list_transactions(
        &self,
        filter: X402CreditTransactionFilter,
    ) -> Result<Vec<X402CreditTransaction>>;
}

// ============================================================================
// Agent Card Repository
// ============================================================================

/// Agent Card repository trait for AI agent identity and capabilities
pub trait AgentCardRepository {
    /// Create a new agent card
    fn create(&self, input: CreateAgentCard) -> Result<AgentCard>;

    /// Get agent card by ID
    fn get(&self, id: Uuid) -> Result<Option<AgentCard>>;

    /// Get agent card by wallet address
    fn get_by_wallet(&self, wallet_address: &str) -> Result<Option<AgentCard>>;

    /// Update an agent card
    fn update(&self, id: Uuid, input: UpdateAgentCard) -> Result<AgentCard>;

    /// Delete an agent card
    fn delete(&self, id: Uuid) -> Result<()>;

    /// List agent cards with filter
    fn list(&self, filter: AgentCardFilter) -> Result<Vec<AgentCard>>;

    /// Count agent cards matching filter
    fn count(&self, filter: AgentCardFilter) -> Result<u64>;

    /// Verify an agent card (set trust level and verification info)
    fn verify(&self, id: Uuid, trust_level: TrustLevel, method: &str) -> Result<AgentCard>;

    /// Suspend an agent card
    fn suspend(&self, id: Uuid, reason: &str) -> Result<AgentCard>;

    /// Reactivate a suspended agent card
    fn reactivate(&self, id: Uuid) -> Result<AgentCard>;

    /// Discover agents with specific capabilities
    fn discover(&self, filter: AgentCardFilter) -> Result<Vec<AgentCard>>;

    // === Batch Operations ===

    /// Create multiple agent cards - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateAgentCard>) -> Result<BatchResult<AgentCard>>;

    /// Create multiple agent cards - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateAgentCard>) -> Result<Vec<AgentCard>>;

    /// Get multiple agent cards by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<AgentCard>>;
}

// ============================================================================
// ERC-8004 Agent Identity Repository
// ============================================================================

/// Agent identity registry repository (ERC-8004)
pub trait AgentIdentityRepository {
    /// Register a new agent identity
    fn register(&self, input: CreateAgentIdentity) -> Result<AgentIdentity>;

    /// Get identity by agent registry and agent ID
    fn get(&self, agent_registry: &str, agent_id: &str) -> Result<Option<AgentIdentity>>;

    /// Get identity by agent wallet address
    fn get_by_wallet(&self, agent_wallet: &str) -> Result<Option<AgentIdentity>>;

    /// Update agent identity
    fn update(
        &self,
        agent_registry: &str,
        agent_id: &str,
        input: UpdateAgentIdentity,
    ) -> Result<AgentIdentity>;

    /// Set or update agent wallet with proof metadata
    #[allow(clippy::too_many_arguments)]
    fn set_agent_wallet(
        &self,
        agent_registry: &str,
        agent_id: &str,
        agent_wallet: &str,
        proof_type: Option<AgentWalletProofType>,
        proof: Option<&str>,
        proof_chain_id: Option<u64>,
        proof_deadline: Option<DateTime<Utc>>,
    ) -> Result<AgentIdentity>;

    /// Clear agent wallet
    fn clear_agent_wallet(&self, agent_registry: &str, agent_id: &str) -> Result<AgentIdentity>;

    /// List identities with optional filtering
    fn list(&self, filter: AgentIdentityFilter) -> Result<Vec<AgentIdentity>>;

    /// Count identities matching filter
    fn count(&self, filter: AgentIdentityFilter) -> Result<u64>;

    /// Set identity metadata entry
    fn set_metadata(
        &self,
        agent_registry: &str,
        agent_id: &str,
        entry: AgentMetadataEntry,
    ) -> Result<()>;

    /// Get identity metadata entry
    fn get_metadata(
        &self,
        agent_registry: &str,
        agent_id: &str,
        metadata_key: &str,
    ) -> Result<Option<Vec<u8>>>;

    /// Delete identity metadata entry
    fn delete_metadata(
        &self,
        agent_registry: &str,
        agent_id: &str,
        metadata_key: &str,
    ) -> Result<()>;
}

// ============================================================================
// ERC-8004 Reputation Registry
// ============================================================================

/// Reputation feedback registry repository (ERC-8004)
pub trait AgentReputationRepository {
    /// Submit feedback for an agent
    fn give_feedback(&self, input: CreateAgentFeedback) -> Result<AgentFeedback>;

    /// Revoke previously submitted feedback
    fn revoke_feedback(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
        feedback_index: u64,
    ) -> Result<AgentFeedback>;

    /// Read a specific feedback entry
    fn read_feedback(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
        feedback_index: u64,
    ) -> Result<Option<AgentFeedback>>;

    /// Read feedback entries with filters
    fn read_all_feedback(&self, filter: AgentFeedbackFilter) -> Result<Vec<AgentFeedback>>;

    /// Get feedback summary for an agent (filtered by client addresses + tags)
    fn get_summary(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_addresses: Vec<String>,
        tag1: Option<String>,
        tag2: Option<String>,
    ) -> Result<FeedbackSummary>;

    /// Append a response to a feedback entry
    fn append_response(&self, input: CreateAgentFeedbackResponse) -> Result<AgentFeedbackResponse>;

    /// Count responses for a feedback entry
    fn get_response_count(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
        feedback_index: u64,
        responders: Option<Vec<String>>,
    ) -> Result<u64>;

    /// List client addresses that have provided feedback
    fn get_clients(&self, agent_registry: &str, agent_id: &str) -> Result<Vec<String>>;

    /// Get last feedback index for a client/agent pair
    fn get_last_index(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
    ) -> Result<u64>;
}

// ============================================================================
// ERC-8004 Validation Registry
// ============================================================================

/// Validation registry repository (ERC-8004)
pub trait AgentValidationRepository {
    /// Submit a validation request
    fn request_validation(
        &self,
        input: CreateAgentValidationRequest,
    ) -> Result<AgentValidationRequest>;

    /// Record a validation response for a request hash
    fn respond_validation(
        &self,
        request_hash: &str,
        input: CreateAgentValidationResponse,
    ) -> Result<AgentValidationResponse>;

    /// Get latest validation status for a request hash
    fn get_validation_status(&self, request_hash: &str) -> Result<Option<AgentValidationStatus>>;

    /// Get validation summary for an agent
    fn get_summary(
        &self,
        agent_registry: &str,
        agent_id: &str,
        validator_addresses: Option<Vec<String>>,
        tag: Option<String>,
    ) -> Result<ValidationSummary>;

    /// Get all request hashes for an agent
    fn get_agent_validations(&self, agent_registry: &str, agent_id: &str) -> Result<Vec<String>>;

    /// Get all request hashes for a validator
    fn get_validator_requests(&self, validator_address: &str) -> Result<Vec<String>>;
}

// ============================================================================
// A2A Commerce Repository
// ============================================================================

/// A2A (Agent-to-Agent) Commerce repository trait
pub trait A2ACommerceRepository {
    // Quote operations
    /// Create a new quote
    fn create_quote(&self, input: CreateA2AQuote) -> Result<A2AQuote>;

    /// Get quote by ID
    fn get_quote(&self, id: Uuid) -> Result<Option<A2AQuote>>;

    /// Get quote by quote number
    fn get_quote_by_number(&self, quote_number: &str) -> Result<Option<A2AQuote>>;

    /// Update quote status
    fn update_quote_status(&self, id: Uuid, status: QuoteStatus) -> Result<A2AQuote>;

    /// List quotes with filter
    fn list_quotes(&self, filter: A2AQuoteFilter) -> Result<Vec<A2AQuote>>;

    /// Count quotes matching filter
    fn count_quotes(&self, filter: A2AQuoteFilter) -> Result<u64>;

    // Purchase operations
    /// Create a new purchase
    fn create_purchase(&self, input: CreateA2APurchase) -> Result<A2APurchase>;

    /// Get purchase by ID
    fn get_purchase(&self, id: Uuid) -> Result<Option<A2APurchase>>;

    /// Get purchase by purchase number
    fn get_purchase_by_number(&self, purchase_number: &str) -> Result<Option<A2APurchase>>;

    /// Update purchase status
    fn update_purchase_status(&self, id: Uuid, status: PurchaseStatus) -> Result<A2APurchase>;

    /// Link purchase to order
    fn link_purchase_to_order(&self, purchase_id: Uuid, order_id: Uuid) -> Result<A2APurchase>;

    /// Confirm delivery
    fn confirm_delivery(
        &self,
        purchase_id: Uuid,
        signature: &str,
        rating: Option<u8>,
        feedback: Option<&str>,
    ) -> Result<A2APurchase>;

    /// List purchases with filter
    fn list_purchases(&self, filter: A2APurchaseFilter) -> Result<Vec<A2APurchase>>;

    /// Count purchases matching filter
    fn count_purchases(&self, filter: A2APurchaseFilter) -> Result<u64>;
}

// ============================================================================
// Custom Objects Repository
// ============================================================================

/// Custom Objects repository trait (custom states / metaobjects).
///
/// Provides a schema-driven custom data system:
/// - Define types (schemas) with typed fields
/// - Create records (instances) that validate against the schema
pub trait CustomObjectRepository {
    // ------------------------------------------------------------------------
    // Type (schema) operations
    // ------------------------------------------------------------------------

    fn create_type(&self, input: CreateCustomObjectType) -> Result<CustomObjectType>;

    fn get_type(&self, id: Uuid) -> Result<Option<CustomObjectType>>;

    fn get_type_by_handle(&self, handle: &str) -> Result<Option<CustomObjectType>>;

    fn update_type(&self, id: Uuid, input: UpdateCustomObjectType) -> Result<CustomObjectType>;

    fn list_types(&self, filter: CustomObjectTypeFilter) -> Result<Vec<CustomObjectType>>;

    fn delete_type(&self, id: Uuid) -> Result<()>;

    // ------------------------------------------------------------------------
    // Record operations
    // ------------------------------------------------------------------------

    fn create_object(&self, input: CreateCustomObject) -> Result<CustomObject>;

    fn get_object(&self, id: Uuid) -> Result<Option<CustomObject>>;

    fn get_object_by_handle(
        &self,
        type_handle: &str,
        object_handle: &str,
    ) -> Result<Option<CustomObject>>;

    fn update_object(&self, id: Uuid, input: UpdateCustomObject) -> Result<CustomObject>;

    fn list_objects(&self, filter: CustomObjectFilter) -> Result<Vec<CustomObject>>;

    fn delete_object(&self, id: Uuid) -> Result<()>;
}
