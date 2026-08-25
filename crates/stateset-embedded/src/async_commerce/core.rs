//! Core commerce accessors: orders, inventory, customers, products, custom objects, returns, shipments, payments.

use super::*;

/// Async order operations.
pub struct AsyncOrders {
    db: Arc<PostgresDatabase>,
    metrics: Metrics,
    #[cfg(feature = "events")]
    event_system: Arc<EventSystem>,
}

impl AsyncOrders {
    pub(crate) const fn new(
        db: Arc<PostgresDatabase>,
        metrics: Metrics,
        #[cfg(feature = "events")] event_system: Arc<EventSystem>,
    ) -> Self {
        Self {
            db,
            metrics,
            #[cfg(feature = "events")]
            event_system,
        }
    }

    #[cfg(feature = "events")]
    fn emit(&self, event: CommerceEvent) {
        self.event_system.emit(event);
    }

    /// Create a new order.
    ///
    /// Mirrors the sync [`Orders::create`](crate::Orders::create) behaviour:
    /// it computes the expected order total via `stateset_pricing`, warns on
    /// pricing drift between the persisted total and the engine total, records
    /// metrics, and emits a [`CommerceEvent::OrderCreated`] event.
    pub async fn create(&self, input: CreateOrder) -> Result<Order> {
        // Validate pricing: compute expected total via pricing engine.
        let pricing_items: Vec<stateset_pricing::LineItem> = input
            .items
            .iter()
            .map(|item| stateset_pricing::LineItem {
                sku: item.sku.clone(),
                name: item.name.clone(),
                unit_price: item.unit_price,
                quantity: item.quantity as u32,
                discount: item.discount.map(stateset_pricing::LineDiscount::FixedAmount),
                tax_rate: None,
            })
            .collect();
        let pricing_input = stateset_pricing::OrderTotalInput {
            items: pricing_items,
            shipping_cost: Decimal::ZERO,
            shipping_tax_rate: None,
            order_discount: None,
            fees: vec![],
            rounding: {
                let minor = stateset_pricing::minor_units_for_currency(
                    &input.currency.unwrap_or_default().to_string(),
                );
                stateset_pricing::RoundingPolicy::new(stateset_pricing::RoundingMode::HalfUp, minor)
            },
        };
        let pricing_total = stateset_pricing::try_compute_order_total(&pricing_input).ok();
        if let Some(ref computed) = pricing_total {
            tracing::debug!(
                computed_total = %computed.grand_total,
                subtotal = %computed.subtotal,
                discount = %computed.total_discount,
                "Pricing engine computed order total"
            );
        }

        let order = self.db.orders().create_async(input).await?;

        // Detect pricing drift: warn if DB total diverges from pricing engine.
        if let Some(computed) = pricing_total {
            let diff = (order.total_amount - computed.grand_total).abs();
            if diff > Decimal::new(1, 2) {
                // Divergence > $0.01
                tracing::warn!(
                    order_id = %order.id,
                    db_total = %order.total_amount,
                    engine_total = %computed.grand_total,
                    diff = %diff,
                    "Pricing drift detected: DB total differs from pricing engine"
                );
            }
        }

        self.metrics.record_order_created(
            &order.customer_id.to_string(),
            order.total_amount.to_f64().unwrap_or(0.0),
        );

        #[cfg(feature = "events")]
        {
            self.emit(CommerceEvent::OrderCreated {
                order_id: order.id,
                customer_id: order.customer_id,
                total_amount: order.total_amount,
                item_count: order.items.len(),
                timestamp: order.created_at,
            });
        }
        Ok(order)
    }

    /// Get an order by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Order>> {
        self.db.orders().get_async(id).await
    }

    /// Get an order by order number.
    pub async fn get_by_number(&self, order_number: &str) -> Result<Option<Order>> {
        self.db.orders().get_by_number_async(order_number).await
    }

    /// Update an order.
    pub async fn update(&self, id: Uuid, input: UpdateOrder) -> Result<Order> {
        self.db.orders().update_async(id, input).await
    }

    /// Update order status.
    pub async fn update_status(&self, id: Uuid, status: OrderStatus) -> Result<Order> {
        let mut tracking_number = None;
        let mut payment_status = None;
        if status == OrderStatus::Shipped {
            if let Some(order) = self.get(id).await? {
                if order.tracking_number.is_none() {
                    tracking_number = Some(format!("AUTO-{}", id));
                }
            }
        }
        if status == OrderStatus::Refunded {
            payment_status = Some(PaymentStatus::Refunded);
        }
        self.update(
            id,
            UpdateOrder {
                status: Some(status),
                payment_status,
                tracking_number,
                ..Default::default()
            },
        )
        .await
    }

    /// List orders with optional filtering.
    pub async fn list(&self, filter: OrderFilter) -> Result<Vec<Order>> {
        self.db.orders().list_async(filter).await
    }

    /// List orders for a specific customer.
    pub async fn list_for_customer(&self, customer_id: Uuid) -> Result<Vec<Order>> {
        self.db
            .orders()
            .list_async(OrderFilter { customer_id: Some(customer_id.into()), ..Default::default() })
            .await
    }

    /// Delete an order.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.orders().delete_async(id).await
    }

    /// Add an item to an order.
    pub async fn add_item(&self, order_id: Uuid, item: CreateOrderItem) -> Result<OrderItem> {
        self.db.orders().add_item_async(order_id, item).await
    }

    /// Remove an item from an order.
    pub async fn remove_item(&self, order_id: Uuid, item_id: Uuid) -> Result<()> {
        self.db.orders().remove_item_async(order_id, item_id).await
    }

    /// Count orders matching a filter.
    pub async fn count(&self, filter: OrderFilter) -> Result<u64> {
        self.db.orders().count_async(filter).await
    }

    /// Cancel an order.
    pub async fn cancel(&self, id: Uuid) -> Result<Order> {
        self.update_status(id, OrderStatus::Cancelled).await
    }

    /// Mark an order as shipped (every remaining unit on every line).
    pub async fn ship(&self, id: Uuid, tracking_number: Option<&str>) -> Result<Order> {
        self.ship_lines(id, tracking_number, None).await
    }

    /// Ship an order, optionally only some units of some lines.
    ///
    /// Async counterpart of [`crate::Orders::ship_lines`].
    pub async fn ship_lines(
        &self,
        id: Uuid,
        tracking_number: Option<&str>,
        lines: Option<Vec<ShipmentLineInput>>,
    ) -> Result<Order> {
        if let Some(order) = self.get(id).await? {
            match order.status {
                OrderStatus::Pending => {
                    self.update_status(id, OrderStatus::Confirmed).await?;
                    self.update_status(id, OrderStatus::Processing).await?;
                }
                OrderStatus::Confirmed => {
                    self.update_status(id, OrderStatus::Processing).await?;
                }
                _ => {}
            }
        }
        let tracking_number = tracking_number.map(|s| s.to_string());
        self.db.orders().ship_async(id, ShipOrder { tracking_number, lines }).await
    }

    /// Mark an order as delivered.
    pub async fn deliver(&self, id: Uuid) -> Result<Order> {
        self.update_status(id, OrderStatus::Delivered).await
    }
}

// ============================================================================
// Async Inventory
// ============================================================================

/// Async inventory operations.
pub struct AsyncInventory {
    db: Arc<PostgresDatabase>,
    metrics: Metrics,
}

impl AsyncInventory {
    pub(crate) const fn new(db: Arc<PostgresDatabase>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    /// Create a new inventory item.
    pub async fn create_item(&self, input: CreateInventoryItem) -> Result<InventoryItem> {
        self.db.inventory().create_item_async(input).await
    }

    /// Get inventory item by ID.
    pub async fn get_item(&self, id: i64) -> Result<Option<InventoryItem>> {
        self.db.inventory().get_item_async(id).await
    }

    /// Get inventory item by SKU.
    pub async fn get_item_by_sku(&self, sku: &str) -> Result<Option<InventoryItem>> {
        self.db.inventory().get_item_by_sku_async(sku).await
    }

    /// Get stock level for a SKU.
    pub async fn get_stock(&self, sku: &str) -> Result<Option<StockLevel>> {
        self.db.inventory().get_stock_async(sku).await
    }

    /// Get balance at a specific location.
    pub async fn get_balance(
        &self,
        item_id: i64,
        location_id: i32,
    ) -> Result<Option<InventoryBalance>> {
        self.db.inventory().get_balance_async(item_id, location_id).await
    }

    /// Adjust inventory quantity.
    pub async fn adjust_inventory(&self, input: AdjustInventory) -> Result<InventoryTransaction> {
        let sku = input.sku.clone();
        let delta = input.quantity.to_f64().unwrap_or(0.0);
        let transaction = self.db.inventory().adjust_async(input).await?;
        self.metrics.record_inventory_adjusted(&sku, delta);
        Ok(transaction)
    }

    /// Convenience method to adjust inventory by SKU.
    pub async fn adjust(
        &self,
        sku: &str,
        quantity: rust_decimal::Decimal,
        reason: &str,
    ) -> Result<InventoryTransaction> {
        self.adjust_inventory(AdjustInventory {
            sku: sku.to_string(),
            location_id: Some(1), // Default location
            quantity,
            reason: reason.to_string(),
            reference_type: None,
            reference_id: None,
        })
        .await
    }

    /// Reserve inventory.
    pub async fn reserve(&self, input: ReserveInventory) -> Result<InventoryReservation> {
        self.db.inventory().reserve_async(input).await
    }

    /// Release a reservation.
    pub async fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.db.inventory().release_reservation_async(reservation_id).await
    }

    /// Confirm a reservation.
    pub async fn confirm_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.db.inventory().confirm_reservation_async(reservation_id).await
    }

    /// List reservations by reference (e.g., order id).
    pub async fn list_reservations_by_reference(
        &self,
        reference_type: &str,
        reference_id: &str,
    ) -> Result<Vec<InventoryReservation>> {
        self.db.inventory().list_reservations_by_reference_async(reference_type, reference_id).await
    }

    /// List inventory items.
    pub async fn list(&self, filter: InventoryFilter) -> Result<Vec<InventoryItem>> {
        self.db.inventory().list_async(filter).await
    }

    /// Get items that need reordering.
    pub async fn get_reorder_needed(&self) -> Result<Vec<StockLevel>> {
        self.db.inventory().get_reorder_needed_async().await
    }

    /// Get transaction history.
    pub async fn get_transactions(
        &self,
        item_id: i64,
        limit: u32,
    ) -> Result<Vec<InventoryTransaction>> {
        self.db.inventory().get_transactions_async(item_id, limit).await
    }
}

// ============================================================================
// Async Customers
// ============================================================================

/// Async customer operations.
pub struct AsyncCustomers {
    db: Arc<PostgresDatabase>,
    metrics: Metrics,
}

impl AsyncCustomers {
    pub(crate) const fn new(db: Arc<PostgresDatabase>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    /// Create a new customer.
    pub async fn create(&self, input: CreateCustomer) -> Result<Customer> {
        let customer = self.db.customers().create_async(input).await?;
        self.metrics.record_customer_created(&customer.id.to_string());
        Ok(customer)
    }

    /// Get customer by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Customer>> {
        self.db.customers().get_async(id.into()).await
    }

    /// Get customer by email.
    pub async fn get_by_email(&self, email: &str) -> Result<Option<Customer>> {
        self.db.customers().get_by_email_async(email).await
    }

    /// Update a customer.
    pub async fn update(&self, id: Uuid, input: UpdateCustomer) -> Result<Customer> {
        self.db.customers().update_async(id.into(), input).await
    }

    /// List customers.
    pub async fn list(&self, filter: CustomerFilter) -> Result<Vec<Customer>> {
        self.db.customers().list_async(filter).await
    }

    /// Delete a customer.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.customers().delete_async(id.into()).await
    }

    /// Add an address to a customer.
    pub async fn add_address(&self, input: CreateCustomerAddress) -> Result<CustomerAddress> {
        self.db.customers().add_address_async(input).await
    }

    /// Get addresses for a customer.
    pub async fn get_addresses(&self, customer_id: Uuid) -> Result<Vec<CustomerAddress>> {
        self.db.customers().get_addresses_async(customer_id.into()).await
    }

    /// Count customers.
    pub async fn count(&self, filter: CustomerFilter) -> Result<u64> {
        self.db.customers().count_async(filter).await
    }
}

// ============================================================================
// Async Products
// ============================================================================

/// Async product operations.
pub struct AsyncProducts {
    db: Arc<PostgresDatabase>,
    metrics: Metrics,
}

impl AsyncProducts {
    pub(crate) const fn new(db: Arc<PostgresDatabase>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    /// Create a new product.
    pub async fn create(&self, input: CreateProduct) -> Result<Product> {
        let product = self.db.products().create_async(input).await?;
        self.metrics.record_product_created(&product.id.to_string());
        Ok(product)
    }

    /// Get product by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Product>> {
        self.db.products().get_async(id.into()).await
    }

    /// Get product by slug.
    pub async fn get_by_slug(&self, slug: &str) -> Result<Option<Product>> {
        self.db.products().get_by_slug_async(slug).await
    }

    /// Update a product.
    pub async fn update(&self, id: Uuid, input: UpdateProduct) -> Result<Product> {
        self.db.products().update_async(id.into(), input).await
    }

    /// List products.
    pub async fn list(&self, filter: ProductFilter) -> Result<Vec<Product>> {
        self.db.products().list_async(filter).await
    }

    /// Delete a product.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.products().delete_async(id.into()).await
    }

    /// Add a variant to a product.
    pub async fn add_variant(
        &self,
        product_id: Uuid,
        variant: CreateProductVariant,
    ) -> Result<ProductVariant> {
        self.db.products().add_variant_public_async(product_id.into(), variant).await
    }

    /// Get variant by ID.
    pub async fn get_variant(&self, id: Uuid) -> Result<Option<ProductVariant>> {
        self.db.products().get_variant_async(id).await
    }

    /// Get variant by SKU.
    pub async fn get_variant_by_sku(&self, sku: &str) -> Result<Option<ProductVariant>> {
        self.db.products().get_variant_by_sku_async(sku).await
    }

    /// Update a variant.
    pub async fn update_variant(
        &self,
        id: Uuid,
        variant: CreateProductVariant,
    ) -> Result<ProductVariant> {
        self.db.products().update_variant_async(id, variant).await
    }

    /// Delete a variant.
    pub async fn delete_variant(&self, id: Uuid) -> Result<()> {
        self.db.products().delete_variant_async(id).await
    }

    /// Get all variants for a product.
    pub async fn get_variants(&self, product_id: Uuid) -> Result<Vec<ProductVariant>> {
        self.db.products().get_variants_async(product_id.into()).await
    }

    /// Count products.
    pub async fn count(&self, filter: ProductFilter) -> Result<u64> {
        self.db.products().count_async(filter).await
    }
}

// ============================================================================
// Async Custom Objects (Custom States / Metaobjects)
// ============================================================================

/// Async custom object operations.
pub struct AsyncCustomObjects {
    db: Arc<PostgresDatabase>,
}

impl AsyncCustomObjects {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    // ------------------------------------------------------------------------
    // Types (schemas)
    // ------------------------------------------------------------------------

    /// Create a new custom object type (schema).
    pub async fn create_type(&self, input: CreateCustomObjectType) -> Result<CustomObjectType> {
        self.db.custom_objects().create_type_async(input).await
    }

    /// Get a custom object type by ID.
    pub async fn get_type(&self, id: Uuid) -> Result<Option<CustomObjectType>> {
        self.db.custom_objects().get_type_async(id).await
    }

    /// Get a custom object type by handle.
    pub async fn get_type_by_handle(&self, handle: &str) -> Result<Option<CustomObjectType>> {
        self.db.custom_objects().get_type_by_handle_async(handle).await
    }

    /// Update a custom object type.
    pub async fn update_type(
        &self,
        id: Uuid,
        input: UpdateCustomObjectType,
    ) -> Result<CustomObjectType> {
        self.db.custom_objects().update_type_async(id, input).await
    }

    /// List custom object types.
    pub async fn list_types(
        &self,
        filter: CustomObjectTypeFilter,
    ) -> Result<Vec<CustomObjectType>> {
        self.db.custom_objects().list_types_async(filter).await
    }

    /// Delete a custom object type.
    pub async fn delete_type(&self, id: Uuid) -> Result<()> {
        self.db.custom_objects().delete_type_async(id).await
    }

    // ------------------------------------------------------------------------
    // Records
    // ------------------------------------------------------------------------

    /// Create a new custom object record.
    pub async fn create_object(&self, input: CreateCustomObject) -> Result<CustomObject> {
        self.db.custom_objects().create_object_async(input).await
    }

    /// Get a custom object record by ID.
    pub async fn get_object(&self, id: Uuid) -> Result<Option<CustomObject>> {
        self.db.custom_objects().get_object_async(id).await
    }

    /// Get a custom object record by type handle and object handle.
    pub async fn get_object_by_handle(
        &self,
        type_handle: &str,
        object_handle: &str,
    ) -> Result<Option<CustomObject>> {
        self.db.custom_objects().get_object_by_handle_async(type_handle, object_handle).await
    }

    /// Update a custom object record.
    pub async fn update_object(&self, id: Uuid, input: UpdateCustomObject) -> Result<CustomObject> {
        self.db.custom_objects().update_object_async(id, input).await
    }

    /// List custom object records.
    pub async fn list_objects(&self, filter: CustomObjectFilter) -> Result<Vec<CustomObject>> {
        self.db.custom_objects().list_objects_async(filter).await
    }

    /// Delete a custom object record.
    pub async fn delete_object(&self, id: Uuid) -> Result<()> {
        self.db.custom_objects().delete_object_async(id).await
    }
}

// ============================================================================
// Async Returns
// ============================================================================

/// Async return operations.
pub struct AsyncReturns {
    db: Arc<PostgresDatabase>,
    metrics: Metrics,
}

impl AsyncReturns {
    pub(crate) const fn new(db: Arc<PostgresDatabase>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    /// Create a new return.
    pub async fn create(&self, input: CreateReturn) -> Result<Return> {
        let ret = self.db.returns().create_async(input).await?;
        self.metrics.record_return_requested(&ret.id.to_string());
        Ok(ret)
    }

    /// Get return by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Return>> {
        self.db.returns().get_async(id).await
    }

    /// Update a return.
    pub async fn update(&self, id: Uuid, input: UpdateReturn) -> Result<Return> {
        self.db.returns().update_async(id, input).await
    }

    /// List returns.
    pub async fn list(&self, filter: ReturnFilter) -> Result<Vec<Return>> {
        self.db.returns().list_async(filter).await
    }

    /// Approve a return.
    pub async fn approve(&self, id: Uuid) -> Result<Return> {
        self.db.returns().approve_async(id).await
    }

    /// Reject a return.
    pub async fn reject(&self, id: Uuid, reason: &str) -> Result<Return> {
        self.db.returns().reject_async(id, reason).await
    }

    /// Complete a return.
    pub async fn complete(&self, id: Uuid) -> Result<Return> {
        self.db.returns().complete_async(id).await
    }

    /// Cancel a return.
    pub async fn cancel(&self, id: Uuid) -> Result<Return> {
        self.db.returns().cancel_async(id).await
    }

    /// Count returns.
    pub async fn count(&self, filter: ReturnFilter) -> Result<u64> {
        self.db.returns().count_async(filter).await
    }
}

// ============================================================================
// Async Shipments
// ============================================================================

/// Async shipment operations.
pub struct AsyncShipments {
    db: Arc<PostgresDatabase>,
    metrics: Metrics,
}

impl AsyncShipments {
    pub(crate) const fn new(db: Arc<PostgresDatabase>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    /// Create a new shipment.
    pub async fn create(&self, input: CreateShipment) -> Result<Shipment> {
        let shipment = self.db.shipments().create_async(input).await?;
        self.metrics.record_shipment_created(&shipment.id.to_string());
        Ok(shipment)
    }

    /// Get shipment by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Shipment>> {
        self.db.shipments().get_async(id).await
    }

    /// Get shipment by shipment number.
    pub async fn get_by_number(&self, shipment_number: &str) -> Result<Option<Shipment>> {
        self.db.shipments().get_by_number_async(shipment_number).await
    }

    /// Get shipment by tracking number.
    pub async fn get_by_tracking(&self, tracking_number: &str) -> Result<Option<Shipment>> {
        self.db.shipments().get_by_tracking_async(tracking_number).await
    }

    /// Update a shipment.
    pub async fn update(&self, id: Uuid, input: UpdateShipment) -> Result<Shipment> {
        self.db.shipments().update_async(id, input).await
    }

    /// List shipments.
    pub async fn list(&self, filter: ShipmentFilter) -> Result<Vec<Shipment>> {
        self.db.shipments().list_async(filter).await
    }

    /// Get shipments for an order.
    pub async fn for_order(&self, order_id: Uuid) -> Result<Vec<Shipment>> {
        self.db.shipments().for_order_async(order_id).await
    }

    /// Delete a shipment.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.shipments().delete_async(id).await
    }

    /// Mark shipment as processing.
    pub async fn mark_processing(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().mark_processing_async(id).await
    }

    /// Mark shipment as ready.
    pub async fn mark_ready(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().mark_ready_async(id).await
    }

    /// Ship with optional tracking number.
    pub async fn ship(&self, id: Uuid, tracking_number: Option<String>) -> Result<Shipment> {
        self.db.shipments().ship_async(id, tracking_number).await
    }

    /// Mark shipment as in transit.
    pub async fn mark_in_transit(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().mark_in_transit_async(id).await
    }

    /// Mark shipment as out for delivery.
    pub async fn mark_out_for_delivery(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().mark_out_for_delivery_async(id).await
    }

    /// Mark shipment as delivered.
    pub async fn mark_delivered(&self, id: Uuid) -> Result<Shipment> {
        let shipment = self.db.shipments().mark_delivered_async(id).await?;
        self.metrics.record_shipment_delivered(&shipment.id.to_string());
        Ok(shipment)
    }

    /// Mark shipment as failed.
    pub async fn mark_failed(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().mark_failed_async(id).await
    }

    /// Put shipment on hold.
    pub async fn hold(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().hold_async(id).await
    }

    /// Cancel shipment.
    pub async fn cancel(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().cancel_async(id).await
    }

    /// Add item to shipment.
    pub async fn add_item(
        &self,
        shipment_id: Uuid,
        item: CreateShipmentItem,
    ) -> Result<ShipmentItem> {
        self.db.shipments().add_item_async(shipment_id, item).await
    }

    /// Remove item from shipment.
    pub async fn remove_item(&self, item_id: Uuid) -> Result<()> {
        self.db.shipments().remove_item_async(item_id).await
    }

    /// Get items in shipment.
    pub async fn get_items(&self, shipment_id: Uuid) -> Result<Vec<ShipmentItem>> {
        self.db.shipments().get_items_async(shipment_id).await
    }

    /// Add tracking event.
    pub async fn add_event(
        &self,
        shipment_id: Uuid,
        event: AddShipmentEvent,
    ) -> Result<ShipmentEvent> {
        self.db.shipments().add_event_async(shipment_id, event).await
    }

    /// Get tracking events.
    pub async fn get_events(&self, shipment_id: Uuid) -> Result<Vec<ShipmentEvent>> {
        self.db.shipments().get_events_async(shipment_id).await
    }

    /// Count shipments.
    pub async fn count(&self, filter: ShipmentFilter) -> Result<u64> {
        self.db.shipments().count_async(filter).await
    }
}

// ============================================================================
// Async Payments
// ============================================================================

/// Async payment operations.
pub struct AsyncPayments {
    db: Arc<PostgresDatabase>,
    metrics: Metrics,
}

impl AsyncPayments {
    pub(crate) const fn new(db: Arc<PostgresDatabase>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    /// Create a new payment.
    pub async fn create(&self, input: CreatePayment) -> Result<Payment> {
        self.db.payments().create_async(input).await
    }

    /// Get payment by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Payment>> {
        self.db.payments().get_async(id).await
    }

    /// Get payment by payment number.
    pub async fn get_by_number(&self, payment_number: &str) -> Result<Option<Payment>> {
        self.db.payments().get_by_number_async(payment_number).await
    }

    /// Get payment by external ID.
    pub async fn get_by_external_id(&self, external_id: &str) -> Result<Option<Payment>> {
        self.db.payments().get_by_external_id_async(external_id).await
    }

    /// Update a payment.
    pub async fn update(&self, id: Uuid, input: UpdatePayment) -> Result<Payment> {
        self.db.payments().update_async(id, input).await
    }

    /// List payments.
    pub async fn list(&self, filter: PaymentFilter) -> Result<Vec<Payment>> {
        self.db.payments().list_async(filter).await
    }

    /// Get payments for an order.
    pub async fn for_order(&self, order_id: Uuid) -> Result<Vec<Payment>> {
        self.db.payments().for_order_async(order_id).await
    }

    /// Get payments for an invoice.
    pub async fn for_invoice(&self, invoice_id: Uuid) -> Result<Vec<Payment>> {
        self.db.payments().for_invoice_async(invoice_id).await
    }

    /// Mark payment as processing.
    pub async fn mark_processing(&self, id: Uuid) -> Result<Payment> {
        self.db.payments().mark_processing_async(id).await
    }

    /// Mark payment as completed.
    pub async fn mark_completed(&self, id: Uuid) -> Result<Payment> {
        let payment = self.db.payments().mark_completed_async(id).await?;
        self.metrics.record_payment_completed(
            &payment.id.to_string(),
            payment.amount.to_f64().unwrap_or(0.0),
        );
        Ok(payment)
    }

    /// Mark payment as failed.
    pub async fn mark_failed(&self, id: Uuid, reason: &str, code: Option<&str>) -> Result<Payment> {
        self.db.payments().mark_failed_async(id, reason, code).await
    }

    /// Cancel payment.
    pub async fn cancel(&self, id: Uuid) -> Result<Payment> {
        self.db.payments().cancel_async(id).await
    }

    /// Create a refund.
    pub async fn create_refund(&self, input: CreateRefund) -> Result<Refund> {
        self.db.payments().create_refund_async(input).await
    }

    /// Get refund by ID.
    pub async fn get_refund(&self, id: Uuid) -> Result<Option<Refund>> {
        self.db.payments().get_refund_async(id).await
    }

    /// Get refunds for a payment.
    pub async fn get_refunds(&self, payment_id: Uuid) -> Result<Vec<Refund>> {
        self.db.payments().get_refunds_async(payment_id).await
    }

    /// Complete a refund.
    pub async fn complete_refund(&self, id: Uuid) -> Result<Refund> {
        self.db.payments().complete_refund_async(id).await
    }

    /// Fail a refund.
    pub async fn fail_refund(&self, id: Uuid, reason: &str) -> Result<Refund> {
        self.db.payments().fail_refund_async(id, reason).await
    }

    /// Create a payment method.
    pub async fn create_payment_method(&self, input: CreatePaymentMethod) -> Result<PaymentMethod> {
        self.db.payments().create_payment_method_async(input).await
    }

    /// Get payment methods for a customer.
    pub async fn get_payment_methods(&self, customer_id: Uuid) -> Result<Vec<PaymentMethod>> {
        self.db.payments().get_payment_methods_async(customer_id).await
    }

    /// Delete a payment method.
    pub async fn delete_payment_method(&self, id: Uuid) -> Result<()> {
        self.db.payments().delete_payment_method_async(id).await
    }

    /// Set default payment method.
    pub async fn set_default_payment_method(
        &self,
        customer_id: Uuid,
        method_id: Uuid,
    ) -> Result<()> {
        self.db.payments().set_default_payment_method_async(customer_id, method_id).await
    }

    /// Count payments.
    pub async fn count(&self, filter: PaymentFilter) -> Result<u64> {
        self.db.payments().count_async(filter).await
    }
}

// ============================================================================
// Async Warranties
// ============================================================================
