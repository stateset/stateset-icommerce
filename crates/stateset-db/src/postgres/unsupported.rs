//! Placeholder repository implementations for PostgreSQL.
//!
//! The PostgreSQL backend is still under development. These implementations prevent runtime panics
//! by returning a structured `CommerceError` when a domain repository is not yet supported.

// This module intentionally implements a large number of repository traits as stubs.
// Keep builds warning-free until the PostgreSQL backend is fully implemented.
#![allow(unused_variables)]

use chrono::{DateTime, NaiveDate, Utc};
use stateset_core::*;
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub struct UnsupportedPostgresRepository {
    domain: &'static str,
}

impl UnsupportedPostgresRepository {
    pub fn new(domain: &'static str) -> Self {
        Self { domain }
    }

    fn not_supported<T>(&self) -> Result<T> {
        Err(CommerceError::NotPermitted(format!(
            "PostgreSQL backend: {} repository not yet implemented",
            self.domain
        )))
    }
}

impl ShipmentRepository for UnsupportedPostgresRepository {
    fn create(&self, _input: CreateShipment) -> Result<Shipment> {
        self.not_supported()
    }
    fn get(&self, _id: Uuid) -> Result<Option<Shipment>> {
        self.not_supported()
    }
    fn get_by_number(&self, _shipment_number: &str) -> Result<Option<Shipment>> {
        self.not_supported()
    }
    fn get_by_tracking(&self, _tracking_number: &str) -> Result<Option<Shipment>> {
        self.not_supported()
    }
    fn update(&self, _id: Uuid, _input: UpdateShipment) -> Result<Shipment> {
        self.not_supported()
    }
    fn list(&self, _filter: ShipmentFilter) -> Result<Vec<Shipment>> {
        self.not_supported()
    }
    fn for_order(&self, _order_id: Uuid) -> Result<Vec<Shipment>> {
        self.not_supported()
    }
    fn delete(&self, _id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn mark_processing(&self, _id: Uuid) -> Result<Shipment> {
        self.not_supported()
    }
    fn mark_ready(&self, _id: Uuid) -> Result<Shipment> {
        self.not_supported()
    }
    fn ship(&self, _id: Uuid, _tracking_number: Option<String>) -> Result<Shipment> {
        self.not_supported()
    }
    fn mark_in_transit(&self, _id: Uuid) -> Result<Shipment> {
        self.not_supported()
    }
    fn mark_out_for_delivery(&self, _id: Uuid) -> Result<Shipment> {
        self.not_supported()
    }
    fn mark_delivered(&self, _id: Uuid) -> Result<Shipment> {
        self.not_supported()
    }
    fn mark_failed(&self, _id: Uuid) -> Result<Shipment> {
        self.not_supported()
    }
    fn hold(&self, _id: Uuid) -> Result<Shipment> {
        self.not_supported()
    }
    fn cancel(&self, _id: Uuid) -> Result<Shipment> {
        self.not_supported()
    }
    fn add_item(&self, _shipment_id: Uuid, _item: CreateShipmentItem) -> Result<ShipmentItem> {
        self.not_supported()
    }
    fn remove_item(&self, _item_id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn get_items(&self, _shipment_id: Uuid) -> Result<Vec<ShipmentItem>> {
        self.not_supported()
    }
    fn add_event(&self, _shipment_id: Uuid, _event: AddShipmentEvent) -> Result<ShipmentEvent> {
        self.not_supported()
    }
    fn get_events(&self, _shipment_id: Uuid) -> Result<Vec<ShipmentEvent>> {
        self.not_supported()
    }
    fn count(&self, _filter: ShipmentFilter) -> Result<u64> {
        self.not_supported()
    }

    fn create_batch(&self, _inputs: Vec<CreateShipment>) -> Result<BatchResult<Shipment>> {
        self.not_supported()
    }
    fn create_batch_atomic(&self, _inputs: Vec<CreateShipment>) -> Result<Vec<Shipment>> {
        self.not_supported()
    }
    fn update_batch(&self, _updates: Vec<(Uuid, UpdateShipment)>) -> Result<BatchResult<Shipment>> {
        self.not_supported()
    }
    fn update_batch_atomic(&self, _updates: Vec<(Uuid, UpdateShipment)>) -> Result<Vec<Shipment>> {
        self.not_supported()
    }
    fn delete_batch(&self, _ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        self.not_supported()
    }
    fn delete_batch_atomic(&self, _ids: Vec<Uuid>) -> Result<()> {
        self.not_supported()
    }
    fn get_batch(&self, _ids: Vec<Uuid>) -> Result<Vec<Shipment>> {
        self.not_supported()
    }
}

impl PaymentRepository for UnsupportedPostgresRepository {
    fn create(&self, _input: CreatePayment) -> Result<Payment> {
        self.not_supported()
    }
    fn get(&self, _id: Uuid) -> Result<Option<Payment>> {
        self.not_supported()
    }
    fn get_by_number(&self, _payment_number: &str) -> Result<Option<Payment>> {
        self.not_supported()
    }
    fn get_by_external_id(&self, _external_id: &str) -> Result<Option<Payment>> {
        self.not_supported()
    }
    fn update(&self, _id: Uuid, _input: UpdatePayment) -> Result<Payment> {
        self.not_supported()
    }
    fn list(&self, _filter: PaymentFilter) -> Result<Vec<Payment>> {
        self.not_supported()
    }
    fn for_order(&self, _order_id: Uuid) -> Result<Vec<Payment>> {
        self.not_supported()
    }
    fn for_invoice(&self, _invoice_id: Uuid) -> Result<Vec<Payment>> {
        self.not_supported()
    }
    fn mark_processing(&self, _id: Uuid) -> Result<Payment> {
        self.not_supported()
    }
    fn mark_completed(&self, _id: Uuid) -> Result<Payment> {
        self.not_supported()
    }
    fn mark_failed(&self, _id: Uuid, _reason: &str, _code: Option<&str>) -> Result<Payment> {
        self.not_supported()
    }
    fn cancel(&self, _id: Uuid) -> Result<Payment> {
        self.not_supported()
    }
    fn create_refund(&self, _input: CreateRefund) -> Result<Refund> {
        self.not_supported()
    }
    fn get_refund(&self, _id: Uuid) -> Result<Option<Refund>> {
        self.not_supported()
    }
    fn get_refunds(&self, _payment_id: Uuid) -> Result<Vec<Refund>> {
        self.not_supported()
    }
    fn complete_refund(&self, _id: Uuid) -> Result<Refund> {
        self.not_supported()
    }
    fn fail_refund(&self, _id: Uuid, _reason: &str) -> Result<Refund> {
        self.not_supported()
    }
    fn create_payment_method(&self, _input: CreatePaymentMethod) -> Result<PaymentMethod> {
        self.not_supported()
    }
    fn get_payment_methods(&self, _customer_id: Uuid) -> Result<Vec<PaymentMethod>> {
        self.not_supported()
    }
    fn delete_payment_method(&self, _id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn set_default_payment_method(&self, _customer_id: Uuid, _method_id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn count(&self, _filter: PaymentFilter) -> Result<u64> {
        self.not_supported()
    }

    fn create_batch(&self, _inputs: Vec<CreatePayment>) -> Result<BatchResult<Payment>> {
        self.not_supported()
    }
    fn create_batch_atomic(&self, _inputs: Vec<CreatePayment>) -> Result<Vec<Payment>> {
        self.not_supported()
    }
    fn update_batch(&self, _updates: Vec<(Uuid, UpdatePayment)>) -> Result<BatchResult<Payment>> {
        self.not_supported()
    }
    fn update_batch_atomic(&self, _updates: Vec<(Uuid, UpdatePayment)>) -> Result<Vec<Payment>> {
        self.not_supported()
    }
    fn delete_batch(&self, _ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        self.not_supported()
    }
    fn delete_batch_atomic(&self, _ids: Vec<Uuid>) -> Result<()> {
        self.not_supported()
    }
    fn get_batch(&self, _ids: Vec<Uuid>) -> Result<Vec<Payment>> {
        self.not_supported()
    }
}

impl WarrantyRepository for UnsupportedPostgresRepository {
    fn create(&self, _input: CreateWarranty) -> Result<Warranty> {
        self.not_supported()
    }
    fn get(&self, _id: Uuid) -> Result<Option<Warranty>> {
        self.not_supported()
    }
    fn get_by_number(&self, _warranty_number: &str) -> Result<Option<Warranty>> {
        self.not_supported()
    }
    fn get_by_serial(&self, _serial_number: &str) -> Result<Option<Warranty>> {
        self.not_supported()
    }
    fn update(&self, _id: Uuid, _input: UpdateWarranty) -> Result<Warranty> {
        self.not_supported()
    }
    fn list(&self, _filter: WarrantyFilter) -> Result<Vec<Warranty>> {
        self.not_supported()
    }
    fn for_customer(&self, _customer_id: Uuid) -> Result<Vec<Warranty>> {
        self.not_supported()
    }
    fn for_order(&self, _order_id: Uuid) -> Result<Vec<Warranty>> {
        self.not_supported()
    }
    fn void(&self, _id: Uuid) -> Result<Warranty> {
        self.not_supported()
    }
    fn expire(&self, _id: Uuid) -> Result<Warranty> {
        self.not_supported()
    }
    fn transfer(&self, _id: Uuid, _new_customer_id: Uuid) -> Result<Warranty> {
        self.not_supported()
    }
    fn create_claim(&self, _input: CreateWarrantyClaim) -> Result<WarrantyClaim> {
        self.not_supported()
    }
    fn get_claim(&self, _id: Uuid) -> Result<Option<WarrantyClaim>> {
        self.not_supported()
    }
    fn get_claim_by_number(&self, _claim_number: &str) -> Result<Option<WarrantyClaim>> {
        self.not_supported()
    }
    fn update_claim(&self, _id: Uuid, _input: UpdateWarrantyClaim) -> Result<WarrantyClaim> {
        self.not_supported()
    }
    fn list_claims(&self, _filter: WarrantyClaimFilter) -> Result<Vec<WarrantyClaim>> {
        self.not_supported()
    }
    fn get_claims(&self, _warranty_id: Uuid) -> Result<Vec<WarrantyClaim>> {
        self.not_supported()
    }
    fn approve_claim(&self, _id: Uuid) -> Result<WarrantyClaim> {
        self.not_supported()
    }
    fn deny_claim(&self, _id: Uuid, _reason: &str) -> Result<WarrantyClaim> {
        self.not_supported()
    }
    fn complete_claim(&self, _id: Uuid, _resolution: ClaimResolution) -> Result<WarrantyClaim> {
        self.not_supported()
    }
    fn cancel_claim(&self, _id: Uuid) -> Result<WarrantyClaim> {
        self.not_supported()
    }
    fn count(&self, _filter: WarrantyFilter) -> Result<u64> {
        self.not_supported()
    }
    fn count_claims(&self, _filter: WarrantyClaimFilter) -> Result<u64> {
        self.not_supported()
    }

    fn create_batch(&self, _inputs: Vec<CreateWarranty>) -> Result<BatchResult<Warranty>> {
        self.not_supported()
    }
    fn create_batch_atomic(&self, _inputs: Vec<CreateWarranty>) -> Result<Vec<Warranty>> {
        self.not_supported()
    }
    fn update_batch(&self, _updates: Vec<(Uuid, UpdateWarranty)>) -> Result<BatchResult<Warranty>> {
        self.not_supported()
    }
    fn update_batch_atomic(&self, _updates: Vec<(Uuid, UpdateWarranty)>) -> Result<Vec<Warranty>> {
        self.not_supported()
    }
    fn delete_batch(&self, _ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        self.not_supported()
    }
    fn delete_batch_atomic(&self, _ids: Vec<Uuid>) -> Result<()> {
        self.not_supported()
    }
    fn get_batch(&self, _ids: Vec<Uuid>) -> Result<Vec<Warranty>> {
        self.not_supported()
    }
}

impl PurchaseOrderRepository for UnsupportedPostgresRepository {
    fn create_supplier(&self, _input: CreateSupplier) -> Result<Supplier> {
        self.not_supported()
    }
    fn get_supplier(&self, _id: Uuid) -> Result<Option<Supplier>> {
        self.not_supported()
    }
    fn get_supplier_by_code(&self, _code: &str) -> Result<Option<Supplier>> {
        self.not_supported()
    }
    fn update_supplier(&self, _id: Uuid, _input: UpdateSupplier) -> Result<Supplier> {
        self.not_supported()
    }
    fn list_suppliers(&self, _filter: SupplierFilter) -> Result<Vec<Supplier>> {
        self.not_supported()
    }
    fn delete_supplier(&self, _id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn create(&self, _input: CreatePurchaseOrder) -> Result<PurchaseOrder> {
        self.not_supported()
    }
    fn get(&self, _id: Uuid) -> Result<Option<PurchaseOrder>> {
        self.not_supported()
    }
    fn get_by_number(&self, _po_number: &str) -> Result<Option<PurchaseOrder>> {
        self.not_supported()
    }
    fn update(&self, _id: Uuid, _input: UpdatePurchaseOrder) -> Result<PurchaseOrder> {
        self.not_supported()
    }
    fn list(&self, _filter: PurchaseOrderFilter) -> Result<Vec<PurchaseOrder>> {
        self.not_supported()
    }
    fn for_supplier(&self, _supplier_id: Uuid) -> Result<Vec<PurchaseOrder>> {
        self.not_supported()
    }
    fn delete(&self, _id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn submit_for_approval(&self, _id: Uuid) -> Result<PurchaseOrder> {
        self.not_supported()
    }
    fn approve(&self, _id: Uuid, _approved_by: &str) -> Result<PurchaseOrder> {
        self.not_supported()
    }
    fn send(&self, _id: Uuid) -> Result<PurchaseOrder> {
        self.not_supported()
    }
    fn acknowledge(&self, _id: Uuid, _supplier_reference: Option<&str>) -> Result<PurchaseOrder> {
        self.not_supported()
    }
    fn hold(&self, _id: Uuid) -> Result<PurchaseOrder> {
        self.not_supported()
    }
    fn cancel(&self, _id: Uuid) -> Result<PurchaseOrder> {
        self.not_supported()
    }
    fn receive(&self, _id: Uuid, _items: ReceivePurchaseOrderItems) -> Result<PurchaseOrder> {
        self.not_supported()
    }
    fn complete(&self, _id: Uuid) -> Result<PurchaseOrder> {
        self.not_supported()
    }
    fn add_item(&self, _po_id: Uuid, _item: CreatePurchaseOrderItem) -> Result<PurchaseOrderItem> {
        self.not_supported()
    }
    fn update_item(
        &self,
        _item_id: Uuid,
        _item: CreatePurchaseOrderItem,
    ) -> Result<PurchaseOrderItem> {
        self.not_supported()
    }
    fn remove_item(&self, _item_id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn get_items(&self, _po_id: Uuid) -> Result<Vec<PurchaseOrderItem>> {
        self.not_supported()
    }
    fn count(&self, _filter: PurchaseOrderFilter) -> Result<u64> {
        self.not_supported()
    }
    fn count_suppliers(&self, _filter: SupplierFilter) -> Result<u64> {
        self.not_supported()
    }

    fn create_batch(
        &self,
        _inputs: Vec<CreatePurchaseOrder>,
    ) -> Result<BatchResult<PurchaseOrder>> {
        self.not_supported()
    }
    fn create_batch_atomic(&self, _inputs: Vec<CreatePurchaseOrder>) -> Result<Vec<PurchaseOrder>> {
        self.not_supported()
    }
    fn update_batch(
        &self,
        _updates: Vec<(Uuid, UpdatePurchaseOrder)>,
    ) -> Result<BatchResult<PurchaseOrder>> {
        self.not_supported()
    }
    fn update_batch_atomic(
        &self,
        _updates: Vec<(Uuid, UpdatePurchaseOrder)>,
    ) -> Result<Vec<PurchaseOrder>> {
        self.not_supported()
    }
    fn delete_batch(&self, _ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        self.not_supported()
    }
    fn delete_batch_atomic(&self, _ids: Vec<Uuid>) -> Result<()> {
        self.not_supported()
    }
    fn get_batch(&self, _ids: Vec<Uuid>) -> Result<Vec<PurchaseOrder>> {
        self.not_supported()
    }
}

impl InvoiceRepository for UnsupportedPostgresRepository {
    fn create(&self, _input: CreateInvoice) -> Result<Invoice> {
        self.not_supported()
    }
    fn get(&self, _id: Uuid) -> Result<Option<Invoice>> {
        self.not_supported()
    }
    fn get_by_number(&self, _invoice_number: &str) -> Result<Option<Invoice>> {
        self.not_supported()
    }
    fn update(&self, _id: Uuid, _input: UpdateInvoice) -> Result<Invoice> {
        self.not_supported()
    }
    fn list(&self, _filter: InvoiceFilter) -> Result<Vec<Invoice>> {
        self.not_supported()
    }
    fn for_customer(&self, _customer_id: Uuid) -> Result<Vec<Invoice>> {
        self.not_supported()
    }
    fn for_order(&self, _order_id: Uuid) -> Result<Vec<Invoice>> {
        self.not_supported()
    }
    fn delete(&self, _id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn send(&self, _id: Uuid) -> Result<Invoice> {
        self.not_supported()
    }
    fn mark_viewed(&self, _id: Uuid) -> Result<Invoice> {
        self.not_supported()
    }
    fn record_payment(&self, _id: Uuid, _payment: RecordInvoicePayment) -> Result<Invoice> {
        self.not_supported()
    }
    fn void(&self, _id: Uuid) -> Result<Invoice> {
        self.not_supported()
    }
    fn write_off(&self, _id: Uuid) -> Result<Invoice> {
        self.not_supported()
    }
    fn dispute(&self, _id: Uuid) -> Result<Invoice> {
        self.not_supported()
    }
    fn add_item(&self, _invoice_id: Uuid, _item: CreateInvoiceItem) -> Result<InvoiceItem> {
        self.not_supported()
    }
    fn update_item(&self, _item_id: Uuid, _item: CreateInvoiceItem) -> Result<InvoiceItem> {
        self.not_supported()
    }
    fn remove_item(&self, _item_id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn get_items(&self, _invoice_id: Uuid) -> Result<Vec<InvoiceItem>> {
        self.not_supported()
    }
    fn recalculate(&self, _id: Uuid) -> Result<Invoice> {
        self.not_supported()
    }
    fn get_overdue(&self) -> Result<Vec<Invoice>> {
        self.not_supported()
    }
    fn count(&self, _filter: InvoiceFilter) -> Result<u64> {
        self.not_supported()
    }

    fn create_batch(&self, _inputs: Vec<CreateInvoice>) -> Result<BatchResult<Invoice>> {
        self.not_supported()
    }
    fn create_batch_atomic(&self, _inputs: Vec<CreateInvoice>) -> Result<Vec<Invoice>> {
        self.not_supported()
    }
    fn update_batch(&self, _updates: Vec<(Uuid, UpdateInvoice)>) -> Result<BatchResult<Invoice>> {
        self.not_supported()
    }
    fn update_batch_atomic(&self, _updates: Vec<(Uuid, UpdateInvoice)>) -> Result<Vec<Invoice>> {
        self.not_supported()
    }
    fn delete_batch(&self, _ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        self.not_supported()
    }
    fn delete_batch_atomic(&self, _ids: Vec<Uuid>) -> Result<()> {
        self.not_supported()
    }
    fn get_batch(&self, _ids: Vec<Uuid>) -> Result<Vec<Invoice>> {
        self.not_supported()
    }
}

impl CartRepository for UnsupportedPostgresRepository {
    fn create(&self, _input: CreateCart) -> Result<Cart> {
        self.not_supported()
    }
    fn get(&self, _id: Uuid) -> Result<Option<Cart>> {
        self.not_supported()
    }
    fn get_by_number(&self, _cart_number: &str) -> Result<Option<Cart>> {
        self.not_supported()
    }
    fn update(&self, _id: Uuid, _input: UpdateCart) -> Result<Cart> {
        self.not_supported()
    }
    fn list(&self, _filter: CartFilter) -> Result<Vec<Cart>> {
        self.not_supported()
    }
    fn for_customer(&self, _customer_id: Uuid) -> Result<Vec<Cart>> {
        self.not_supported()
    }
    fn delete(&self, _id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn add_item(&self, _cart_id: Uuid, _item: AddCartItem) -> Result<CartItem> {
        self.not_supported()
    }
    fn update_item(&self, _item_id: Uuid, _input: UpdateCartItem) -> Result<CartItem> {
        self.not_supported()
    }
    fn remove_item(&self, _item_id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn get_items(&self, _cart_id: Uuid) -> Result<Vec<CartItem>> {
        self.not_supported()
    }
    fn clear_items(&self, _cart_id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn set_shipping_address(&self, _id: Uuid, _address: CartAddress) -> Result<Cart> {
        self.not_supported()
    }
    fn set_billing_address(&self, _id: Uuid, _address: CartAddress) -> Result<Cart> {
        self.not_supported()
    }
    fn set_shipping(&self, _id: Uuid, _shipping: SetCartShipping) -> Result<Cart> {
        self.not_supported()
    }
    fn get_shipping_rates(&self, _id: Uuid) -> Result<Vec<ShippingRate>> {
        self.not_supported()
    }
    fn set_payment(&self, _id: Uuid, _payment: SetCartPayment) -> Result<Cart> {
        self.not_supported()
    }
    fn set_x402_payment(&self, _id: Uuid, _payment: SetCartX402Payment) -> Result<Cart> {
        self.not_supported()
    }
    fn complete_with_x402(&self, _id: Uuid, _payee_address: &str) -> Result<X402CheckoutResult> {
        self.not_supported()
    }
    fn apply_discount(&self, _id: Uuid, _coupon_code: &str) -> Result<Cart> {
        self.not_supported()
    }
    fn remove_discount(&self, _id: Uuid) -> Result<Cart> {
        self.not_supported()
    }
    fn mark_ready_for_payment(&self, _id: Uuid) -> Result<Cart> {
        self.not_supported()
    }
    fn begin_checkout(&self, _id: Uuid) -> Result<Cart> {
        self.not_supported()
    }
    fn complete(&self, _id: Uuid) -> Result<CheckoutResult> {
        self.not_supported()
    }
    fn cancel(&self, _id: Uuid) -> Result<Cart> {
        self.not_supported()
    }
    fn abandon(&self, _id: Uuid) -> Result<Cart> {
        self.not_supported()
    }
    fn expire(&self, _id: Uuid) -> Result<Cart> {
        self.not_supported()
    }
    fn reserve_inventory(&self, _id: Uuid) -> Result<Cart> {
        self.not_supported()
    }
    fn release_inventory(&self, _id: Uuid) -> Result<Cart> {
        self.not_supported()
    }
    fn recalculate(&self, _id: Uuid) -> Result<Cart> {
        self.not_supported()
    }
    fn set_tax(&self, _id: Uuid, _tax_amount: rust_decimal::Decimal) -> Result<Cart> {
        self.not_supported()
    }
    fn get_abandoned(&self) -> Result<Vec<Cart>> {
        self.not_supported()
    }
    fn get_expired(&self) -> Result<Vec<Cart>> {
        self.not_supported()
    }
    fn count(&self, _filter: CartFilter) -> Result<u64> {
        self.not_supported()
    }

    fn create_batch(&self, _inputs: Vec<CreateCart>) -> Result<BatchResult<Cart>> {
        self.not_supported()
    }
    fn create_batch_atomic(&self, _inputs: Vec<CreateCart>) -> Result<Vec<Cart>> {
        self.not_supported()
    }
    fn update_batch(&self, _updates: Vec<(Uuid, UpdateCart)>) -> Result<BatchResult<Cart>> {
        self.not_supported()
    }
    fn update_batch_atomic(&self, _updates: Vec<(Uuid, UpdateCart)>) -> Result<Vec<Cart>> {
        self.not_supported()
    }
    fn delete_batch(&self, _ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        self.not_supported()
    }
    fn delete_batch_atomic(&self, _ids: Vec<Uuid>) -> Result<()> {
        self.not_supported()
    }
    fn get_batch(&self, _ids: Vec<Uuid>) -> Result<Vec<Cart>> {
        self.not_supported()
    }
}

impl AnalyticsRepository for UnsupportedPostgresRepository {
    fn get_sales_summary(&self, _query: AnalyticsQuery) -> Result<SalesSummary> {
        self.not_supported()
    }
    fn get_sales_summary_batch(&self, _queries: Vec<AnalyticsQuery>) -> Result<Vec<SalesSummary>> {
        self.not_supported()
    }
    fn get_revenue_by_period(&self, _query: AnalyticsQuery) -> Result<Vec<RevenueByPeriod>> {
        self.not_supported()
    }
    fn get_top_products(&self, _query: AnalyticsQuery) -> Result<Vec<TopProduct>> {
        self.not_supported()
    }
    fn get_product_performance(&self, _query: AnalyticsQuery) -> Result<Vec<ProductPerformance>> {
        self.not_supported()
    }
    fn get_customer_metrics(&self, _query: AnalyticsQuery) -> Result<CustomerMetrics> {
        self.not_supported()
    }
    fn get_top_customers(&self, _query: AnalyticsQuery) -> Result<Vec<TopCustomer>> {
        self.not_supported()
    }
    fn get_inventory_health(&self) -> Result<InventoryHealth> {
        self.not_supported()
    }
    fn get_low_stock_items(
        &self,
        _threshold: Option<rust_decimal::Decimal>,
    ) -> Result<Vec<LowStockItem>> {
        self.not_supported()
    }
    fn get_inventory_movement(&self, _query: AnalyticsQuery) -> Result<Vec<InventoryMovement>> {
        self.not_supported()
    }
    fn get_order_status_breakdown(&self, _query: AnalyticsQuery) -> Result<OrderStatusBreakdown> {
        self.not_supported()
    }
    fn get_fulfillment_metrics(&self, _query: AnalyticsQuery) -> Result<FulfillmentMetrics> {
        self.not_supported()
    }
    fn get_return_metrics(&self, _query: AnalyticsQuery) -> Result<ReturnMetrics> {
        self.not_supported()
    }
    fn get_demand_forecast(
        &self,
        _skus: Option<Vec<String>>,
        _days_ahead: u32,
    ) -> Result<Vec<DemandForecast>> {
        self.not_supported()
    }
    fn get_revenue_forecast(
        &self,
        _periods_ahead: u32,
        _granularity: TimeGranularity,
    ) -> Result<Vec<RevenueForecast>> {
        self.not_supported()
    }
}

impl CurrencyRepository for UnsupportedPostgresRepository {
    fn get_rate(&self, _from: Currency, _to: Currency) -> Result<Option<ExchangeRate>> {
        self.not_supported()
    }
    fn get_rates_for(&self, _base: Currency) -> Result<Vec<ExchangeRate>> {
        self.not_supported()
    }
    fn list_rates(&self, _filter: ExchangeRateFilter) -> Result<Vec<ExchangeRate>> {
        self.not_supported()
    }
    fn set_rate(&self, _input: SetExchangeRate) -> Result<ExchangeRate> {
        self.not_supported()
    }
    fn set_rates(&self, _rates: Vec<SetExchangeRate>) -> Result<Vec<ExchangeRate>> {
        self.not_supported()
    }
    fn delete_rate(&self, _id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn convert(&self, _input: ConvertCurrency) -> Result<ConversionResult> {
        self.not_supported()
    }
    fn get_settings(&self) -> Result<StoreCurrencySettings> {
        self.not_supported()
    }
    fn update_settings(&self, _settings: StoreCurrencySettings) -> Result<StoreCurrencySettings> {
        self.not_supported()
    }

    fn set_rates_atomic(&self, _rates: Vec<SetExchangeRate>) -> Result<Vec<ExchangeRate>> {
        self.not_supported()
    }
    fn delete_rates_batch(&self, _ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        self.not_supported()
    }
    fn delete_rates_atomic(&self, _ids: Vec<Uuid>) -> Result<()> {
        self.not_supported()
    }
    fn get_rates_batch(&self, _pairs: Vec<(Currency, Currency)>) -> Result<Vec<ExchangeRate>> {
        self.not_supported()
    }
}

impl TaxRepository for UnsupportedPostgresRepository {
    fn create_jurisdiction(&self, _input: CreateTaxJurisdiction) -> Result<TaxJurisdiction> {
        self.not_supported()
    }
    fn get_jurisdiction(&self, _id: Uuid) -> Result<Option<TaxJurisdiction>> {
        self.not_supported()
    }
    fn get_jurisdiction_by_code(&self, _code: &str) -> Result<Option<TaxJurisdiction>> {
        self.not_supported()
    }
    fn list_jurisdictions(&self, _filter: TaxJurisdictionFilter) -> Result<Vec<TaxJurisdiction>> {
        self.not_supported()
    }
    fn create_rate(&self, _input: CreateTaxRate) -> Result<TaxRate> {
        self.not_supported()
    }
    fn get_rate(&self, _id: Uuid) -> Result<Option<TaxRate>> {
        self.not_supported()
    }
    fn list_rates(&self, _filter: TaxRateFilter) -> Result<Vec<TaxRate>> {
        self.not_supported()
    }
    fn get_rates_for_address(
        &self,
        _address: &TaxAddress,
        _category: ProductTaxCategory,
        _date: chrono::NaiveDate,
    ) -> Result<Vec<TaxRate>> {
        self.not_supported()
    }
    fn create_exemption(&self, _input: CreateTaxExemption) -> Result<TaxExemption> {
        self.not_supported()
    }
    fn get_exemption(&self, _id: Uuid) -> Result<Option<TaxExemption>> {
        self.not_supported()
    }
    fn get_customer_exemptions(&self, _customer_id: Uuid) -> Result<Vec<TaxExemption>> {
        self.not_supported()
    }
    fn get_settings(&self) -> Result<TaxSettings> {
        self.not_supported()
    }
    fn update_settings(&self, _settings: TaxSettings) -> Result<TaxSettings> {
        self.not_supported()
    }
    fn calculate_tax(&self, _request: TaxCalculationRequest) -> Result<TaxCalculationResult> {
        self.not_supported()
    }
    fn save_calculation(
        &self,
        _result: &TaxCalculationResult,
        _order_id: Option<Uuid>,
        _cart_id: Option<Uuid>,
        _customer_id: Option<Uuid>,
        _address: &TaxAddress,
        _currency: &str,
    ) -> Result<()> {
        self.not_supported()
    }
}

impl PromotionRepository for UnsupportedPostgresRepository {
    fn create(&self, _input: CreatePromotion) -> Result<Promotion> {
        self.not_supported()
    }
    fn get(&self, _id: Uuid) -> Result<Option<Promotion>> {
        self.not_supported()
    }
    fn get_by_code(&self, _code: &str) -> Result<Option<Promotion>> {
        self.not_supported()
    }
    fn list(&self, _filter: PromotionFilter) -> Result<Vec<Promotion>> {
        self.not_supported()
    }
    fn update(&self, _id: Uuid, _input: UpdatePromotion) -> Result<Promotion> {
        self.not_supported()
    }
    fn delete(&self, _id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn activate(&self, _id: Uuid) -> Result<Promotion> {
        self.not_supported()
    }
    fn deactivate(&self, _id: Uuid) -> Result<Promotion> {
        self.not_supported()
    }
    fn create_coupon(&self, _input: CreateCouponCode) -> Result<CouponCode> {
        self.not_supported()
    }
    fn get_coupon(&self, _id: Uuid) -> Result<Option<CouponCode>> {
        self.not_supported()
    }
    fn get_coupon_by_code(&self, _code: &str) -> Result<Option<CouponCode>> {
        self.not_supported()
    }
    fn list_coupons(&self, _filter: CouponFilter) -> Result<Vec<CouponCode>> {
        self.not_supported()
    }
    fn apply_promotions(&self, _request: ApplyPromotionsRequest) -> Result<ApplyPromotionsResult> {
        self.not_supported()
    }
    fn record_usage(
        &self,
        _promotion_id: Uuid,
        _coupon_id: Option<Uuid>,
        _customer_id: Option<Uuid>,
        _order_id: Option<Uuid>,
        _cart_id: Option<Uuid>,
        _discount_amount: rust_decimal::Decimal,
        _currency: &str,
    ) -> Result<PromotionUsage> {
        self.not_supported()
    }
}

impl SubscriptionRepository for UnsupportedPostgresRepository {
    fn create_plan(&self, _input: CreateSubscriptionPlan) -> Result<SubscriptionPlan> {
        self.not_supported()
    }
    fn get_plan(&self, _id: Uuid) -> Result<Option<SubscriptionPlan>> {
        self.not_supported()
    }
    fn get_plan_by_code(&self, _code: &str) -> Result<Option<SubscriptionPlan>> {
        self.not_supported()
    }
    fn list_plans(&self, _filter: SubscriptionPlanFilter) -> Result<Vec<SubscriptionPlan>> {
        self.not_supported()
    }
    fn update_plan(&self, _id: Uuid, _input: UpdateSubscriptionPlan) -> Result<SubscriptionPlan> {
        self.not_supported()
    }
    fn activate_plan(&self, _id: Uuid) -> Result<SubscriptionPlan> {
        self.not_supported()
    }
    fn archive_plan(&self, _id: Uuid) -> Result<SubscriptionPlan> {
        self.not_supported()
    }
    fn create_subscription(&self, _input: CreateSubscription) -> Result<Subscription> {
        self.not_supported()
    }
    fn get_subscription(&self, _id: Uuid) -> Result<Option<Subscription>> {
        self.not_supported()
    }
    fn get_subscription_by_number(&self, _number: &str) -> Result<Option<Subscription>> {
        self.not_supported()
    }
    fn list_subscriptions(&self, _filter: SubscriptionFilter) -> Result<Vec<Subscription>> {
        self.not_supported()
    }
    fn update_subscription(&self, _id: Uuid, _input: UpdateSubscription) -> Result<Subscription> {
        self.not_supported()
    }
    fn cancel_subscription(&self, _id: Uuid, _input: CancelSubscription) -> Result<Subscription> {
        self.not_supported()
    }
    fn pause_subscription(&self, _id: Uuid, _input: PauseSubscription) -> Result<Subscription> {
        self.not_supported()
    }
    fn resume_subscription(&self, _id: Uuid) -> Result<Subscription> {
        self.not_supported()
    }
    fn create_billing_cycle(&self, _input: CreateBillingCycle) -> Result<BillingCycle> {
        self.not_supported()
    }
    fn get_billing_cycle(&self, _id: Uuid) -> Result<Option<BillingCycle>> {
        self.not_supported()
    }
    fn list_billing_cycles(&self, _filter: BillingCycleFilter) -> Result<Vec<BillingCycle>> {
        self.not_supported()
    }
    fn update_billing_cycle_status(
        &self,
        _id: Uuid,
        _status: BillingCycleStatus,
    ) -> Result<BillingCycle> {
        self.not_supported()
    }
    fn skip_billing_cycle(&self, _id: Uuid, _input: SkipBillingCycle) -> Result<Subscription> {
        self.not_supported()
    }
    fn record_event(
        &self,
        _subscription_id: Uuid,
        _event_type: SubscriptionEventType,
        _notes: Option<String>,
    ) -> Result<SubscriptionEvent> {
        self.not_supported()
    }
    fn get_subscription_events(&self, _subscription_id: Uuid) -> Result<Vec<SubscriptionEvent>> {
        self.not_supported()
    }
}

impl QualityRepository for UnsupportedPostgresRepository {
    fn create_inspection(&self, input: CreateInspection) -> Result<Inspection> {
        self.not_supported()
    }
    fn get_inspection(&self, id: Uuid) -> Result<Option<Inspection>> {
        self.not_supported()
    }
    fn get_inspection_by_number(&self, number: &str) -> Result<Option<Inspection>> {
        self.not_supported()
    }
    fn update_inspection(&self, id: Uuid, input: UpdateInspection) -> Result<Inspection> {
        self.not_supported()
    }
    fn list_inspections(&self, filter: InspectionFilter) -> Result<Vec<Inspection>> {
        self.not_supported()
    }
    fn delete_inspection(&self, id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn start_inspection(&self, id: Uuid) -> Result<Inspection> {
        self.not_supported()
    }
    fn complete_inspection(&self, id: Uuid) -> Result<Inspection> {
        self.not_supported()
    }
    fn record_inspection_result(&self, input: RecordInspectionResult) -> Result<InspectionItem> {
        self.not_supported()
    }
    fn get_inspection_items(&self, inspection_id: Uuid) -> Result<Vec<InspectionItem>> {
        self.not_supported()
    }
    fn count_inspections(&self, filter: InspectionFilter) -> Result<u64> {
        self.not_supported()
    }
    fn create_ncr(&self, input: CreateNonConformance) -> Result<NonConformance> {
        self.not_supported()
    }
    fn get_ncr(&self, id: Uuid) -> Result<Option<NonConformance>> {
        self.not_supported()
    }
    fn get_ncr_by_number(&self, number: &str) -> Result<Option<NonConformance>> {
        self.not_supported()
    }
    fn update_ncr(&self, id: Uuid, input: UpdateNonConformance) -> Result<NonConformance> {
        self.not_supported()
    }
    fn list_ncrs(&self, filter: NonConformanceFilter) -> Result<Vec<NonConformance>> {
        self.not_supported()
    }
    fn close_ncr(&self, id: Uuid) -> Result<NonConformance> {
        self.not_supported()
    }
    fn cancel_ncr(&self, id: Uuid) -> Result<NonConformance> {
        self.not_supported()
    }
    fn count_ncrs(&self, filter: NonConformanceFilter) -> Result<u64> {
        self.not_supported()
    }
    fn create_hold(&self, input: CreateQualityHold) -> Result<QualityHold> {
        self.not_supported()
    }
    fn get_hold(&self, id: Uuid) -> Result<Option<QualityHold>> {
        self.not_supported()
    }
    fn list_holds(&self, filter: QualityHoldFilter) -> Result<Vec<QualityHold>> {
        self.not_supported()
    }
    fn release_hold(&self, id: Uuid, input: ReleaseQualityHold) -> Result<QualityHold> {
        self.not_supported()
    }
    fn get_active_holds_for_sku(&self, sku: &str) -> Result<Vec<QualityHold>> {
        self.not_supported()
    }
    fn get_active_holds_for_lot(&self, lot_number: &str) -> Result<Vec<QualityHold>> {
        self.not_supported()
    }
    fn count_active_holds(&self) -> Result<u64> {
        self.not_supported()
    }
    fn create_defect_code(&self, input: CreateDefectCode) -> Result<DefectCode> {
        self.not_supported()
    }
    fn get_defect_code(&self, code: &str) -> Result<Option<DefectCode>> {
        self.not_supported()
    }
    fn list_defect_codes(&self, category: Option<&str>) -> Result<Vec<DefectCode>> {
        self.not_supported()
    }
    fn deactivate_defect_code(&self, id: Uuid) -> Result<()> {
        self.not_supported()
    }
}

impl LotRepository for UnsupportedPostgresRepository {
    fn create(&self, input: CreateLot) -> Result<Lot> {
        self.not_supported()
    }
    fn get(&self, id: Uuid) -> Result<Option<Lot>> {
        self.not_supported()
    }
    fn get_by_number(&self, lot_number: &str) -> Result<Option<Lot>> {
        self.not_supported()
    }
    fn update(&self, id: Uuid, input: UpdateLot) -> Result<Lot> {
        self.not_supported()
    }
    fn list(&self, filter: LotFilter) -> Result<Vec<Lot>> {
        self.not_supported()
    }
    fn delete(&self, id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn adjust(&self, input: AdjustLot) -> Result<LotTransaction> {
        self.not_supported()
    }
    fn consume(&self, input: ConsumeLot) -> Result<LotTransaction> {
        self.not_supported()
    }
    fn reserve(&self, input: ReserveLot) -> Result<Uuid> {
        self.not_supported()
    }
    fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn confirm_reservation(&self, reservation_id: Uuid) -> Result<LotTransaction> {
        self.not_supported()
    }
    fn transfer(&self, input: TransferLot) -> Result<LotTransaction> {
        self.not_supported()
    }
    fn split(&self, input: SplitLot) -> Result<Lot> {
        self.not_supported()
    }
    fn merge(&self, input: MergeLots) -> Result<Lot> {
        self.not_supported()
    }
    fn quarantine(&self, id: Uuid, reason: &str) -> Result<Lot> {
        self.not_supported()
    }
    fn release_quarantine(&self, id: Uuid) -> Result<Lot> {
        self.not_supported()
    }
    fn get_transactions(&self, lot_id: Uuid, limit: u32) -> Result<Vec<LotTransaction>> {
        self.not_supported()
    }
    fn get_quantity_at_location(
        &self,
        lot_id: Uuid,
        location_id: i32,
    ) -> Result<Option<rust_decimal::Decimal>> {
        self.not_supported()
    }
    fn get_lot_locations(&self, lot_id: Uuid) -> Result<Vec<LotLocation>> {
        self.not_supported()
    }
    fn add_certificate(&self, input: AddLotCertificate) -> Result<LotCertificate> {
        self.not_supported()
    }
    fn get_certificates(&self, lot_id: Uuid) -> Result<Vec<LotCertificate>> {
        self.not_supported()
    }
    fn delete_certificate(&self, certificate_id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn get_expiring_lots(&self, days: i32) -> Result<Vec<Lot>> {
        self.not_supported()
    }
    fn get_expired_lots(&self) -> Result<Vec<Lot>> {
        self.not_supported()
    }
    fn get_available_lots_for_sku(&self, sku: &str) -> Result<Vec<Lot>> {
        self.not_supported()
    }
    fn trace(&self, lot_id: Uuid) -> Result<TraceabilityResult> {
        self.not_supported()
    }
    fn count(&self, filter: LotFilter) -> Result<u64> {
        self.not_supported()
    }
    fn create_batch(&self, inputs: Vec<CreateLot>) -> Result<BatchResult<Lot>> {
        self.not_supported()
    }
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Lot>> {
        self.not_supported()
    }
}

impl SerialRepository for UnsupportedPostgresRepository {
    fn create(&self, input: CreateSerialNumber) -> Result<SerialNumber> {
        self.not_supported()
    }
    fn create_bulk(&self, input: CreateSerialNumbersBulk) -> Result<Vec<SerialNumber>> {
        self.not_supported()
    }
    fn get(&self, id: Uuid) -> Result<Option<SerialNumber>> {
        self.not_supported()
    }
    fn get_by_serial(&self, serial: &str) -> Result<Option<SerialNumber>> {
        self.not_supported()
    }
    fn update(&self, id: Uuid, input: UpdateSerialNumber) -> Result<SerialNumber> {
        self.not_supported()
    }
    fn list(&self, filter: SerialFilter) -> Result<Vec<SerialNumber>> {
        self.not_supported()
    }
    fn delete(&self, id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn change_status(&self, input: ChangeSerialStatus) -> Result<SerialNumber> {
        self.not_supported()
    }
    fn reserve(&self, input: ReserveSerialNumber) -> Result<SerialReservation> {
        self.not_supported()
    }
    fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn confirm_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn move_serial(&self, input: MoveSerial) -> Result<SerialNumber> {
        self.not_supported()
    }
    fn transfer_ownership(&self, input: TransferSerialOwnership) -> Result<SerialNumber> {
        self.not_supported()
    }
    fn mark_sold(
        &self,
        id: Uuid,
        customer_id: Uuid,
        order_id: Option<Uuid>,
    ) -> Result<SerialNumber> {
        self.not_supported()
    }
    fn mark_shipped(&self, id: Uuid, shipment_id: Uuid) -> Result<SerialNumber> {
        self.not_supported()
    }
    fn mark_returned(&self, id: Uuid, return_id: Uuid) -> Result<SerialNumber> {
        self.not_supported()
    }
    fn activate(&self, id: Uuid) -> Result<SerialNumber> {
        self.not_supported()
    }
    fn quarantine(&self, id: Uuid, reason: &str) -> Result<SerialNumber> {
        self.not_supported()
    }
    fn release_quarantine(&self, id: Uuid) -> Result<SerialNumber> {
        self.not_supported()
    }
    fn scrap(&self, id: Uuid, reason: &str) -> Result<SerialNumber> {
        self.not_supported()
    }
    fn get_history(
        &self,
        serial_id: Uuid,
        filter: SerialHistoryFilter,
    ) -> Result<Vec<SerialHistory>> {
        self.not_supported()
    }
    fn lookup(&self, serial: &str) -> Result<Option<SerialLookupResult>> {
        self.not_supported()
    }
    fn validate(&self, serial: &str) -> Result<SerialValidation> {
        self.not_supported()
    }
    fn get_available_for_sku(&self, sku: &str, limit: u32) -> Result<Vec<SerialNumber>> {
        self.not_supported()
    }
    fn get_for_lot(&self, lot_id: Uuid) -> Result<Vec<SerialNumber>> {
        self.not_supported()
    }
    fn get_for_customer(&self, customer_id: Uuid) -> Result<Vec<SerialNumber>> {
        self.not_supported()
    }
    fn count(&self, filter: SerialFilter) -> Result<u64> {
        self.not_supported()
    }
    fn create_batch(&self, inputs: Vec<CreateSerialNumber>) -> Result<BatchResult<SerialNumber>> {
        self.not_supported()
    }
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<SerialNumber>> {
        self.not_supported()
    }
    fn get_batch_by_serial(&self, serials: Vec<String>) -> Result<Vec<SerialNumber>> {
        self.not_supported()
    }
}

impl WarehouseRepository for UnsupportedPostgresRepository {
    fn create_warehouse(&self, input: CreateWarehouse) -> Result<Warehouse> {
        self.not_supported()
    }
    fn get_warehouse(&self, id: i32) -> Result<Option<Warehouse>> {
        self.not_supported()
    }
    fn get_warehouse_by_code(&self, code: &str) -> Result<Option<Warehouse>> {
        self.not_supported()
    }
    fn update_warehouse(&self, id: i32, input: UpdateWarehouse) -> Result<Warehouse> {
        self.not_supported()
    }
    fn list_warehouses(&self, filter: WarehouseFilter) -> Result<Vec<Warehouse>> {
        self.not_supported()
    }
    fn delete_warehouse(&self, id: i32) -> Result<()> {
        self.not_supported()
    }
    fn count_warehouses(&self, filter: WarehouseFilter) -> Result<u64> {
        self.not_supported()
    }
    fn create_zone(&self, input: CreateZone) -> Result<Zone> {
        self.not_supported()
    }
    fn get_zone(&self, id: i32) -> Result<Option<Zone>> {
        self.not_supported()
    }
    fn get_zones(&self, warehouse_id: i32) -> Result<Vec<Zone>> {
        self.not_supported()
    }
    fn update_zone(&self, id: i32, input: UpdateZone) -> Result<Zone> {
        self.not_supported()
    }
    fn delete_zone(&self, id: i32) -> Result<()> {
        self.not_supported()
    }
    fn create_location(&self, input: CreateLocation) -> Result<Location> {
        self.not_supported()
    }
    fn get_location(&self, id: i32) -> Result<Option<Location>> {
        self.not_supported()
    }
    fn get_location_by_code(&self, warehouse_id: i32, code: &str) -> Result<Option<Location>> {
        self.not_supported()
    }
    fn update_location(&self, id: i32, input: UpdateLocation) -> Result<Location> {
        self.not_supported()
    }
    fn list_locations(&self, filter: LocationFilter) -> Result<Vec<Location>> {
        self.not_supported()
    }
    fn delete_location(&self, id: i32) -> Result<()> {
        self.not_supported()
    }
    fn count_locations(&self, filter: LocationFilter) -> Result<u64> {
        self.not_supported()
    }
    fn get_locations_for_warehouse(&self, warehouse_id: i32) -> Result<Vec<Location>> {
        self.not_supported()
    }
    fn get_pickable_locations(&self, warehouse_id: i32, sku: &str) -> Result<Vec<Location>> {
        self.not_supported()
    }
    fn get_receivable_locations(&self, warehouse_id: i32) -> Result<Vec<Location>> {
        self.not_supported()
    }
    fn get_location_inventory(&self, location_id: i32) -> Result<Vec<LocationInventory>> {
        self.not_supported()
    }
    fn get_inventory_for_sku(
        &self,
        warehouse_id: i32,
        sku: &str,
    ) -> Result<Vec<LocationInventory>> {
        self.not_supported()
    }
    fn adjust_inventory(&self, input: AdjustLocationInventory) -> Result<LocationInventory> {
        self.not_supported()
    }
    fn move_inventory(&self, input: MoveInventory) -> Result<LocationMovement> {
        self.not_supported()
    }
    fn list_location_inventory(
        &self,
        filter: LocationInventoryFilter,
    ) -> Result<Vec<LocationInventory>> {
        self.not_supported()
    }
    fn get_movements(&self, filter: MovementFilter) -> Result<Vec<LocationMovement>> {
        self.not_supported()
    }
    fn count_movements(&self, filter: MovementFilter) -> Result<u64> {
        self.not_supported()
    }
    fn create_locations_batch(&self, inputs: Vec<CreateLocation>) -> Result<BatchResult<Location>> {
        self.not_supported()
    }
    fn get_locations_batch(&self, ids: Vec<i32>) -> Result<Vec<Location>> {
        self.not_supported()
    }
}

impl ReceivingRepository for UnsupportedPostgresRepository {
    fn create_receipt(&self, input: CreateReceipt) -> Result<Receipt> {
        self.not_supported()
    }
    fn get_receipt(&self, id: Uuid) -> Result<Option<Receipt>> {
        self.not_supported()
    }
    fn get_receipt_by_number(&self, number: &str) -> Result<Option<Receipt>> {
        self.not_supported()
    }
    fn update_receipt(&self, id: Uuid, input: UpdateReceipt) -> Result<Receipt> {
        self.not_supported()
    }
    fn list_receipts(&self, filter: ReceiptFilter) -> Result<Vec<Receipt>> {
        self.not_supported()
    }
    fn delete_receipt(&self, id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn start_receiving(&self, id: Uuid) -> Result<Receipt> {
        self.not_supported()
    }
    fn receive_items(&self, input: ReceiveItems) -> Result<Receipt> {
        self.not_supported()
    }
    fn complete_receiving(&self, id: Uuid) -> Result<Receipt> {
        self.not_supported()
    }
    fn cancel_receipt(&self, id: Uuid) -> Result<Receipt> {
        self.not_supported()
    }
    fn get_receipt_items(&self, receipt_id: Uuid) -> Result<Vec<ReceiptItem>> {
        self.not_supported()
    }
    fn count_receipts(&self, filter: ReceiptFilter) -> Result<u64> {
        self.not_supported()
    }
    fn create_put_away(&self, input: CreatePutAway) -> Result<PutAway> {
        self.not_supported()
    }
    fn get_put_away(&self, id: Uuid) -> Result<Option<PutAway>> {
        self.not_supported()
    }
    fn list_put_aways(&self, filter: PutAwayFilter) -> Result<Vec<PutAway>> {
        self.not_supported()
    }
    fn assign_put_away(&self, id: Uuid, assigned_to: &str) -> Result<PutAway> {
        self.not_supported()
    }
    fn start_put_away(&self, id: Uuid) -> Result<PutAway> {
        self.not_supported()
    }
    fn complete_put_away(&self, input: CompletePutAway) -> Result<PutAway> {
        self.not_supported()
    }
    fn cancel_put_away(&self, id: Uuid) -> Result<PutAway> {
        self.not_supported()
    }
    fn get_pending_put_aways(&self, receipt_id: Uuid) -> Result<Vec<PutAway>> {
        self.not_supported()
    }
    fn count_put_aways(&self, filter: PutAwayFilter) -> Result<u64> {
        self.not_supported()
    }
    fn create_receipt_from_po(&self, po_id: Uuid, warehouse_id: i32) -> Result<Receipt> {
        self.not_supported()
    }
    fn create_receipts_batch(&self, inputs: Vec<CreateReceipt>) -> Result<BatchResult<Receipt>> {
        self.not_supported()
    }
    fn get_receipts_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Receipt>> {
        self.not_supported()
    }
}

impl FulfillmentRepository for UnsupportedPostgresRepository {
    fn create_wave(&self, input: CreateWave) -> Result<Wave> {
        self.not_supported()
    }
    fn get_wave(&self, id: Uuid) -> Result<Option<Wave>> {
        self.not_supported()
    }
    fn get_wave_by_number(&self, number: &str) -> Result<Option<Wave>> {
        self.not_supported()
    }
    fn list_waves(&self, filter: WaveFilter) -> Result<Vec<Wave>> {
        self.not_supported()
    }
    fn release_wave(&self, id: Uuid) -> Result<Wave> {
        self.not_supported()
    }
    fn complete_wave(&self, id: Uuid) -> Result<Wave> {
        self.not_supported()
    }
    fn cancel_wave(&self, id: Uuid) -> Result<Wave> {
        self.not_supported()
    }
    fn get_wave_orders(&self, wave_id: Uuid) -> Result<Vec<Uuid>> {
        self.not_supported()
    }
    fn count_waves(&self, filter: WaveFilter) -> Result<u64> {
        self.not_supported()
    }
    fn create_pick(&self, input: CreatePickTask) -> Result<PickTask> {
        self.not_supported()
    }
    fn get_pick(&self, id: Uuid) -> Result<Option<PickTask>> {
        self.not_supported()
    }
    fn list_picks(&self, filter: PickTaskFilter) -> Result<Vec<PickTask>> {
        self.not_supported()
    }
    fn assign_pick(&self, id: Uuid, assigned_to: &str) -> Result<PickTask> {
        self.not_supported()
    }
    fn start_pick(&self, id: Uuid) -> Result<PickTask> {
        self.not_supported()
    }
    fn complete_pick(&self, input: CompletePick) -> Result<PickTask> {
        self.not_supported()
    }
    fn report_short(
        &self,
        id: Uuid,
        short_qty: rust_decimal::Decimal,
        reason: &str,
    ) -> Result<PickTask> {
        self.not_supported()
    }
    fn cancel_pick(&self, id: Uuid) -> Result<PickTask> {
        self.not_supported()
    }
    fn get_picks_for_order(&self, order_id: Uuid) -> Result<Vec<PickTask>> {
        self.not_supported()
    }
    fn get_picks_for_wave(&self, wave_id: Uuid) -> Result<Vec<PickTask>> {
        self.not_supported()
    }
    fn count_picks(&self, filter: PickTaskFilter) -> Result<u64> {
        self.not_supported()
    }
    fn create_pack(&self, input: CreatePackTask) -> Result<PackTask> {
        self.not_supported()
    }
    fn get_pack(&self, id: Uuid) -> Result<Option<PackTask>> {
        self.not_supported()
    }
    fn list_packs(&self, filter: PackTaskFilter) -> Result<Vec<PackTask>> {
        self.not_supported()
    }
    fn assign_pack(&self, id: Uuid, assigned_to: &str) -> Result<PackTask> {
        self.not_supported()
    }
    fn start_pack(&self, id: Uuid) -> Result<PackTask> {
        self.not_supported()
    }
    fn complete_pack(&self, id: Uuid) -> Result<PackTask> {
        self.not_supported()
    }
    fn add_carton(&self, input: AddCarton) -> Result<Carton> {
        self.not_supported()
    }
    fn add_carton_item(&self, input: AddCartonItem) -> Result<CartonItem> {
        self.not_supported()
    }
    fn get_cartons(&self, pack_task_id: Uuid) -> Result<Vec<Carton>> {
        self.not_supported()
    }
    fn get_carton_items(&self, carton_id: Uuid) -> Result<Vec<CartonItem>> {
        self.not_supported()
    }
    fn mark_label_printed(&self, carton_id: Uuid) -> Result<Carton> {
        self.not_supported()
    }
    fn cancel_pack(&self, id: Uuid) -> Result<PackTask> {
        self.not_supported()
    }
    fn count_packs(&self, filter: PackTaskFilter) -> Result<u64> {
        self.not_supported()
    }
    fn create_ship(&self, input: CreateShipTask) -> Result<ShipTask> {
        self.not_supported()
    }
    fn get_ship(&self, id: Uuid) -> Result<Option<ShipTask>> {
        self.not_supported()
    }
    fn list_ships(&self, filter: ShipTaskFilter) -> Result<Vec<ShipTask>> {
        self.not_supported()
    }
    fn assign_ship(&self, id: Uuid, assigned_to: &str) -> Result<ShipTask> {
        self.not_supported()
    }
    fn print_label(&self, id: Uuid, label_url: &str) -> Result<ShipTask> {
        self.not_supported()
    }
    fn complete_ship(&self, input: CompleteShip) -> Result<ShipTask> {
        self.not_supported()
    }
    fn cancel_ship(&self, id: Uuid) -> Result<ShipTask> {
        self.not_supported()
    }
    fn count_ships(&self, filter: ShipTaskFilter) -> Result<u64> {
        self.not_supported()
    }
    fn create_picks_for_order(&self, order_id: Uuid, warehouse_id: i32) -> Result<Vec<PickTask>> {
        self.not_supported()
    }
    fn is_order_ready_to_pack(&self, order_id: Uuid) -> Result<bool> {
        self.not_supported()
    }
    fn is_order_ready_to_ship(&self, order_id: Uuid) -> Result<bool> {
        self.not_supported()
    }
    fn create_waves_batch(&self, inputs: Vec<CreateWave>) -> Result<BatchResult<Wave>> {
        self.not_supported()
    }
    fn get_picks_batch(&self, ids: Vec<Uuid>) -> Result<Vec<PickTask>> {
        self.not_supported()
    }
}

impl AccountsPayableRepository for UnsupportedPostgresRepository {
    fn create_bill(&self, input: CreateBill) -> Result<Bill> {
        self.not_supported()
    }
    fn get_bill(&self, id: Uuid) -> Result<Option<Bill>> {
        self.not_supported()
    }
    fn get_bill_by_number(&self, number: &str) -> Result<Option<Bill>> {
        self.not_supported()
    }
    fn update_bill(&self, id: Uuid, input: UpdateBill) -> Result<Bill> {
        self.not_supported()
    }
    fn list_bills(&self, filter: BillFilter) -> Result<Vec<Bill>> {
        self.not_supported()
    }
    fn delete_bill(&self, id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn approve_bill(&self, id: Uuid) -> Result<Bill> {
        self.not_supported()
    }
    fn cancel_bill(&self, id: Uuid) -> Result<Bill> {
        self.not_supported()
    }
    fn dispute_bill(&self, id: Uuid) -> Result<Bill> {
        self.not_supported()
    }
    fn get_bill_items(&self, bill_id: Uuid) -> Result<Vec<BillItem>> {
        self.not_supported()
    }
    fn add_bill_item(&self, bill_id: Uuid, item: CreateBillItem) -> Result<BillItem> {
        self.not_supported()
    }
    fn remove_bill_item(&self, item_id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn count_bills(&self, filter: BillFilter) -> Result<u64> {
        self.not_supported()
    }
    fn get_overdue_bills(&self) -> Result<Vec<Bill>> {
        self.not_supported()
    }
    fn get_bills_due_soon(&self, days: i32) -> Result<Vec<Bill>> {
        self.not_supported()
    }
    fn create_payment(&self, input: CreateBillPayment) -> Result<BillPayment> {
        self.not_supported()
    }
    fn get_payment(&self, id: Uuid) -> Result<Option<BillPayment>> {
        self.not_supported()
    }
    fn get_payment_by_number(&self, number: &str) -> Result<Option<BillPayment>> {
        self.not_supported()
    }
    fn list_payments(&self, filter: BillPaymentFilter) -> Result<Vec<BillPayment>> {
        self.not_supported()
    }
    fn void_payment(&self, id: Uuid) -> Result<BillPayment> {
        self.not_supported()
    }
    fn clear_payment(&self, id: Uuid) -> Result<BillPayment> {
        self.not_supported()
    }
    fn get_payment_allocations(&self, payment_id: Uuid) -> Result<Vec<PaymentAllocation>> {
        self.not_supported()
    }
    fn get_payments_for_bill(&self, bill_id: Uuid) -> Result<Vec<BillPayment>> {
        self.not_supported()
    }
    fn count_payments(&self, filter: BillPaymentFilter) -> Result<u64> {
        self.not_supported()
    }
    fn create_payment_run(&self, input: CreatePaymentRun) -> Result<PaymentRun> {
        self.not_supported()
    }
    fn get_payment_run(&self, id: Uuid) -> Result<Option<PaymentRun>> {
        self.not_supported()
    }
    fn list_payment_runs(&self, filter: PaymentRunFilter) -> Result<Vec<PaymentRun>> {
        self.not_supported()
    }
    fn approve_payment_run(&self, id: Uuid, approved_by: &str) -> Result<PaymentRun> {
        self.not_supported()
    }
    fn process_payment_run(&self, id: Uuid) -> Result<PaymentRun> {
        self.not_supported()
    }
    fn cancel_payment_run(&self, id: Uuid) -> Result<PaymentRun> {
        self.not_supported()
    }
    fn get_payment_run_bills(&self, run_id: Uuid) -> Result<Vec<Bill>> {
        self.not_supported()
    }
    fn get_aging_summary(&self) -> Result<ApAgingSummary> {
        self.not_supported()
    }
    fn get_supplier_summary(&self, supplier_id: Uuid) -> Result<Option<SupplierApSummary>> {
        self.not_supported()
    }
    fn get_total_outstanding(&self) -> Result<rust_decimal::Decimal> {
        self.not_supported()
    }
    fn create_bills_batch(&self, inputs: Vec<CreateBill>) -> Result<BatchResult<Bill>> {
        self.not_supported()
    }
    fn get_bills_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Bill>> {
        self.not_supported()
    }
}

impl CostAccountingRepository for UnsupportedPostgresRepository {
    fn get_item_cost(&self, sku: &str) -> Result<Option<ItemCost>> {
        self.not_supported()
    }
    fn set_item_cost(&self, input: SetItemCost) -> Result<ItemCost> {
        self.not_supported()
    }
    fn list_item_costs(&self, filter: ItemCostFilter) -> Result<Vec<ItemCost>> {
        self.not_supported()
    }
    fn update_average_cost(
        &self,
        sku: &str,
        quantity: rust_decimal::Decimal,
        unit_cost: rust_decimal::Decimal,
    ) -> Result<ItemCost> {
        self.not_supported()
    }
    fn update_last_cost(&self, sku: &str, unit_cost: rust_decimal::Decimal) -> Result<ItemCost> {
        self.not_supported()
    }
    fn create_cost_layer(&self, input: CreateCostLayer) -> Result<CostLayer> {
        self.not_supported()
    }
    fn get_cost_layer(&self, id: Uuid) -> Result<Option<CostLayer>> {
        self.not_supported()
    }
    fn list_cost_layers(&self, filter: CostLayerFilter) -> Result<Vec<CostLayer>> {
        self.not_supported()
    }
    fn issue_fifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>> {
        self.not_supported()
    }
    fn issue_lifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>> {
        self.not_supported()
    }
    fn get_layers_remaining(&self, sku: &str) -> Result<rust_decimal::Decimal> {
        self.not_supported()
    }
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
    ) -> Result<CostTransaction> {
        self.not_supported()
    }
    fn list_cost_transactions(
        &self,
        filter: CostTransactionFilter,
    ) -> Result<Vec<CostTransaction>> {
        self.not_supported()
    }
    fn record_variance(&self, input: RecordCostVariance) -> Result<CostVariance> {
        self.not_supported()
    }
    fn list_variances(&self, filter: CostVarianceFilter) -> Result<Vec<CostVariance>> {
        self.not_supported()
    }
    fn get_variance_summary(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<rust_decimal::Decimal> {
        self.not_supported()
    }
    fn create_adjustment(&self, input: CreateCostAdjustment) -> Result<CostAdjustment> {
        self.not_supported()
    }
    fn get_adjustment(&self, id: Uuid) -> Result<Option<CostAdjustment>> {
        self.not_supported()
    }
    fn list_adjustments(&self, filter: CostAdjustmentFilter) -> Result<Vec<CostAdjustment>> {
        self.not_supported()
    }
    fn approve_adjustment(&self, id: Uuid, approved_by: &str) -> Result<CostAdjustment> {
        self.not_supported()
    }
    fn apply_adjustment(&self, id: Uuid) -> Result<CostAdjustment> {
        self.not_supported()
    }
    fn reject_adjustment(&self, id: Uuid) -> Result<CostAdjustment> {
        self.not_supported()
    }
    fn calculate_rollup(&self, sku: &str, bom_id: Option<Uuid>) -> Result<CostRollup> {
        self.not_supported()
    }
    fn get_rollup(&self, sku: &str) -> Result<Option<CostRollup>> {
        self.not_supported()
    }
    fn get_inventory_valuation(&self, cost_method: CostMethod) -> Result<InventoryValuation> {
        self.not_supported()
    }
    fn get_sku_cost_summary(&self, sku: &str) -> Result<Option<SkuCostSummary>> {
        self.not_supported()
    }
    fn get_total_inventory_value(&self) -> Result<rust_decimal::Decimal> {
        self.not_supported()
    }
}

impl CreditRepository for UnsupportedPostgresRepository {
    fn create_credit_account(&self, input: CreateCreditAccount) -> Result<CreditAccount> {
        self.not_supported()
    }
    fn get_credit_account(&self, id: Uuid) -> Result<Option<CreditAccount>> {
        self.not_supported()
    }
    fn get_credit_account_by_customer(&self, customer_id: Uuid) -> Result<Option<CreditAccount>> {
        self.not_supported()
    }
    fn update_credit_account(&self, id: Uuid, input: UpdateCreditAccount) -> Result<CreditAccount> {
        self.not_supported()
    }
    fn list_credit_accounts(&self, filter: CreditAccountFilter) -> Result<Vec<CreditAccount>> {
        self.not_supported()
    }
    fn adjust_credit_limit(
        &self,
        customer_id: Uuid,
        new_limit: rust_decimal::Decimal,
        reason: &str,
    ) -> Result<CreditAccount> {
        self.not_supported()
    }
    fn suspend_credit_account(&self, customer_id: Uuid, reason: &str) -> Result<CreditAccount> {
        self.not_supported()
    }
    fn reactivate_credit_account(&self, customer_id: Uuid) -> Result<CreditAccount> {
        self.not_supported()
    }
    fn check_credit(
        &self,
        customer_id: Uuid,
        order_amount: rust_decimal::Decimal,
    ) -> Result<CreditCheckResult> {
        self.not_supported()
    }
    fn reserve_credit(
        &self,
        customer_id: Uuid,
        order_id: Uuid,
        amount: rust_decimal::Decimal,
    ) -> Result<CreditAccount> {
        self.not_supported()
    }
    fn release_credit_reservation(
        &self,
        customer_id: Uuid,
        order_id: Uuid,
    ) -> Result<CreditAccount> {
        self.not_supported()
    }
    fn charge_credit(
        &self,
        customer_id: Uuid,
        order_id: Uuid,
        amount: rust_decimal::Decimal,
    ) -> Result<CreditAccount> {
        self.not_supported()
    }
    fn place_hold(&self, input: PlaceCreditHold) -> Result<CreditHold> {
        self.not_supported()
    }
    fn get_hold(&self, id: Uuid) -> Result<Option<CreditHold>> {
        self.not_supported()
    }
    fn list_holds(&self, filter: CreditHoldFilter) -> Result<Vec<CreditHold>> {
        self.not_supported()
    }
    fn release_hold(&self, input: ReleaseCreditHold) -> Result<CreditHold> {
        self.not_supported()
    }
    fn get_active_holds(&self, customer_id: Uuid) -> Result<Vec<CreditHold>> {
        self.not_supported()
    }
    fn get_holds_for_order(&self, order_id: Uuid) -> Result<Vec<CreditHold>> {
        self.not_supported()
    }
    fn submit_application(&self, input: SubmitCreditApplication) -> Result<CreditApplication> {
        self.not_supported()
    }
    fn get_application(&self, id: Uuid) -> Result<Option<CreditApplication>> {
        self.not_supported()
    }
    fn list_applications(&self, filter: CreditApplicationFilter) -> Result<Vec<CreditApplication>> {
        self.not_supported()
    }
    fn review_application(&self, input: ReviewCreditApplication) -> Result<CreditApplication> {
        self.not_supported()
    }
    fn withdraw_application(&self, id: Uuid) -> Result<CreditApplication> {
        self.not_supported()
    }
    fn record_transaction(&self, input: RecordCreditTransaction) -> Result<CreditTransaction> {
        self.not_supported()
    }
    fn list_transactions(&self, filter: CreditTransactionFilter) -> Result<Vec<CreditTransaction>> {
        self.not_supported()
    }
    fn apply_payment(
        &self,
        customer_id: Uuid,
        amount: rust_decimal::Decimal,
        reference_id: Option<Uuid>,
    ) -> Result<CreditAccount> {
        self.not_supported()
    }
    fn get_customer_summary(&self, customer_id: Uuid) -> Result<Option<CustomerCreditSummary>> {
        self.not_supported()
    }
    fn get_aging_report(&self) -> Result<Vec<(Uuid, CreditAgingBucket)>> {
        self.not_supported()
    }
    fn get_over_limit_customers(&self) -> Result<Vec<CreditAccount>> {
        self.not_supported()
    }
}

impl BackorderRepository for UnsupportedPostgresRepository {
    fn create_backorder(&self, input: CreateBackorder) -> Result<Backorder> {
        self.not_supported()
    }
    fn get_backorder(&self, id: Uuid) -> Result<Option<Backorder>> {
        self.not_supported()
    }
    fn get_backorder_by_number(&self, number: &str) -> Result<Option<Backorder>> {
        self.not_supported()
    }
    fn update_backorder(&self, id: Uuid, input: UpdateBackorder) -> Result<Backorder> {
        self.not_supported()
    }
    fn list_backorders(&self, filter: BackorderFilter) -> Result<Vec<Backorder>> {
        self.not_supported()
    }
    fn cancel_backorder(&self, id: Uuid) -> Result<Backorder> {
        self.not_supported()
    }
    fn get_backorders_for_order(&self, order_id: Uuid) -> Result<Vec<Backorder>> {
        self.not_supported()
    }
    fn get_backorders_for_customer(&self, customer_id: Uuid) -> Result<Vec<Backorder>> {
        self.not_supported()
    }
    fn get_backorders_for_sku(&self, sku: &str) -> Result<Vec<Backorder>> {
        self.not_supported()
    }
    fn fulfill_backorder(&self, input: FulfillBackorder) -> Result<Backorder> {
        self.not_supported()
    }
    fn get_fulfillment_history(&self, backorder_id: Uuid) -> Result<Vec<BackorderFulfillment>> {
        self.not_supported()
    }
    fn allocate_backorder(&self, input: AllocateBackorder) -> Result<BackorderAllocation> {
        self.not_supported()
    }
    fn get_allocations(&self, backorder_id: Uuid) -> Result<Vec<BackorderAllocation>> {
        self.not_supported()
    }
    fn release_allocation(&self, allocation_id: Uuid) -> Result<BackorderAllocation> {
        self.not_supported()
    }
    fn confirm_allocation(&self, allocation_id: Uuid) -> Result<BackorderAllocation> {
        self.not_supported()
    }
    fn expire_allocations(&self) -> Result<u32> {
        self.not_supported()
    }
    fn auto_allocate_inventory(&self, sku: &str) -> Result<Vec<BackorderAllocation>> {
        self.not_supported()
    }
    fn get_summary(&self) -> Result<BackorderSummary> {
        self.not_supported()
    }
    fn get_sku_summary(&self, sku: &str) -> Result<Option<SkuBackorderSummary>> {
        self.not_supported()
    }
    fn get_overdue_backorders(&self) -> Result<Vec<Backorder>> {
        self.not_supported()
    }
    fn count_pending(&self) -> Result<u64> {
        self.not_supported()
    }
}

impl AccountsReceivableRepository for UnsupportedPostgresRepository {
    fn get_aging_summary(&self) -> Result<ArAgingSummary> {
        self.not_supported()
    }
    fn get_customer_aging(&self, customer_id: Uuid) -> Result<Option<CustomerArAging>> {
        self.not_supported()
    }
    fn get_aging_report(&self, filter: ArAgingFilter) -> Result<Vec<CustomerArAging>> {
        self.not_supported()
    }
    fn log_collection_activity(
        &self,
        input: CreateCollectionActivity,
    ) -> Result<CollectionActivity> {
        self.not_supported()
    }
    fn list_collection_activities(
        &self,
        filter: CollectionActivityFilter,
    ) -> Result<Vec<CollectionActivity>> {
        self.not_supported()
    }
    fn update_collection_status(&self, invoice_id: Uuid, status: CollectionStatus) -> Result<()> {
        self.not_supported()
    }
    fn get_invoices_due_for_dunning(&self) -> Result<Vec<Invoice>> {
        self.not_supported()
    }
    fn send_dunning_letter(
        &self,
        invoice_id: Uuid,
        letter_type: DunningLetterType,
        sent_by: Option<&str>,
    ) -> Result<CollectionActivity> {
        self.not_supported()
    }
    fn create_write_off(&self, input: CreateWriteOff) -> Result<WriteOff> {
        self.not_supported()
    }
    fn get_write_off(&self, id: Uuid) -> Result<Option<WriteOff>> {
        self.not_supported()
    }
    fn list_write_offs(&self, filter: WriteOffFilter) -> Result<Vec<WriteOff>> {
        self.not_supported()
    }
    fn reverse_write_off(&self, id: Uuid) -> Result<WriteOff> {
        self.not_supported()
    }
    fn create_credit_memo(&self, input: CreateCreditMemo) -> Result<CreditMemo> {
        self.not_supported()
    }
    fn get_credit_memo(&self, id: Uuid) -> Result<Option<CreditMemo>> {
        self.not_supported()
    }
    fn get_credit_memo_by_number(&self, number: &str) -> Result<Option<CreditMemo>> {
        self.not_supported()
    }
    fn list_credit_memos(&self, filter: CreditMemoFilter) -> Result<Vec<CreditMemo>> {
        self.not_supported()
    }
    fn apply_credit_memo(&self, input: ApplyCreditMemo) -> Result<CreditMemo> {
        self.not_supported()
    }
    fn void_credit_memo(&self, id: Uuid) -> Result<CreditMemo> {
        self.not_supported()
    }
    fn get_unapplied_credits(&self, customer_id: Uuid) -> Result<Vec<CreditMemo>> {
        self.not_supported()
    }
    fn apply_payment_to_invoices(
        &self,
        input: ApplyPaymentToInvoices,
    ) -> Result<Vec<ArPaymentApplication>> {
        self.not_supported()
    }
    fn get_payment_applications(&self, payment_id: Uuid) -> Result<Vec<ArPaymentApplication>> {
        self.not_supported()
    }
    fn unapply_payment(&self, application_id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn get_customer_summary(&self, customer_id: Uuid) -> Result<Option<CustomerArSummary>> {
        self.not_supported()
    }
    fn generate_statement(&self, request: GenerateStatementRequest) -> Result<CustomerStatement> {
        self.not_supported()
    }
    fn get_total_outstanding(&self) -> Result<rust_decimal::Decimal> {
        self.not_supported()
    }
    fn get_dso(&self, days: i32) -> Result<rust_decimal::Decimal> {
        self.not_supported()
    }
    fn get_average_days_to_pay(&self, customer_id: Uuid) -> Result<Option<i32>> {
        self.not_supported()
    }
    fn get_customers_batch(&self, ids: Vec<Uuid>) -> Result<Vec<CustomerArSummary>> {
        self.not_supported()
    }
}

impl GeneralLedgerRepository for UnsupportedPostgresRepository {
    fn create_account(&self, input: CreateGlAccount) -> Result<GlAccount> {
        self.not_supported()
    }
    fn get_account(&self, id: Uuid) -> Result<Option<GlAccount>> {
        self.not_supported()
    }
    fn get_account_by_number(&self, account_number: &str) -> Result<Option<GlAccount>> {
        self.not_supported()
    }
    fn update_account(&self, id: Uuid, input: UpdateGlAccount) -> Result<GlAccount> {
        self.not_supported()
    }
    fn list_accounts(&self, filter: GlAccountFilter) -> Result<Vec<GlAccount>> {
        self.not_supported()
    }
    fn get_account_hierarchy(&self) -> Result<Vec<GlAccount>> {
        self.not_supported()
    }
    fn delete_account(&self, id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn initialize_chart_of_accounts(&self) -> Result<Vec<GlAccount>> {
        self.not_supported()
    }
    fn create_period(&self, input: CreateGlPeriod) -> Result<GlPeriod> {
        self.not_supported()
    }
    fn get_period(&self, id: Uuid) -> Result<Option<GlPeriod>> {
        self.not_supported()
    }
    fn get_current_period(&self) -> Result<Option<GlPeriod>> {
        self.not_supported()
    }
    fn get_period_for_date(&self, date: NaiveDate) -> Result<Option<GlPeriod>> {
        self.not_supported()
    }
    fn list_periods(&self, filter: GlPeriodFilter) -> Result<Vec<GlPeriod>> {
        self.not_supported()
    }
    fn open_period(&self, id: Uuid) -> Result<GlPeriod> {
        self.not_supported()
    }
    fn close_period(&self, id: Uuid, closed_by: &str) -> Result<GlPeriod> {
        self.not_supported()
    }
    fn lock_period(&self, id: Uuid, locked_by: &str) -> Result<GlPeriod> {
        self.not_supported()
    }
    fn reopen_period(&self, id: Uuid) -> Result<GlPeriod> {
        self.not_supported()
    }
    fn create_journal_entry(&self, input: CreateJournalEntry) -> Result<JournalEntry> {
        self.not_supported()
    }
    fn get_journal_entry(&self, id: Uuid) -> Result<Option<JournalEntry>> {
        self.not_supported()
    }
    fn get_journal_entry_by_number(&self, number: &str) -> Result<Option<JournalEntry>> {
        self.not_supported()
    }
    fn list_journal_entries(&self, filter: JournalEntryFilter) -> Result<Vec<JournalEntry>> {
        self.not_supported()
    }
    fn post_journal_entry(&self, id: Uuid, posted_by: &str) -> Result<JournalEntry> {
        self.not_supported()
    }
    fn void_journal_entry(&self, id: Uuid) -> Result<JournalEntry> {
        self.not_supported()
    }
    fn reverse_journal_entry(&self, id: Uuid, reversal_date: NaiveDate) -> Result<JournalEntry> {
        self.not_supported()
    }
    fn get_journal_entry_lines(&self, journal_entry_id: Uuid) -> Result<Vec<JournalEntryLine>> {
        self.not_supported()
    }
    fn get_auto_posting_config(&self) -> Result<Option<AutoPostingConfig>> {
        self.not_supported()
    }
    fn set_auto_posting_config(&self, input: CreateAutoPostingConfig) -> Result<AutoPostingConfig> {
        self.not_supported()
    }
    fn auto_post_invoice(&self, invoice_id: Uuid) -> Result<JournalEntry> {
        self.not_supported()
    }
    fn auto_post_payment_received(&self, payment_id: Uuid) -> Result<JournalEntry> {
        self.not_supported()
    }
    fn auto_post_bill(&self, bill_id: Uuid) -> Result<JournalEntry> {
        self.not_supported()
    }
    fn auto_post_bill_payment(&self, payment_id: Uuid) -> Result<JournalEntry> {
        self.not_supported()
    }
    fn auto_post_inventory_cost(&self, cost_transaction_id: Uuid) -> Result<JournalEntry> {
        self.not_supported()
    }
    fn auto_post_write_off(&self, write_off_id: Uuid) -> Result<JournalEntry> {
        self.not_supported()
    }
    fn get_trial_balance(&self, as_of_date: NaiveDate) -> Result<TrialBalance> {
        self.not_supported()
    }
    fn get_balance_sheet(&self, as_of_date: NaiveDate) -> Result<BalanceSheet> {
        self.not_supported()
    }
    fn get_income_statement(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<IncomeStatement> {
        self.not_supported()
    }
    fn get_account_balance(
        &self,
        account_id: Uuid,
        as_of_date: Option<NaiveDate>,
    ) -> Result<Option<rust_decimal::Decimal>> {
        self.not_supported()
    }
    fn get_account_transactions(
        &self,
        account_id: Uuid,
        filter: JournalEntryFilter,
    ) -> Result<Vec<JournalEntryLine>> {
        self.not_supported()
    }
    fn run_period_close(&self, period_id: Uuid, closed_by: &str) -> Result<JournalEntry> {
        self.not_supported()
    }
    fn create_accounts_batch(
        &self,
        inputs: Vec<CreateGlAccount>,
    ) -> Result<BatchResult<GlAccount>> {
        self.not_supported()
    }
    fn get_accounts_batch(&self, ids: Vec<Uuid>) -> Result<Vec<GlAccount>> {
        self.not_supported()
    }
}

impl X402PaymentIntentRepository for UnsupportedPostgresRepository {
    fn create(&self, _input: CreateX402PaymentIntent) -> Result<X402PaymentIntent> {
        self.not_supported()
    }
    fn get(&self, _id: Uuid) -> Result<Option<X402PaymentIntent>> {
        self.not_supported()
    }
    fn get_by_idempotency_key(&self, _key: &str) -> Result<Option<X402PaymentIntent>> {
        self.not_supported()
    }
    fn sign(&self, _id: Uuid, _input: SignX402PaymentIntent) -> Result<X402PaymentIntent> {
        self.not_supported()
    }
    fn mark_sequenced(
        &self,
        _id: Uuid,
        _sequence_number: u64,
        _batch_id: Uuid,
    ) -> Result<X402PaymentIntent> {
        self.not_supported()
    }
    fn mark_settled(
        &self,
        _id: Uuid,
        _tx_hash: &str,
        _block_number: u64,
    ) -> Result<X402PaymentIntent> {
        self.not_supported()
    }
    fn mark_failed(&self, _id: Uuid, _reason: &str) -> Result<X402PaymentIntent> {
        self.not_supported()
    }
    fn mark_expired(&self, _id: Uuid) -> Result<X402PaymentIntent> {
        self.not_supported()
    }
    fn cancel(&self, _id: Uuid) -> Result<X402PaymentIntent> {
        self.not_supported()
    }
    fn for_cart(&self, _cart_id: Uuid) -> Result<Vec<X402PaymentIntent>> {
        self.not_supported()
    }
    fn for_order(&self, _order_id: Uuid) -> Result<Vec<X402PaymentIntent>> {
        self.not_supported()
    }
    fn get_next_nonce(&self, _payer_address: &str) -> Result<u64> {
        self.not_supported()
    }
    fn list(&self, _filter: X402PaymentIntentFilter) -> Result<Vec<X402PaymentIntent>> {
        self.not_supported()
    }
    fn count(&self, _filter: X402PaymentIntentFilter) -> Result<u64> {
        self.not_supported()
    }
    fn expire_stale_intents(&self) -> Result<u64> {
        self.not_supported()
    }
    fn create_batch(
        &self,
        _inputs: Vec<CreateX402PaymentIntent>,
    ) -> Result<BatchResult<X402PaymentIntent>> {
        self.not_supported()
    }
    fn create_batch_atomic(
        &self,
        _inputs: Vec<CreateX402PaymentIntent>,
    ) -> Result<Vec<X402PaymentIntent>> {
        self.not_supported()
    }
    fn get_batch(&self, _ids: Vec<Uuid>) -> Result<Vec<X402PaymentIntent>> {
        self.not_supported()
    }
}

impl X402CreditRepository for UnsupportedPostgresRepository {
    fn get_account(
        &self,
        _payer_address: &str,
        _asset: X402Asset,
        _network: X402Network,
    ) -> Result<Option<X402CreditAccount>> {
        self.not_supported()
    }
    fn get_or_create_account(
        &self,
        _payer_address: &str,
        _asset: X402Asset,
        _network: X402Network,
    ) -> Result<X402CreditAccount> {
        self.not_supported()
    }
    fn get_balance(
        &self,
        _payer_address: &str,
        _asset: X402Asset,
        _network: X402Network,
    ) -> Result<u64> {
        self.not_supported()
    }
    fn adjust_balance(&self, _input: X402CreditAdjustment) -> Result<X402CreditTransaction> {
        self.not_supported()
    }
    fn list_transactions(
        &self,
        _filter: X402CreditTransactionFilter,
    ) -> Result<Vec<X402CreditTransaction>> {
        self.not_supported()
    }
}

impl AgentCardRepository for UnsupportedPostgresRepository {
    fn create(&self, _input: CreateAgentCard) -> Result<AgentCard> {
        self.not_supported()
    }
    fn get(&self, _id: Uuid) -> Result<Option<AgentCard>> {
        self.not_supported()
    }
    fn get_by_wallet(&self, _wallet_address: &str) -> Result<Option<AgentCard>> {
        self.not_supported()
    }
    fn update(&self, _id: Uuid, _input: UpdateAgentCard) -> Result<AgentCard> {
        self.not_supported()
    }
    fn delete(&self, _id: Uuid) -> Result<()> {
        self.not_supported()
    }
    fn list(&self, _filter: AgentCardFilter) -> Result<Vec<AgentCard>> {
        self.not_supported()
    }
    fn count(&self, _filter: AgentCardFilter) -> Result<u64> {
        self.not_supported()
    }
    fn verify(&self, _id: Uuid, _trust_level: TrustLevel, _method: &str) -> Result<AgentCard> {
        self.not_supported()
    }
    fn suspend(&self, _id: Uuid, _reason: &str) -> Result<AgentCard> {
        self.not_supported()
    }
    fn reactivate(&self, _id: Uuid) -> Result<AgentCard> {
        self.not_supported()
    }
    fn discover(&self, _filter: AgentCardFilter) -> Result<Vec<AgentCard>> {
        self.not_supported()
    }
    fn create_batch(&self, _inputs: Vec<CreateAgentCard>) -> Result<BatchResult<AgentCard>> {
        self.not_supported()
    }
    fn create_batch_atomic(&self, _inputs: Vec<CreateAgentCard>) -> Result<Vec<AgentCard>> {
        self.not_supported()
    }
    fn get_batch(&self, _ids: Vec<Uuid>) -> Result<Vec<AgentCard>> {
        self.not_supported()
    }
}

impl AgentIdentityRepository for UnsupportedPostgresRepository {
    fn register(&self, _input: CreateAgentIdentity) -> Result<AgentIdentity> {
        self.not_supported()
    }
    fn get(&self, _agent_registry: &str, _agent_id: &str) -> Result<Option<AgentIdentity>> {
        self.not_supported()
    }
    fn get_by_wallet(&self, _agent_wallet: &str) -> Result<Option<AgentIdentity>> {
        self.not_supported()
    }
    fn update(
        &self,
        _agent_registry: &str,
        _agent_id: &str,
        _input: UpdateAgentIdentity,
    ) -> Result<AgentIdentity> {
        self.not_supported()
    }
    fn set_agent_wallet(
        &self,
        _agent_registry: &str,
        _agent_id: &str,
        _agent_wallet: &str,
        _proof_type: Option<AgentWalletProofType>,
        _proof: Option<&str>,
        _proof_chain_id: Option<u64>,
        _proof_deadline: Option<DateTime<Utc>>,
    ) -> Result<AgentIdentity> {
        self.not_supported()
    }
    fn clear_agent_wallet(&self, _agent_registry: &str, _agent_id: &str) -> Result<AgentIdentity> {
        self.not_supported()
    }
    fn list(&self, _filter: AgentIdentityFilter) -> Result<Vec<AgentIdentity>> {
        self.not_supported()
    }
    fn count(&self, _filter: AgentIdentityFilter) -> Result<u64> {
        self.not_supported()
    }
    fn set_metadata(
        &self,
        _agent_registry: &str,
        _agent_id: &str,
        _entry: AgentMetadataEntry,
    ) -> Result<()> {
        self.not_supported()
    }
    fn get_metadata(
        &self,
        _agent_registry: &str,
        _agent_id: &str,
        _metadata_key: &str,
    ) -> Result<Option<Vec<u8>>> {
        self.not_supported()
    }
    fn delete_metadata(
        &self,
        _agent_registry: &str,
        _agent_id: &str,
        _metadata_key: &str,
    ) -> Result<()> {
        self.not_supported()
    }
}

impl AgentReputationRepository for UnsupportedPostgresRepository {
    fn give_feedback(&self, _input: CreateAgentFeedback) -> Result<AgentFeedback> {
        self.not_supported()
    }
    fn revoke_feedback(
        &self,
        _agent_registry: &str,
        _agent_id: &str,
        _client_address: &str,
        _feedback_index: u64,
    ) -> Result<AgentFeedback> {
        self.not_supported()
    }
    fn read_feedback(
        &self,
        _agent_registry: &str,
        _agent_id: &str,
        _client_address: &str,
        _feedback_index: u64,
    ) -> Result<Option<AgentFeedback>> {
        self.not_supported()
    }
    fn read_all_feedback(&self, _filter: AgentFeedbackFilter) -> Result<Vec<AgentFeedback>> {
        self.not_supported()
    }
    fn get_summary(
        &self,
        _agent_registry: &str,
        _agent_id: &str,
        _client_addresses: Vec<String>,
        _tag1: Option<String>,
        _tag2: Option<String>,
    ) -> Result<FeedbackSummary> {
        self.not_supported()
    }
    fn append_response(
        &self,
        _input: CreateAgentFeedbackResponse,
    ) -> Result<AgentFeedbackResponse> {
        self.not_supported()
    }
    fn get_response_count(
        &self,
        _agent_registry: &str,
        _agent_id: &str,
        _client_address: &str,
        _feedback_index: u64,
        _responders: Option<Vec<String>>,
    ) -> Result<u64> {
        self.not_supported()
    }
    fn get_clients(&self, _agent_registry: &str, _agent_id: &str) -> Result<Vec<String>> {
        self.not_supported()
    }
    fn get_last_index(
        &self,
        _agent_registry: &str,
        _agent_id: &str,
        _client_address: &str,
    ) -> Result<u64> {
        self.not_supported()
    }
}

impl AgentValidationRepository for UnsupportedPostgresRepository {
    fn request_validation(
        &self,
        _input: CreateAgentValidationRequest,
    ) -> Result<AgentValidationRequest> {
        self.not_supported()
    }
    fn respond_validation(
        &self,
        _request_hash: &str,
        _input: CreateAgentValidationResponse,
    ) -> Result<AgentValidationResponse> {
        self.not_supported()
    }
    fn get_validation_status(&self, _request_hash: &str) -> Result<Option<AgentValidationStatus>> {
        self.not_supported()
    }
    fn get_summary(
        &self,
        _agent_registry: &str,
        _agent_id: &str,
        _validator_addresses: Option<Vec<String>>,
        _tag: Option<String>,
    ) -> Result<ValidationSummary> {
        self.not_supported()
    }
    fn get_agent_validations(&self, _agent_registry: &str, _agent_id: &str) -> Result<Vec<String>> {
        self.not_supported()
    }
    fn get_validator_requests(&self, _validator_address: &str) -> Result<Vec<String>> {
        self.not_supported()
    }
}
