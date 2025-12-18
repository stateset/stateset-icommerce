//! Repository traits for data access abstraction
//!
//! These traits define the interface for data persistence.
//! Implementations can be SQLite, PostgreSQL, in-memory, etc.

use crate::errors::Result;
use crate::models::*;
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

    /// Delete an order (soft delete)
    fn delete(&self, id: Uuid) -> Result<()>;

    /// Add item to order
    fn add_item(&self, order_id: Uuid, item: CreateOrderItem) -> Result<OrderItem>;

    /// Remove item from order
    fn remove_item(&self, order_id: Uuid, item_id: Uuid) -> Result<()>;

    /// Count orders matching filter
    fn count(&self, filter: OrderFilter) -> Result<u64>;
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

    /// Release reservation
    fn release_reservation(&self, reservation_id: Uuid) -> Result<()>;

    /// Confirm reservation (convert to allocation)
    fn confirm_reservation(&self, reservation_id: Uuid) -> Result<()>;

    /// List inventory items with filter
    fn list(&self, filter: InventoryFilter) -> Result<Vec<InventoryItem>>;

    /// Get items below reorder point
    fn get_reorder_needed(&self) -> Result<Vec<StockLevel>>;

    /// Record transaction
    fn record_transaction(&self, transaction: InventoryTransaction) -> Result<InventoryTransaction>;

    /// Get transaction history
    fn get_transactions(&self, item_id: i64, limit: u32) -> Result<Vec<InventoryTransaction>>;
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
    fn update_address(&self, address_id: Uuid, input: CreateCustomerAddress) -> Result<CustomerAddress>;

    /// Delete address
    fn delete_address(&self, address_id: Uuid) -> Result<()>;

    /// Set default address
    fn set_default_address(&self, customer_id: Uuid, address_id: Uuid, address_type: AddressType) -> Result<()>;

    /// Count customers matching filter
    fn count(&self, filter: CustomerFilter) -> Result<u64>;
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
    fn add_variant(&self, product_id: Uuid, variant: CreateProductVariant) -> Result<ProductVariant>;

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
    fn update_component(&self, component_id: Uuid, component: CreateBomComponent) -> Result<BomComponent>;

    /// Remove component from BOM
    fn remove_component(&self, component_id: Uuid) -> Result<()>;

    /// Get all components for a BOM
    fn get_components(&self, bom_id: Uuid) -> Result<Vec<BomComponent>>;

    /// Activate a BOM (make it ready for production use)
    fn activate(&self, id: Uuid) -> Result<BillOfMaterials>;

    /// Count BOMs matching filter
    fn count(&self, filter: BomFilter) -> Result<u64>;
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
    fn complete_task(&self, task_id: Uuid, actual_hours: Option<rust_decimal::Decimal>) -> Result<WorkOrderTask>;

    // Material operations
    /// Add material to work order
    fn add_material(&self, work_order_id: Uuid, material: AddWorkOrderMaterial) -> Result<WorkOrderMaterial>;

    /// Consume material
    fn consume_material(&self, material_id: Uuid, quantity: rust_decimal::Decimal) -> Result<WorkOrderMaterial>;

    /// Get materials for work order
    fn get_materials(&self, work_order_id: Uuid) -> Result<Vec<WorkOrderMaterial>>;

    /// Count work orders matching filter
    fn count(&self, filter: WorkOrderFilter) -> Result<u64>;
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
    fn update_item(&self, item_id: Uuid, item: CreatePurchaseOrderItem) -> Result<PurchaseOrderItem>;

    /// Remove item from purchase order
    fn remove_item(&self, item_id: Uuid) -> Result<()>;

    /// Get items for purchase order
    fn get_items(&self, po_id: Uuid) -> Result<Vec<PurchaseOrderItem>>;

    /// Count purchase orders matching filter
    fn count(&self, filter: PurchaseOrderFilter) -> Result<u64>;

    /// Count suppliers matching filter
    fn count_suppliers(&self, filter: SupplierFilter) -> Result<u64>;
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
    fn get_low_stock_items(&self, threshold: Option<rust_decimal::Decimal>) -> Result<Vec<LowStockItem>>;

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
    fn get_demand_forecast(&self, skus: Option<Vec<String>>, days_ahead: u32) -> Result<Vec<DemandForecast>>;

    /// Get revenue forecast
    fn get_revenue_forecast(&self, periods_ahead: u32, granularity: TimeGranularity) -> Result<Vec<RevenueForecast>>;
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
