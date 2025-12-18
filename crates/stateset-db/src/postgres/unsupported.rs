//! Placeholder repository implementations for PostgreSQL.
//!
//! The PostgreSQL backend is still under development. These implementations prevent runtime panics
//! by returning a structured `CommerceError` when a domain repository is not yet supported.

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
    fn update_item(&self, _item_id: Uuid, _item: CreatePurchaseOrderItem) -> Result<PurchaseOrderItem> {
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
}

impl AnalyticsRepository for UnsupportedPostgresRepository {
    fn get_sales_summary(&self, _query: AnalyticsQuery) -> Result<SalesSummary> {
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
}

