//! WASM bindings for StateSet Embedded Commerce
//!
//! Provides an in-memory commerce library for browser environments.
//!
//! ```javascript
//! import init, { Commerce } from '@stateset/embedded-wasm';
//!
//! async function main() {
//!   await init();
//!   const commerce = new Commerce();
//!   const customer = commerce.createCustomer({
//!     email: "alice@example.com",
//!     firstName: "Alice",
//!     lastName: "Smith"
//!   });
//! }
//! ```

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use uuid::Uuid;
use wasm_bindgen::prelude::*;

// Initialize panic hook for better error messages
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

// ============================================================================
// In-Memory Store
// ============================================================================

#[derive(Default)]
struct Store {
    customers: HashMap<Uuid, CustomerData>,
    orders: HashMap<Uuid, OrderData>,
    order_items: HashMap<Uuid, Vec<OrderItemData>>,
    products: HashMap<Uuid, ProductData>,
    variants: HashMap<Uuid, VariantData>,
    inventory_items: HashMap<i64, InventoryItemData>,
    inventory_by_sku: HashMap<String, i64>,
    inventory_balances: HashMap<i64, InventoryBalanceData>,
    reservations: HashMap<Uuid, ReservationData>,
    returns: HashMap<Uuid, ReturnData>,
    return_items: HashMap<Uuid, Vec<ReturnItemData>>,
    // New modules
    payments: HashMap<Uuid, PaymentData>,
    refunds: HashMap<Uuid, RefundData>,
    shipments: HashMap<Uuid, ShipmentData>,
    warranties: HashMap<Uuid, WarrantyData>,
    warranty_claims: HashMap<Uuid, WarrantyClaimData>,
    suppliers: HashMap<Uuid, SupplierData>,
    purchase_orders: HashMap<Uuid, PurchaseOrderData>,
    invoices: HashMap<Uuid, InvoiceData>,
    boms: HashMap<Uuid, BomData>,
    bom_components: HashMap<Uuid, Vec<BomComponentData>>,
    work_orders: HashMap<Uuid, WorkOrderData>,
    // Carts
    carts: HashMap<Uuid, CartData>,
    cart_items: HashMap<Uuid, Vec<CartItemData>>,
    // Counters
    next_inventory_id: i64,
    next_order_number: u64,
    next_payment_number: u64,
    next_shipment_number: u64,
    next_warranty_number: u64,
    next_claim_number: u64,
    next_supplier_code: u64,
    next_po_number: u64,
    next_invoice_number: u64,
    next_bom_number: u64,
    next_work_order_number: u64,
    next_cart_number: u64,
}

type StoreRef = Rc<RefCell<Store>>;

// ============================================================================
// Internal Data Types
// ============================================================================

#[derive(Clone)]
struct CustomerData {
    id: Uuid,
    email: String,
    first_name: String,
    last_name: String,
    phone: Option<String>,
    status: String,
    accepts_marketing: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct OrderData {
    id: Uuid,
    order_number: String,
    customer_id: Uuid,
    status: String,
    total_amount: f64,
    currency: String,
    payment_status: String,
    fulfillment_status: String,
    tracking_number: Option<String>,
    notes: Option<String>,
    version: i32,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Serialize)]
struct OrderItemData {
    id: Uuid,
    order_id: Uuid,
    sku: String,
    name: String,
    quantity: i32,
    unit_price: f64,
    total: f64,
}

#[derive(Clone)]
struct ProductData {
    id: Uuid,
    name: String,
    slug: String,
    description: String,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct VariantData {
    id: Uuid,
    product_id: Uuid,
    sku: String,
    name: String,
    price: f64,
    compare_at_price: Option<f64>,
    is_default: bool,
}

#[derive(Clone)]
struct InventoryItemData {
    id: i64,
    sku: String,
    name: String,
    description: Option<String>,
    unit_of_measure: String,
    is_active: bool,
}

#[derive(Clone)]
struct InventoryBalanceData {
    item_id: i64,
    on_hand: f64,
    allocated: f64,
}

#[derive(Clone)]
struct ReservationData {
    id: Uuid,
    item_id: i64,
    quantity: f64,
    status: String,
    reference_type: String,
    reference_id: String,
}

#[derive(Clone)]
struct ReturnData {
    id: Uuid,
    order_id: Uuid,
    status: String,
    reason: String,
    reason_details: Option<String>,
    version: i32,
    created_at: String,
}

#[derive(Clone)]
struct ReturnItemData {
    id: Uuid,
    return_id: Uuid,
    order_item_id: Uuid,
    quantity: i32,
}

#[derive(Clone)]
struct PaymentData {
    id: Uuid,
    payment_number: String,
    order_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    amount: f64,
    currency: String,
    status: String,
    payment_method: Option<String>,
    version: i32,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct RefundData {
    id: Uuid,
    payment_id: Uuid,
    amount: f64,
    reason: Option<String>,
    status: String,
    created_at: String,
}

#[derive(Clone)]
struct ShipmentData {
    id: Uuid,
    shipment_number: String,
    order_id: Uuid,
    carrier: Option<String>,
    tracking_number: Option<String>,
    status: String,
    shipped_at: Option<String>,
    delivered_at: Option<String>,
    version: i32,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct WarrantyData {
    id: Uuid,
    warranty_number: String,
    customer_id: Uuid,
    product_id: Option<Uuid>,
    order_id: Option<Uuid>,
    status: String,
    duration_months: i32,
    start_date: String,
    end_date: String,
    created_at: String,
}

#[derive(Clone)]
struct WarrantyClaimData {
    id: Uuid,
    claim_number: String,
    warranty_id: Uuid,
    issue_description: String,
    status: String,
    resolution: Option<String>,
    created_at: String,
}

#[derive(Clone)]
struct SupplierData {
    id: Uuid,
    supplier_code: String,
    name: String,
    email: Option<String>,
    phone: Option<String>,
    status: String,
    created_at: String,
}

#[derive(Clone)]
struct PurchaseOrderData {
    id: Uuid,
    po_number: String,
    supplier_id: Uuid,
    status: String,
    total_amount: f64,
    currency: String,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct InvoiceData {
    id: Uuid,
    invoice_number: String,
    customer_id: Uuid,
    order_id: Option<Uuid>,
    status: String,
    subtotal: f64,
    tax_amount: f64,
    total: f64,
    amount_paid: f64,
    due_date: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct BomData {
    id: Uuid,
    bom_number: String,
    sku: String,
    name: String,
    description: Option<String>,
    status: String,
    version: i32,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct BomComponentData {
    id: Uuid,
    bom_id: Uuid,
    component_sku: String,
    component_name: String,
    quantity: f64,
    unit_of_measure: String,
}

#[derive(Clone)]
struct WorkOrderData {
    id: Uuid,
    work_order_number: String,
    bom_id: Uuid,
    status: String,
    quantity_to_build: f64,
    quantity_built: f64,
    priority: String,
    scheduled_start: Option<String>,
    scheduled_end: Option<String>,
    version: i32,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct CartData {
    id: Uuid,
    cart_number: String,
    customer_id: Option<Uuid>,
    status: String,
    currency: String,
    subtotal: f64,
    tax_amount: f64,
    shipping_amount: f64,
    discount_amount: f64,
    grand_total: f64,
    customer_email: Option<String>,
    customer_name: Option<String>,
    payment_method: Option<String>,
    payment_status: String,
    fulfillment_type: String,
    shipping_method: Option<String>,
    coupon_code: Option<String>,
    notes: Option<String>,
    created_at: String,
    updated_at: String,
    expires_at: Option<String>,
}

#[derive(Clone)]
struct CartItemData {
    id: Uuid,
    cart_id: Uuid,
    sku: String,
    name: String,
    description: Option<String>,
    quantity: i32,
    unit_price: f64,
    total: f64,
}

#[derive(Clone, Serialize)]
struct CartAddressData {
    first_name: String,
    last_name: String,
    line1: String,
    city: String,
    postal_code: String,
    country: String,
    company: Option<String>,
    line2: Option<String>,
    state: Option<String>,
    phone: Option<String>,
    email: Option<String>,
}

// ============================================================================
// JS Return Types (plain objects via serde)
// ============================================================================

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsCustomer {
    id: String,
    email: String,
    first_name: String,
    last_name: String,
    full_name: String,
    phone: Option<String>,
    status: String,
    accepts_marketing: bool,
    created_at: String,
    updated_at: String,
}

impl From<&CustomerData> for JsCustomer {
    fn from(data: &CustomerData) -> Self {
        JsCustomer {
            id: data.id.to_string(),
            email: data.email.clone(),
            first_name: data.first_name.clone(),
            last_name: data.last_name.clone(),
            full_name: format!("{} {}", data.first_name, data.last_name),
            phone: data.phone.clone(),
            status: data.status.clone(),
            accepts_marketing: data.accepts_marketing,
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsOrderItem {
    id: String,
    sku: String,
    name: String,
    quantity: i32,
    unit_price: f64,
    total: f64,
}

impl From<&OrderItemData> for JsOrderItem {
    fn from(data: &OrderItemData) -> Self {
        JsOrderItem {
            id: data.id.to_string(),
            sku: data.sku.clone(),
            name: data.name.clone(),
            quantity: data.quantity,
            unit_price: data.unit_price,
            total: data.total,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsOrder {
    id: String,
    order_number: String,
    customer_id: String,
    status: String,
    total_amount: f64,
    currency: String,
    payment_status: String,
    fulfillment_status: String,
    tracking_number: Option<String>,
    items: Vec<JsOrderItem>,
    version: i32,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsProduct {
    id: String,
    name: String,
    slug: String,
    description: String,
    status: String,
    created_at: String,
    updated_at: String,
}

impl From<&ProductData> for JsProduct {
    fn from(data: &ProductData) -> Self {
        JsProduct {
            id: data.id.to_string(),
            name: data.name.clone(),
            slug: data.slug.clone(),
            description: data.description.clone(),
            status: data.status.clone(),
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsProductVariant {
    id: String,
    product_id: String,
    sku: String,
    name: String,
    price: f64,
    compare_at_price: Option<f64>,
    is_default: bool,
}

impl From<&VariantData> for JsProductVariant {
    fn from(data: &VariantData) -> Self {
        JsProductVariant {
            id: data.id.to_string(),
            product_id: data.product_id.to_string(),
            sku: data.sku.clone(),
            name: data.name.clone(),
            price: data.price,
            compare_at_price: data.compare_at_price,
            is_default: data.is_default,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsInventoryItem {
    id: i64,
    sku: String,
    name: String,
    description: Option<String>,
    unit_of_measure: String,
    is_active: bool,
}

impl From<&InventoryItemData> for JsInventoryItem {
    fn from(data: &InventoryItemData) -> Self {
        JsInventoryItem {
            id: data.id,
            sku: data.sku.clone(),
            name: data.name.clone(),
            description: data.description.clone(),
            unit_of_measure: data.unit_of_measure.clone(),
            is_active: data.is_active,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsStockLevel {
    sku: String,
    name: String,
    total_on_hand: f64,
    total_allocated: f64,
    total_available: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsReservation {
    id: String,
    item_id: i64,
    quantity: f64,
    status: String,
}

impl From<&ReservationData> for JsReservation {
    fn from(data: &ReservationData) -> Self {
        JsReservation {
            id: data.id.to_string(),
            item_id: data.item_id,
            quantity: data.quantity,
            status: data.status.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsReturn {
    id: String,
    order_id: String,
    status: String,
    reason: String,
    reason_details: Option<String>,
    version: i32,
    created_at: String,
}

impl From<&ReturnData> for JsReturn {
    fn from(data: &ReturnData) -> Self {
        JsReturn {
            id: data.id.to_string(),
            order_id: data.order_id.to_string(),
            status: data.status.clone(),
            reason: data.reason.clone(),
            reason_details: data.reason_details.clone(),
            version: data.version,
            created_at: data.created_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsPayment {
    id: String,
    payment_number: String,
    order_id: Option<String>,
    customer_id: Option<String>,
    amount: f64,
    currency: String,
    status: String,
    payment_method: Option<String>,
    version: i32,
    created_at: String,
    updated_at: String,
}

impl From<&PaymentData> for JsPayment {
    fn from(data: &PaymentData) -> Self {
        JsPayment {
            id: data.id.to_string(),
            payment_number: data.payment_number.clone(),
            order_id: data.order_id.map(|id| id.to_string()),
            customer_id: data.customer_id.map(|id| id.to_string()),
            amount: data.amount,
            currency: data.currency.clone(),
            status: data.status.clone(),
            payment_method: data.payment_method.clone(),
            version: data.version,
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsRefund {
    id: String,
    payment_id: String,
    amount: f64,
    reason: Option<String>,
    status: String,
    created_at: String,
}

impl From<&RefundData> for JsRefund {
    fn from(data: &RefundData) -> Self {
        JsRefund {
            id: data.id.to_string(),
            payment_id: data.payment_id.to_string(),
            amount: data.amount,
            reason: data.reason.clone(),
            status: data.status.clone(),
            created_at: data.created_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsShipment {
    id: String,
    shipment_number: String,
    order_id: String,
    carrier: Option<String>,
    tracking_number: Option<String>,
    status: String,
    shipped_at: Option<String>,
    delivered_at: Option<String>,
    version: i32,
    created_at: String,
    updated_at: String,
}

impl From<&ShipmentData> for JsShipment {
    fn from(data: &ShipmentData) -> Self {
        JsShipment {
            id: data.id.to_string(),
            shipment_number: data.shipment_number.clone(),
            order_id: data.order_id.to_string(),
            carrier: data.carrier.clone(),
            tracking_number: data.tracking_number.clone(),
            status: data.status.clone(),
            shipped_at: data.shipped_at.clone(),
            delivered_at: data.delivered_at.clone(),
            version: data.version,
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsWarranty {
    id: String,
    warranty_number: String,
    customer_id: String,
    product_id: Option<String>,
    order_id: Option<String>,
    status: String,
    duration_months: i32,
    start_date: String,
    end_date: String,
    created_at: String,
}

impl From<&WarrantyData> for JsWarranty {
    fn from(data: &WarrantyData) -> Self {
        JsWarranty {
            id: data.id.to_string(),
            warranty_number: data.warranty_number.clone(),
            customer_id: data.customer_id.to_string(),
            product_id: data.product_id.map(|id| id.to_string()),
            order_id: data.order_id.map(|id| id.to_string()),
            status: data.status.clone(),
            duration_months: data.duration_months,
            start_date: data.start_date.clone(),
            end_date: data.end_date.clone(),
            created_at: data.created_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsWarrantyClaim {
    id: String,
    claim_number: String,
    warranty_id: String,
    issue_description: String,
    status: String,
    resolution: Option<String>,
    created_at: String,
}

impl From<&WarrantyClaimData> for JsWarrantyClaim {
    fn from(data: &WarrantyClaimData) -> Self {
        JsWarrantyClaim {
            id: data.id.to_string(),
            claim_number: data.claim_number.clone(),
            warranty_id: data.warranty_id.to_string(),
            issue_description: data.issue_description.clone(),
            status: data.status.clone(),
            resolution: data.resolution.clone(),
            created_at: data.created_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsSupplier {
    id: String,
    supplier_code: String,
    name: String,
    email: Option<String>,
    phone: Option<String>,
    status: String,
    created_at: String,
}

impl From<&SupplierData> for JsSupplier {
    fn from(data: &SupplierData) -> Self {
        JsSupplier {
            id: data.id.to_string(),
            supplier_code: data.supplier_code.clone(),
            name: data.name.clone(),
            email: data.email.clone(),
            phone: data.phone.clone(),
            status: data.status.clone(),
            created_at: data.created_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsPurchaseOrder {
    id: String,
    po_number: String,
    supplier_id: String,
    status: String,
    total_amount: f64,
    currency: String,
    created_at: String,
    updated_at: String,
}

impl From<&PurchaseOrderData> for JsPurchaseOrder {
    fn from(data: &PurchaseOrderData) -> Self {
        JsPurchaseOrder {
            id: data.id.to_string(),
            po_number: data.po_number.clone(),
            supplier_id: data.supplier_id.to_string(),
            status: data.status.clone(),
            total_amount: data.total_amount,
            currency: data.currency.clone(),
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsInvoice {
    id: String,
    invoice_number: String,
    customer_id: String,
    order_id: Option<String>,
    status: String,
    subtotal: f64,
    tax_amount: f64,
    total: f64,
    amount_paid: f64,
    balance_due: f64,
    due_date: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<&InvoiceData> for JsInvoice {
    fn from(data: &InvoiceData) -> Self {
        JsInvoice {
            id: data.id.to_string(),
            invoice_number: data.invoice_number.clone(),
            customer_id: data.customer_id.to_string(),
            order_id: data.order_id.map(|id| id.to_string()),
            status: data.status.clone(),
            subtotal: data.subtotal,
            tax_amount: data.tax_amount,
            total: data.total,
            amount_paid: data.amount_paid,
            balance_due: data.total - data.amount_paid,
            due_date: data.due_date.clone(),
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsBom {
    id: String,
    bom_number: String,
    sku: String,
    name: String,
    description: Option<String>,
    status: String,
    version: i32,
    created_at: String,
    updated_at: String,
}

impl From<&BomData> for JsBom {
    fn from(data: &BomData) -> Self {
        JsBom {
            id: data.id.to_string(),
            bom_number: data.bom_number.clone(),
            sku: data.sku.clone(),
            name: data.name.clone(),
            description: data.description.clone(),
            status: data.status.clone(),
            version: data.version,
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsBomComponent {
    id: String,
    bom_id: String,
    component_sku: String,
    component_name: String,
    quantity: f64,
    unit_of_measure: String,
}

impl From<&BomComponentData> for JsBomComponent {
    fn from(data: &BomComponentData) -> Self {
        JsBomComponent {
            id: data.id.to_string(),
            bom_id: data.bom_id.to_string(),
            component_sku: data.component_sku.clone(),
            component_name: data.component_name.clone(),
            quantity: data.quantity,
            unit_of_measure: data.unit_of_measure.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsWorkOrder {
    id: String,
    work_order_number: String,
    bom_id: String,
    status: String,
    quantity_to_build: f64,
    quantity_built: f64,
    priority: String,
    scheduled_start: Option<String>,
    scheduled_end: Option<String>,
    version: i32,
    created_at: String,
    updated_at: String,
}

impl From<&WorkOrderData> for JsWorkOrder {
    fn from(data: &WorkOrderData) -> Self {
        JsWorkOrder {
            id: data.id.to_string(),
            work_order_number: data.work_order_number.clone(),
            bom_id: data.bom_id.to_string(),
            status: data.status.clone(),
            quantity_to_build: data.quantity_to_build,
            quantity_built: data.quantity_built,
            priority: data.priority.clone(),
            scheduled_start: data.scheduled_start.clone(),
            scheduled_end: data.scheduled_end.clone(),
            version: data.version,
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsCartItem {
    id: String,
    cart_id: String,
    sku: String,
    name: String,
    description: Option<String>,
    quantity: i32,
    unit_price: f64,
    total: f64,
}

impl From<&CartItemData> for JsCartItem {
    fn from(data: &CartItemData) -> Self {
        JsCartItem {
            id: data.id.to_string(),
            cart_id: data.cart_id.to_string(),
            sku: data.sku.clone(),
            name: data.name.clone(),
            description: data.description.clone(),
            quantity: data.quantity,
            unit_price: data.unit_price,
            total: data.total,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsCart {
    id: String,
    cart_number: String,
    customer_id: Option<String>,
    status: String,
    currency: String,
    subtotal: f64,
    tax_amount: f64,
    shipping_amount: f64,
    discount_amount: f64,
    grand_total: f64,
    customer_email: Option<String>,
    customer_name: Option<String>,
    payment_method: Option<String>,
    payment_status: String,
    fulfillment_type: String,
    shipping_method: Option<String>,
    coupon_code: Option<String>,
    notes: Option<String>,
    item_count: usize,
    items: Vec<JsCartItem>,
    created_at: String,
    updated_at: String,
    expires_at: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsCheckoutResult {
    order_id: String,
    order_number: String,
    cart_id: String,
    total_charged: f64,
    currency: String,
}

// ============================================================================
// Input Types
// ============================================================================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateCustomerInput {
    email: String,
    first_name: String,
    last_name: String,
    phone: Option<String>,
    accepts_marketing: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateOrderItemInput {
    sku: String,
    name: String,
    quantity: i32,
    unit_price: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateOrderInput {
    customer_id: String,
    items: Vec<CreateOrderItemInput>,
    currency: Option<String>,
    notes: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateVariantInput {
    sku: String,
    name: Option<String>,
    price: f64,
    compare_at_price: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateProductInput {
    name: String,
    description: Option<String>,
    variants: Option<Vec<CreateVariantInput>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateInventoryItemInput {
    sku: String,
    name: String,
    description: Option<String>,
    initial_quantity: Option<f64>,
    reorder_point: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateReturnItemInput {
    order_item_id: String,
    quantity: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateReturnInput {
    order_id: String,
    reason: String,
    items: Vec<CreateReturnItemInput>,
    reason_details: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePaymentInput {
    amount: f64,
    currency: Option<String>,
    order_id: Option<String>,
    customer_id: Option<String>,
    payment_method: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateRefundInput {
    payment_id: String,
    amount: f64,
    reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateShipmentInput {
    order_id: String,
    carrier: Option<String>,
    tracking_number: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateWarrantyInput {
    customer_id: String,
    product_id: Option<String>,
    order_id: Option<String>,
    duration_months: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateWarrantyClaimInput {
    warranty_id: String,
    issue_description: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateSupplierInput {
    name: String,
    email: Option<String>,
    phone: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePurchaseOrderInput {
    supplier_id: String,
    currency: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateInvoiceInput {
    customer_id: String,
    order_id: Option<String>,
    subtotal: f64,
    tax_amount: Option<f64>,
    due_date: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBomInput {
    sku: String,
    name: String,
    description: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddBomComponentInput {
    component_sku: String,
    component_name: String,
    quantity: f64,
    unit_of_measure: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateWorkOrderInput {
    bom_id: String,
    quantity_to_build: f64,
    priority: Option<String>,
    scheduled_start: Option<String>,
    scheduled_end: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateCartInput {
    customer_id: Option<String>,
    customer_email: Option<String>,
    customer_name: Option<String>,
    currency: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddCartItemInput {
    sku: String,
    name: String,
    quantity: i32,
    unit_price: f64,
    description: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetCartPaymentInput {
    payment_method: String,
    payment_token: Option<String>,
}

// ============================================================================
// Commerce - Main Entry Point
// ============================================================================

/// Main Commerce instance for browser-based commerce operations.
/// Uses in-memory storage (data is lost on page refresh).
#[wasm_bindgen]
pub struct Commerce {
    store: StoreRef,
}

#[wasm_bindgen]
impl Commerce {
    /// Create a new Commerce instance with in-memory storage.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Commerce {
        Commerce {
            store: Rc::new(RefCell::new(Store::default())),
        }
    }

    /// Get the customers API.
    #[wasm_bindgen(getter)]
    pub fn customers(&self) -> Customers {
        Customers {
            store: Rc::clone(&self.store),
        }
    }

    /// Get the orders API.
    #[wasm_bindgen(getter)]
    pub fn orders(&self) -> Orders {
        Orders {
            store: Rc::clone(&self.store),
        }
    }

    /// Get the products API.
    #[wasm_bindgen(getter)]
    pub fn products(&self) -> Products {
        Products {
            store: Rc::clone(&self.store),
        }
    }

    /// Get the inventory API.
    #[wasm_bindgen(getter)]
    pub fn inventory(&self) -> Inventory {
        Inventory {
            store: Rc::clone(&self.store),
        }
    }

    /// Get the returns API.
    #[wasm_bindgen(getter)]
    pub fn returns(&self) -> Returns {
        Returns {
            store: Rc::clone(&self.store),
        }
    }

    /// Get the payments API.
    #[wasm_bindgen(getter)]
    pub fn payments(&self) -> Payments {
        Payments {
            store: Rc::clone(&self.store),
        }
    }

    /// Get the shipments API.
    #[wasm_bindgen(getter)]
    pub fn shipments(&self) -> Shipments {
        Shipments {
            store: Rc::clone(&self.store),
        }
    }

    /// Get the warranties API.
    #[wasm_bindgen(getter)]
    pub fn warranties(&self) -> Warranties {
        Warranties {
            store: Rc::clone(&self.store),
        }
    }

    /// Get the purchase orders API.
    #[wasm_bindgen(getter, js_name = purchaseOrders)]
    pub fn purchase_orders(&self) -> PurchaseOrders {
        PurchaseOrders {
            store: Rc::clone(&self.store),
        }
    }

    /// Get the invoices API.
    #[wasm_bindgen(getter)]
    pub fn invoices(&self) -> Invoices {
        Invoices {
            store: Rc::clone(&self.store),
        }
    }

    /// Get the bill of materials API.
    #[wasm_bindgen(getter)]
    pub fn bom(&self) -> Bom {
        Bom {
            store: Rc::clone(&self.store),
        }
    }

    /// Get the work orders API.
    #[wasm_bindgen(getter, js_name = workOrders)]
    pub fn work_orders(&self) -> WorkOrders {
        WorkOrders {
            store: Rc::clone(&self.store),
        }
    }

    /// Get the carts API.
    #[wasm_bindgen(getter)]
    pub fn carts(&self) -> Carts {
        Carts {
            store: Rc::clone(&self.store),
        }
    }
}

impl Default for Commerce {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Customers API
// ============================================================================

/// Customer management operations.
#[wasm_bindgen]
pub struct Customers {
    store: StoreRef,
}

#[wasm_bindgen]
impl Customers {
    /// Create a new customer.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateCustomerInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let data = CustomerData {
            id,
            email: input.email,
            first_name: input.first_name,
            last_name: input.last_name,
            phone: input.phone,
            status: "active".to_string(),
            accepts_marketing: input.accepts_marketing.unwrap_or(false),
            created_at: now.clone(),
            updated_at: now,
        };

        self.store.borrow_mut().customers.insert(id, data.clone());

        let js_customer: JsCustomer = (&data).into();
        serde_wasm_bindgen::to_value(&js_customer).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a customer by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.customers.get(&uuid) {
            Some(data) => {
                let js_customer: JsCustomer = data.into();
                serde_wasm_bindgen::to_value(&js_customer)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Get a customer by email.
    #[wasm_bindgen(js_name = getByEmail)]
    pub fn get_by_email(&self, email: &str) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();

        match store.customers.values().find(|c| c.email == email) {
            Some(data) => {
                let js_customer: JsCustomer = data.into();
                serde_wasm_bindgen::to_value(&js_customer)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// List all customers.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let customers: Vec<JsCustomer> = store.customers.values().map(|data| data.into()).collect();

        serde_wasm_bindgen::to_value(&customers).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count customers.
    pub fn count(&self) -> u32 {
        self.store.borrow().customers.len() as u32
    }
}

// ============================================================================
// Orders API
// ============================================================================

/// Order management operations.
#[wasm_bindgen]
pub struct Orders {
    store: StoreRef,
}

#[wasm_bindgen]
impl Orders {
    /// Create a new order.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateOrderInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let customer_id = Uuid::parse_str(&input.customer_id)
            .map_err(|_| JsValue::from_str("Invalid customer UUID"))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_order_number += 1;
        let order_number = format!("ORD-{}", store.next_order_number);

        // Calculate total and create items
        let mut total = 0.0;
        let mut items = Vec::new();

        for item_input in &input.items {
            let item_total = item_input.unit_price * item_input.quantity as f64;
            total += item_total;

            items.push(OrderItemData {
                id: Uuid::new_v4(),
                order_id: id,
                sku: item_input.sku.clone(),
                name: item_input.name.clone(),
                quantity: item_input.quantity,
                unit_price: item_input.unit_price,
                total: item_total,
            });
        }

        let data = OrderData {
            id,
            order_number: order_number.clone(),
            customer_id,
            status: "pending".to_string(),
            total_amount: total,
            currency: input.currency.unwrap_or_else(|| "USD".to_string()),
            payment_status: "pending".to_string(),
            fulfillment_status: "unfulfilled".to_string(),
            tracking_number: None,
            notes: input.notes,
            version: 1,
            created_at: now.clone(),
            updated_at: now,
        };

        store.orders.insert(id, data.clone());
        store.order_items.insert(id, items.clone());

        let js_items: Vec<JsOrderItem> = items.iter().map(|i| i.into()).collect();

        let js_order = JsOrder {
            id: data.id.to_string(),
            order_number: data.order_number,
            customer_id: data.customer_id.to_string(),
            status: data.status,
            total_amount: data.total_amount,
            currency: data.currency,
            payment_status: data.payment_status,
            fulfillment_status: data.fulfillment_status,
            tracking_number: data.tracking_number,
            items: js_items,
            version: data.version,
            created_at: data.created_at,
            updated_at: data.updated_at,
        };

        serde_wasm_bindgen::to_value(&js_order).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get an order by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.orders.get(&uuid) {
            Some(data) => {
                let items = store.order_items.get(&uuid).cloned().unwrap_or_default();
                let js_items: Vec<JsOrderItem> = items.iter().map(|i| i.into()).collect();

                let js_order = JsOrder {
                    id: data.id.to_string(),
                    order_number: data.order_number.clone(),
                    customer_id: data.customer_id.to_string(),
                    status: data.status.clone(),
                    total_amount: data.total_amount,
                    currency: data.currency.clone(),
                    payment_status: data.payment_status.clone(),
                    fulfillment_status: data.fulfillment_status.clone(),
                    tracking_number: data.tracking_number.clone(),
                    items: js_items,
                    version: data.version,
                    created_at: data.created_at.clone(),
                    updated_at: data.updated_at.clone(),
                };

                serde_wasm_bindgen::to_value(&js_order)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Get order items.
    #[wasm_bindgen(js_name = getItems)]
    pub fn get_items(&self, order_id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(order_id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        let items: Vec<JsOrderItem> = store
            .order_items
            .get(&uuid)
            .map(|items| items.iter().map(|i| i.into()).collect())
            .unwrap_or_default();

        serde_wasm_bindgen::to_value(&items).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all orders.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let orders: Vec<JsOrder> = store
            .orders
            .values()
            .map(|data| {
                let items = store
                    .order_items
                    .get(&data.id)
                    .cloned()
                    .unwrap_or_default();
                let js_items: Vec<JsOrderItem> = items.iter().map(|i| i.into()).collect();

                JsOrder {
                    id: data.id.to_string(),
                    order_number: data.order_number.clone(),
                    customer_id: data.customer_id.to_string(),
                    status: data.status.clone(),
                    total_amount: data.total_amount,
                    currency: data.currency.clone(),
                    payment_status: data.payment_status.clone(),
                    fulfillment_status: data.fulfillment_status.clone(),
                    tracking_number: data.tracking_number.clone(),
                    items: js_items,
                    version: data.version,
                    created_at: data.created_at.clone(),
                    updated_at: data.updated_at.clone(),
                }
            })
            .collect();

        serde_wasm_bindgen::to_value(&orders).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Update order status.
    #[wasm_bindgen(js_name = updateStatus)]
    pub fn update_status(&self, id: &str, status: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        // Update the order
        {
            let data = store
                .orders
                .get_mut(&uuid)
                .ok_or_else(|| JsValue::from_str("Order not found"))?;

            data.status = status.to_string();
            data.updated_at = Utc::now().to_rfc3339();
        }

        // Now get immutable references for the response
        let data = store.orders.get(&uuid).unwrap();
        let items = store.order_items.get(&uuid).cloned().unwrap_or_default();
        let js_items: Vec<JsOrderItem> = items.iter().map(|i| i.into()).collect();

        let js_order = JsOrder {
            id: data.id.to_string(),
            order_number: data.order_number.clone(),
            customer_id: data.customer_id.to_string(),
            status: data.status.clone(),
            total_amount: data.total_amount,
            currency: data.currency.clone(),
            payment_status: data.payment_status.clone(),
            fulfillment_status: data.fulfillment_status.clone(),
            tracking_number: data.tracking_number.clone(),
            items: js_items,
            version: data.version,
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        };

        serde_wasm_bindgen::to_value(&js_order).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Ship an order.
    pub fn ship(&self, id: &str, tracking_number: Option<String>) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        // Update the order
        {
            let data = store
                .orders
                .get_mut(&uuid)
                .ok_or_else(|| JsValue::from_str("Order not found"))?;

            data.status = "shipped".to_string();
            data.fulfillment_status = "shipped".to_string();
            data.tracking_number = tracking_number;
            data.updated_at = Utc::now().to_rfc3339();
        }

        // Now get immutable references for the response
        let data = store.orders.get(&uuid).unwrap();
        let items = store.order_items.get(&uuid).cloned().unwrap_or_default();
        let js_items: Vec<JsOrderItem> = items.iter().map(|i| i.into()).collect();

        let js_order = JsOrder {
            id: data.id.to_string(),
            order_number: data.order_number.clone(),
            customer_id: data.customer_id.to_string(),
            status: data.status.clone(),
            total_amount: data.total_amount,
            currency: data.currency.clone(),
            payment_status: data.payment_status.clone(),
            fulfillment_status: data.fulfillment_status.clone(),
            tracking_number: data.tracking_number.clone(),
            items: js_items,
            version: data.version,
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        };

        serde_wasm_bindgen::to_value(&js_order).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Cancel an order.
    pub fn cancel(&self, id: &str) -> Result<JsValue, JsValue> {
        self.update_status(id, "cancelled")
    }

    /// Count orders.
    pub fn count(&self) -> u32 {
        self.store.borrow().orders.len() as u32
    }
}

// ============================================================================
// Products API
// ============================================================================

/// Product catalog operations.
#[wasm_bindgen]
pub struct Products {
    store: StoreRef,
}

#[wasm_bindgen]
impl Products {
    /// Create a new product.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateProductInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();
        let slug = input.name.to_lowercase().replace(' ', "-");

        let data = ProductData {
            id,
            name: input.name.clone(),
            slug,
            description: input.description.unwrap_or_default(),
            status: "draft".to_string(),
            created_at: now.clone(),
            updated_at: now,
        };

        let mut store = self.store.borrow_mut();
        store.products.insert(id, data.clone());

        // Create variants if provided
        if let Some(variants) = input.variants {
            for (i, v) in variants.into_iter().enumerate() {
                let variant_id = Uuid::new_v4();
                let variant = VariantData {
                    id: variant_id,
                    product_id: id,
                    sku: v.sku,
                    name: v.name.unwrap_or_else(|| input.name.clone()),
                    price: v.price,
                    compare_at_price: v.compare_at_price,
                    is_default: i == 0,
                };
                store.variants.insert(variant_id, variant);
            }
        }

        let js_product: JsProduct = (&data).into();
        serde_wasm_bindgen::to_value(&js_product).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a product by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.products.get(&uuid) {
            Some(data) => {
                let js_product: JsProduct = data.into();
                serde_wasm_bindgen::to_value(&js_product)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Get a product variant by SKU.
    #[wasm_bindgen(js_name = getVariantBySku)]
    pub fn get_variant_by_sku(&self, sku: &str) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();

        match store.variants.values().find(|v| v.sku == sku) {
            Some(data) => {
                let js_variant: JsProductVariant = data.into();
                serde_wasm_bindgen::to_value(&js_variant)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// List all products.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let products: Vec<JsProduct> = store.products.values().map(|data| data.into()).collect();

        serde_wasm_bindgen::to_value(&products).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count products.
    pub fn count(&self) -> u32 {
        self.store.borrow().products.len() as u32
    }
}

// ============================================================================
// Inventory API
// ============================================================================

/// Inventory management operations.
#[wasm_bindgen]
pub struct Inventory {
    store: StoreRef,
}

#[wasm_bindgen]
impl Inventory {
    /// Create a new inventory item.
    #[wasm_bindgen(js_name = createItem)]
    pub fn create_item(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateInventoryItemInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut store = self.store.borrow_mut();
        store.next_inventory_id += 1;
        let id = store.next_inventory_id;

        let data = InventoryItemData {
            id,
            sku: input.sku.clone(),
            name: input.name,
            description: input.description,
            unit_of_measure: "each".to_string(),
            is_active: true,
        };

        let balance = InventoryBalanceData {
            item_id: id,
            on_hand: input.initial_quantity.unwrap_or(0.0),
            allocated: 0.0,
        };

        store.inventory_items.insert(id, data.clone());
        store.inventory_by_sku.insert(input.sku, id);
        store.inventory_balances.insert(id, balance);

        let js_item: JsInventoryItem = (&data).into();
        serde_wasm_bindgen::to_value(&js_item).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get stock level for a SKU.
    #[wasm_bindgen(js_name = getStock)]
    pub fn get_stock(&self, sku: &str) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();

        let item_id = match store.inventory_by_sku.get(sku) {
            Some(id) => *id,
            None => return Ok(JsValue::NULL),
        };

        let item = match store.inventory_items.get(&item_id) {
            Some(i) => i,
            None => return Ok(JsValue::NULL),
        };

        let balance = match store.inventory_balances.get(&item_id) {
            Some(b) => b,
            None => return Ok(JsValue::NULL),
        };

        let stock = JsStockLevel {
            sku: item.sku.clone(),
            name: item.name.clone(),
            total_on_hand: balance.on_hand,
            total_allocated: balance.allocated,
            total_available: balance.on_hand - balance.allocated,
        };

        serde_wasm_bindgen::to_value(&stock).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Adjust inventory quantity.
    pub fn adjust(&self, sku: &str, quantity: f64, _reason: &str) -> Result<(), JsValue> {
        let mut store = self.store.borrow_mut();

        let item_id = store
            .inventory_by_sku
            .get(sku)
            .copied()
            .ok_or_else(|| JsValue::from_str("SKU not found"))?;

        let balance = store
            .inventory_balances
            .get_mut(&item_id)
            .ok_or_else(|| JsValue::from_str("Balance not found"))?;

        balance.on_hand += quantity;
        Ok(())
    }

    /// Reserve inventory for an order.
    pub fn reserve(
        &self,
        sku: &str,
        quantity: f64,
        reference_type: &str,
        reference_id: &str,
    ) -> Result<JsValue, JsValue> {
        let mut store = self.store.borrow_mut();

        let item_id = store
            .inventory_by_sku
            .get(sku)
            .copied()
            .ok_or_else(|| JsValue::from_str("SKU not found"))?;

        let balance = store
            .inventory_balances
            .get_mut(&item_id)
            .ok_or_else(|| JsValue::from_str("Balance not found"))?;

        let available = balance.on_hand - balance.allocated;
        if quantity > available {
            return Err(JsValue::from_str("Insufficient stock"));
        }

        balance.allocated += quantity;

        let id = Uuid::new_v4();
        let reservation = ReservationData {
            id,
            item_id,
            quantity,
            status: "pending".to_string(),
            reference_type: reference_type.to_string(),
            reference_id: reference_id.to_string(),
        };

        store.reservations.insert(id, reservation.clone());

        let js_reservation: JsReservation = (&reservation).into();
        serde_wasm_bindgen::to_value(&js_reservation).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Confirm a reservation.
    #[wasm_bindgen(js_name = confirmReservation)]
    pub fn confirm_reservation(&self, reservation_id: &str) -> Result<(), JsValue> {
        let uuid =
            Uuid::parse_str(reservation_id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let reservation = store
            .reservations
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Reservation not found"))?;

        reservation.status = "confirmed".to_string();
        Ok(())
    }

    /// Release a reservation.
    #[wasm_bindgen(js_name = releaseReservation)]
    pub fn release_reservation(&self, reservation_id: &str) -> Result<(), JsValue> {
        let uuid =
            Uuid::parse_str(reservation_id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let reservation = store
            .reservations
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Reservation not found"))?;

        if reservation.status == "released" {
            return Ok(());
        }

        let quantity = reservation.quantity;
        let item_id = reservation.item_id;
        reservation.status = "released".to_string();

        let balance = store
            .inventory_balances
            .get_mut(&item_id)
            .ok_or_else(|| JsValue::from_str("Balance not found"))?;

        balance.allocated -= quantity;
        Ok(())
    }
}

// ============================================================================
// Returns API
// ============================================================================

/// Return processing operations.
#[wasm_bindgen]
pub struct Returns {
    store: StoreRef,
}

#[wasm_bindgen]
impl Returns {
    /// Create a new return request.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateReturnInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let order_id = Uuid::parse_str(&input.order_id)
            .map_err(|_| JsValue::from_str("Invalid order UUID"))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let data = ReturnData {
            id,
            order_id,
            status: "requested".to_string(),
            reason: input.reason,
            reason_details: input.reason_details,
            version: 1,
            created_at: now,
        };

        let mut store = self.store.borrow_mut();
        store.returns.insert(id, data.clone());

        // Create return items
        let items: Vec<ReturnItemData> = input
            .items
            .into_iter()
            .map(|i| ReturnItemData {
                id: Uuid::new_v4(),
                return_id: id,
                order_item_id: Uuid::parse_str(&i.order_item_id).unwrap_or_default(),
                quantity: i.quantity,
            })
            .collect();
        store.return_items.insert(id, items);

        let js_return: JsReturn = (&data).into();
        serde_wasm_bindgen::to_value(&js_return).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a return by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.returns.get(&uuid) {
            Some(data) => {
                let js_return: JsReturn = data.into();
                serde_wasm_bindgen::to_value(&js_return)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Approve a return request.
    pub fn approve(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .returns
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Return not found"))?;

        data.status = "approved".to_string();

        let js_return: JsReturn = (&*data).into();
        serde_wasm_bindgen::to_value(&js_return).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Reject a return request.
    pub fn reject(&self, id: &str, _reason: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .returns
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Return not found"))?;

        data.status = "rejected".to_string();

        let js_return: JsReturn = (&*data).into();
        serde_wasm_bindgen::to_value(&js_return).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all returns.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let returns: Vec<JsReturn> = store.returns.values().map(|data| data.into()).collect();

        serde_wasm_bindgen::to_value(&returns).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count returns.
    pub fn count(&self) -> u32 {
        self.store.borrow().returns.len() as u32
    }
}

// ============================================================================
// Payments API
// ============================================================================

/// Payment processing operations.
#[wasm_bindgen]
pub struct Payments {
    store: StoreRef,
}

#[wasm_bindgen]
impl Payments {
    /// Create a new payment.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreatePaymentInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_payment_number += 1;
        let payment_number = format!("PAY-{}", store.next_payment_number);

        let data = PaymentData {
            id,
            payment_number,
            order_id: input.order_id.as_ref().and_then(|s| Uuid::parse_str(s).ok()),
            customer_id: input.customer_id.as_ref().and_then(|s| Uuid::parse_str(s).ok()),
            amount: input.amount,
            currency: input.currency.unwrap_or_else(|| "USD".to_string()),
            status: "pending".to_string(),
            payment_method: input.payment_method,
            version: 1,
            created_at: now.clone(),
            updated_at: now,
        };

        store.payments.insert(id, data.clone());

        let js_payment: JsPayment = (&data).into();
        serde_wasm_bindgen::to_value(&js_payment).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a payment by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.payments.get(&uuid) {
            Some(data) => {
                let js_payment: JsPayment = data.into();
                serde_wasm_bindgen::to_value(&js_payment)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Complete a payment.
    pub fn complete(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .payments
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Payment not found"))?;

        data.status = "completed".to_string();
        data.updated_at = Utc::now().to_rfc3339();

        let js_payment: JsPayment = (&*data).into();
        serde_wasm_bindgen::to_value(&js_payment).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Create a refund.
    pub fn refund(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateRefundInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let payment_id = Uuid::parse_str(&input.payment_id)
            .map_err(|_| JsValue::from_str("Invalid payment UUID"))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let data = RefundData {
            id,
            payment_id,
            amount: input.amount,
            reason: input.reason,
            status: "pending".to_string(),
            created_at: now,
        };

        self.store.borrow_mut().refunds.insert(id, data.clone());

        let js_refund: JsRefund = (&data).into();
        serde_wasm_bindgen::to_value(&js_refund).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all payments.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let payments: Vec<JsPayment> = store.payments.values().map(|data| data.into()).collect();
        serde_wasm_bindgen::to_value(&payments).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count payments.
    pub fn count(&self) -> u32 {
        self.store.borrow().payments.len() as u32
    }
}

// ============================================================================
// Shipments API
// ============================================================================

/// Shipment management operations.
#[wasm_bindgen]
pub struct Shipments {
    store: StoreRef,
}

#[wasm_bindgen]
impl Shipments {
    /// Create a new shipment.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateShipmentInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let order_id = Uuid::parse_str(&input.order_id)
            .map_err(|_| JsValue::from_str("Invalid order UUID"))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_shipment_number += 1;
        let shipment_number = format!("SHP-{}", store.next_shipment_number);

        let data = ShipmentData {
            id,
            shipment_number,
            order_id,
            carrier: input.carrier,
            tracking_number: input.tracking_number,
            status: "pending".to_string(),
            shipped_at: None,
            delivered_at: None,
            version: 1,
            created_at: now.clone(),
            updated_at: now,
        };

        store.shipments.insert(id, data.clone());

        let js_shipment: JsShipment = (&data).into();
        serde_wasm_bindgen::to_value(&js_shipment).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a shipment by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.shipments.get(&uuid) {
            Some(data) => {
                let js_shipment: JsShipment = data.into();
                serde_wasm_bindgen::to_value(&js_shipment)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Ship a shipment.
    pub fn ship(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .shipments
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Shipment not found"))?;

        let now = Utc::now().to_rfc3339();
        data.status = "shipped".to_string();
        data.shipped_at = Some(now.clone());
        data.updated_at = now;

        let js_shipment: JsShipment = (&*data).into();
        serde_wasm_bindgen::to_value(&js_shipment).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Mark shipment as delivered.
    pub fn deliver(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .shipments
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Shipment not found"))?;

        let now = Utc::now().to_rfc3339();
        data.status = "delivered".to_string();
        data.delivered_at = Some(now.clone());
        data.updated_at = now;

        let js_shipment: JsShipment = (&*data).into();
        serde_wasm_bindgen::to_value(&js_shipment).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all shipments.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let shipments: Vec<JsShipment> = store.shipments.values().map(|data| data.into()).collect();
        serde_wasm_bindgen::to_value(&shipments).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count shipments.
    pub fn count(&self) -> u32 {
        self.store.borrow().shipments.len() as u32
    }
}

// ============================================================================
// Warranties API
// ============================================================================

/// Warranty management operations.
#[wasm_bindgen]
pub struct Warranties {
    store: StoreRef,
}

#[wasm_bindgen]
impl Warranties {
    /// Create a new warranty.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateWarrantyInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let customer_id = Uuid::parse_str(&input.customer_id)
            .map_err(|_| JsValue::from_str("Invalid customer UUID"))?;

        let now = Utc::now();
        let duration_months = input.duration_months.unwrap_or(12);
        let end_date = now + chrono::Duration::days(duration_months as i64 * 30);
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_warranty_number += 1;
        let warranty_number = format!("WTY-{}", store.next_warranty_number);

        let data = WarrantyData {
            id,
            warranty_number,
            customer_id,
            product_id: input.product_id.as_ref().and_then(|s| Uuid::parse_str(s).ok()),
            order_id: input.order_id.as_ref().and_then(|s| Uuid::parse_str(s).ok()),
            status: "active".to_string(),
            duration_months,
            start_date: now.to_rfc3339(),
            end_date: end_date.to_rfc3339(),
            created_at: now.to_rfc3339(),
        };

        store.warranties.insert(id, data.clone());

        let js_warranty: JsWarranty = (&data).into();
        serde_wasm_bindgen::to_value(&js_warranty).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a warranty by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.warranties.get(&uuid) {
            Some(data) => {
                let js_warranty: JsWarranty = data.into();
                serde_wasm_bindgen::to_value(&js_warranty)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// File a warranty claim.
    #[wasm_bindgen(js_name = createClaim)]
    pub fn create_claim(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateWarrantyClaimInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let warranty_id = Uuid::parse_str(&input.warranty_id)
            .map_err(|_| JsValue::from_str("Invalid warranty UUID"))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_claim_number += 1;
        let claim_number = format!("CLM-{}", store.next_claim_number);

        let data = WarrantyClaimData {
            id,
            claim_number,
            warranty_id,
            issue_description: input.issue_description,
            status: "submitted".to_string(),
            resolution: None,
            created_at: now,
        };

        store.warranty_claims.insert(id, data.clone());

        let js_claim: JsWarrantyClaim = (&data).into();
        serde_wasm_bindgen::to_value(&js_claim).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Approve a warranty claim.
    #[wasm_bindgen(js_name = approveClaim)]
    pub fn approve_claim(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .warranty_claims
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Claim not found"))?;

        data.status = "approved".to_string();

        let js_claim: JsWarrantyClaim = (&*data).into();
        serde_wasm_bindgen::to_value(&js_claim).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all warranties.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let warranties: Vec<JsWarranty> = store.warranties.values().map(|data| data.into()).collect();
        serde_wasm_bindgen::to_value(&warranties).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count warranties.
    pub fn count(&self) -> u32 {
        self.store.borrow().warranties.len() as u32
    }
}

// ============================================================================
// Purchase Orders API
// ============================================================================

/// Purchase order management operations.
#[wasm_bindgen]
pub struct PurchaseOrders {
    store: StoreRef,
}

#[wasm_bindgen]
impl PurchaseOrders {
    /// Create a new supplier.
    #[wasm_bindgen(js_name = createSupplier)]
    pub fn create_supplier(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateSupplierInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_supplier_code += 1;
        let supplier_code = format!("SUP-{}", store.next_supplier_code);

        let data = SupplierData {
            id,
            supplier_code,
            name: input.name,
            email: input.email,
            phone: input.phone,
            status: "active".to_string(),
            created_at: now,
        };

        store.suppliers.insert(id, data.clone());

        let js_supplier: JsSupplier = (&data).into();
        serde_wasm_bindgen::to_value(&js_supplier).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a supplier by ID.
    #[wasm_bindgen(js_name = getSupplier)]
    pub fn get_supplier(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.suppliers.get(&uuid) {
            Some(data) => {
                let js_supplier: JsSupplier = data.into();
                serde_wasm_bindgen::to_value(&js_supplier)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Create a new purchase order.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreatePurchaseOrderInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let supplier_id = Uuid::parse_str(&input.supplier_id)
            .map_err(|_| JsValue::from_str("Invalid supplier UUID"))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_po_number += 1;
        let po_number = format!("PO-{}", store.next_po_number);

        let data = PurchaseOrderData {
            id,
            po_number,
            supplier_id,
            status: "draft".to_string(),
            total_amount: 0.0,
            currency: input.currency.unwrap_or_else(|| "USD".to_string()),
            created_at: now.clone(),
            updated_at: now,
        };

        store.purchase_orders.insert(id, data.clone());

        let js_po: JsPurchaseOrder = (&data).into();
        serde_wasm_bindgen::to_value(&js_po).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a purchase order by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.purchase_orders.get(&uuid) {
            Some(data) => {
                let js_po: JsPurchaseOrder = data.into();
                serde_wasm_bindgen::to_value(&js_po)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Submit a PO for approval.
    pub fn submit(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .purchase_orders
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("PO not found"))?;

        data.status = "pending_approval".to_string();
        data.updated_at = Utc::now().to_rfc3339();

        let js_po: JsPurchaseOrder = (&*data).into();
        serde_wasm_bindgen::to_value(&js_po).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Approve a purchase order.
    pub fn approve(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .purchase_orders
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("PO not found"))?;

        data.status = "approved".to_string();
        data.updated_at = Utc::now().to_rfc3339();

        let js_po: JsPurchaseOrder = (&*data).into();
        serde_wasm_bindgen::to_value(&js_po).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all purchase orders.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let pos: Vec<JsPurchaseOrder> = store.purchase_orders.values().map(|data| data.into()).collect();
        serde_wasm_bindgen::to_value(&pos).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count purchase orders.
    pub fn count(&self) -> u32 {
        self.store.borrow().purchase_orders.len() as u32
    }
}

// ============================================================================
// Invoices API
// ============================================================================

/// Invoice management operations.
#[wasm_bindgen]
pub struct Invoices {
    store: StoreRef,
}

#[wasm_bindgen]
impl Invoices {
    /// Create a new invoice.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateInvoiceInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let customer_id = Uuid::parse_str(&input.customer_id)
            .map_err(|_| JsValue::from_str("Invalid customer UUID"))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_invoice_number += 1;
        let invoice_number = format!("INV-{}", store.next_invoice_number);

        let tax_amount = input.tax_amount.unwrap_or(0.0);
        let total = input.subtotal + tax_amount;

        let data = InvoiceData {
            id,
            invoice_number,
            customer_id,
            order_id: input.order_id.as_ref().and_then(|s| Uuid::parse_str(s).ok()),
            status: "draft".to_string(),
            subtotal: input.subtotal,
            tax_amount,
            total,
            amount_paid: 0.0,
            due_date: input.due_date,
            created_at: now.clone(),
            updated_at: now,
        };

        store.invoices.insert(id, data.clone());

        let js_invoice: JsInvoice = (&data).into();
        serde_wasm_bindgen::to_value(&js_invoice).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get an invoice by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.invoices.get(&uuid) {
            Some(data) => {
                let js_invoice: JsInvoice = data.into();
                serde_wasm_bindgen::to_value(&js_invoice)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Send an invoice.
    pub fn send(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .invoices
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Invoice not found"))?;

        data.status = "sent".to_string();
        data.updated_at = Utc::now().to_rfc3339();

        let js_invoice: JsInvoice = (&*data).into();
        serde_wasm_bindgen::to_value(&js_invoice).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Record a payment on an invoice.
    #[wasm_bindgen(js_name = recordPayment)]
    pub fn record_payment(&self, id: &str, amount: f64) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .invoices
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Invoice not found"))?;

        data.amount_paid += amount;
        data.updated_at = Utc::now().to_rfc3339();

        if data.amount_paid >= data.total {
            data.status = "paid".to_string();
        } else {
            data.status = "partially_paid".to_string();
        }

        let js_invoice: JsInvoice = (&*data).into();
        serde_wasm_bindgen::to_value(&js_invoice).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all invoices.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let invoices: Vec<JsInvoice> = store.invoices.values().map(|data| data.into()).collect();
        serde_wasm_bindgen::to_value(&invoices).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count invoices.
    pub fn count(&self) -> u32 {
        self.store.borrow().invoices.len() as u32
    }
}

// ============================================================================
// BOM API
// ============================================================================

/// Bill of Materials management operations.
#[wasm_bindgen]
pub struct Bom {
    store: StoreRef,
}

#[wasm_bindgen]
impl Bom {
    /// Create a new bill of materials.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateBomInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_bom_number += 1;
        let bom_number = format!("BOM-{}", store.next_bom_number);

        let data = BomData {
            id,
            bom_number,
            sku: input.sku,
            name: input.name,
            description: input.description,
            status: "draft".to_string(),
            version: 1,
            created_at: now.clone(),
            updated_at: now,
        };

        store.boms.insert(id, data.clone());
        store.bom_components.insert(id, Vec::new());

        let js_bom: JsBom = (&data).into();
        serde_wasm_bindgen::to_value(&js_bom).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a BOM by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.boms.get(&uuid) {
            Some(data) => {
                let js_bom: JsBom = data.into();
                serde_wasm_bindgen::to_value(&js_bom)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Add a component to a BOM.
    #[wasm_bindgen(js_name = addComponent)]
    pub fn add_component(&self, bom_id: &str, input: JsValue) -> Result<JsValue, JsValue> {
        let bom_uuid = Uuid::parse_str(bom_id).map_err(|_| JsValue::from_str("Invalid BOM UUID"))?;
        let input: AddBomComponentInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let component_id = Uuid::new_v4();
        let component = BomComponentData {
            id: component_id,
            bom_id: bom_uuid,
            component_sku: input.component_sku,
            component_name: input.component_name,
            quantity: input.quantity,
            unit_of_measure: input.unit_of_measure.unwrap_or_else(|| "each".to_string()),
        };

        let mut store = self.store.borrow_mut();
        store
            .bom_components
            .entry(bom_uuid)
            .or_default()
            .push(component.clone());

        let js_component: JsBomComponent = (&component).into();
        serde_wasm_bindgen::to_value(&js_component).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get components for a BOM.
    #[wasm_bindgen(js_name = getComponents)]
    pub fn get_components(&self, bom_id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(bom_id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        let components: Vec<JsBomComponent> = store
            .bom_components
            .get(&uuid)
            .map(|c| c.iter().map(|data| data.into()).collect())
            .unwrap_or_default();

        serde_wasm_bindgen::to_value(&components).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Activate a BOM.
    pub fn activate(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .boms
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("BOM not found"))?;

        data.status = "active".to_string();
        data.updated_at = Utc::now().to_rfc3339();

        let js_bom: JsBom = (&*data).into();
        serde_wasm_bindgen::to_value(&js_bom).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all BOMs.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let boms: Vec<JsBom> = store.boms.values().map(|data| data.into()).collect();
        serde_wasm_bindgen::to_value(&boms).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count BOMs.
    pub fn count(&self) -> u32 {
        self.store.borrow().boms.len() as u32
    }
}

// ============================================================================
// Work Orders API
// ============================================================================

/// Work order management operations.
#[wasm_bindgen]
pub struct WorkOrders {
    store: StoreRef,
}

#[wasm_bindgen]
impl WorkOrders {
    /// Create a new work order.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateWorkOrderInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let bom_id = Uuid::parse_str(&input.bom_id)
            .map_err(|_| JsValue::from_str("Invalid BOM UUID"))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_work_order_number += 1;
        let work_order_number = format!("WO-{}", store.next_work_order_number);

        let data = WorkOrderData {
            id,
            work_order_number,
            bom_id,
            status: "draft".to_string(),
            quantity_to_build: input.quantity_to_build,
            quantity_built: 0.0,
            priority: input.priority.unwrap_or_else(|| "normal".to_string()),
            scheduled_start: input.scheduled_start,
            scheduled_end: input.scheduled_end,
            version: 1,
            created_at: now.clone(),
            updated_at: now,
        };

        store.work_orders.insert(id, data.clone());

        let js_wo: JsWorkOrder = (&data).into();
        serde_wasm_bindgen::to_value(&js_wo).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a work order by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.work_orders.get(&uuid) {
            Some(data) => {
                let js_wo: JsWorkOrder = data.into();
                serde_wasm_bindgen::to_value(&js_wo)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Start a work order.
    pub fn start(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .work_orders
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Work order not found"))?;

        data.status = "in_progress".to_string();
        data.updated_at = Utc::now().to_rfc3339();

        let js_wo: JsWorkOrder = (&*data).into();
        serde_wasm_bindgen::to_value(&js_wo).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Record production output.
    #[wasm_bindgen(js_name = recordOutput)]
    pub fn record_output(&self, id: &str, quantity: f64) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .work_orders
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Work order not found"))?;

        data.quantity_built += quantity;
        data.updated_at = Utc::now().to_rfc3339();

        if data.quantity_built >= data.quantity_to_build {
            data.status = "completed".to_string();
        }

        let js_wo: JsWorkOrder = (&*data).into();
        serde_wasm_bindgen::to_value(&js_wo).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Complete a work order.
    pub fn complete(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .work_orders
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Work order not found"))?;

        data.status = "completed".to_string();
        data.updated_at = Utc::now().to_rfc3339();

        let js_wo: JsWorkOrder = (&*data).into();
        serde_wasm_bindgen::to_value(&js_wo).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all work orders.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let work_orders: Vec<JsWorkOrder> = store.work_orders.values().map(|data| data.into()).collect();
        serde_wasm_bindgen::to_value(&work_orders).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count work orders.
    pub fn count(&self) -> u32 {
        self.store.borrow().work_orders.len() as u32
    }
}

// ============================================================================
// Carts API
// ============================================================================

/// Cart and checkout management operations.
#[wasm_bindgen]
pub struct Carts {
    store: StoreRef,
}

#[wasm_bindgen]
impl Carts {
    /// Create a new cart.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateCartInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let customer_id = input
            .customer_id
            .map(|id| Uuid::parse_str(&id))
            .transpose()
            .map_err(|_| JsValue::from_str("Invalid customer UUID"))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_cart_number += 1;
        let cart_number = format!("CART-{}", store.next_cart_number);

        let data = CartData {
            id,
            cart_number,
            customer_id,
            status: "active".to_string(),
            currency: input.currency.unwrap_or_else(|| "USD".to_string()),
            subtotal: 0.0,
            tax_amount: 0.0,
            shipping_amount: 0.0,
            discount_amount: 0.0,
            grand_total: 0.0,
            customer_email: input.customer_email,
            customer_name: input.customer_name,
            payment_method: None,
            payment_status: "pending".to_string(),
            fulfillment_type: "shipping".to_string(),
            shipping_method: None,
            coupon_code: None,
            notes: None,
            created_at: now.clone(),
            updated_at: now,
            expires_at: None,
        };

        store.carts.insert(id, data.clone());
        store.cart_items.insert(id, Vec::new());

        let items: Vec<JsCartItem> = Vec::new();
        let js_cart = JsCart {
            id: data.id.to_string(),
            cart_number: data.cart_number.clone(),
            customer_id: data.customer_id.map(|id| id.to_string()),
            status: data.status.clone(),
            currency: data.currency.clone(),
            subtotal: data.subtotal,
            tax_amount: data.tax_amount,
            shipping_amount: data.shipping_amount,
            discount_amount: data.discount_amount,
            grand_total: data.grand_total,
            customer_email: data.customer_email.clone(),
            customer_name: data.customer_name.clone(),
            payment_method: data.payment_method.clone(),
            payment_status: data.payment_status.clone(),
            fulfillment_type: data.fulfillment_type.clone(),
            shipping_method: data.shipping_method.clone(),
            coupon_code: data.coupon_code.clone(),
            notes: data.notes.clone(),
            item_count: 0,
            items,
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
            expires_at: data.expires_at.clone(),
        };

        serde_wasm_bindgen::to_value(&js_cart).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a cart by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.carts.get(&uuid) {
            Some(data) => {
                let items: Vec<JsCartItem> = store
                    .cart_items
                    .get(&uuid)
                    .map(|items| items.iter().map(|i| i.into()).collect())
                    .unwrap_or_default();

                let js_cart = JsCart {
                    id: data.id.to_string(),
                    cart_number: data.cart_number.clone(),
                    customer_id: data.customer_id.map(|id| id.to_string()),
                    status: data.status.clone(),
                    currency: data.currency.clone(),
                    subtotal: data.subtotal,
                    tax_amount: data.tax_amount,
                    shipping_amount: data.shipping_amount,
                    discount_amount: data.discount_amount,
                    grand_total: data.grand_total,
                    customer_email: data.customer_email.clone(),
                    customer_name: data.customer_name.clone(),
                    payment_method: data.payment_method.clone(),
                    payment_status: data.payment_status.clone(),
                    fulfillment_type: data.fulfillment_type.clone(),
                    shipping_method: data.shipping_method.clone(),
                    coupon_code: data.coupon_code.clone(),
                    notes: data.notes.clone(),
                    item_count: items.len(),
                    items,
                    created_at: data.created_at.clone(),
                    updated_at: data.updated_at.clone(),
                    expires_at: data.expires_at.clone(),
                };

                serde_wasm_bindgen::to_value(&js_cart)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Add an item to the cart.
    #[wasm_bindgen(js_name = addItem)]
    pub fn add_item(&self, cart_id: &str, input: JsValue) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(cart_id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let input: AddCartItemInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut store = self.store.borrow_mut();

        // Check if cart exists
        if !store.carts.contains_key(&uuid) {
            return Err(JsValue::from_str("Cart not found"));
        }

        let item_id = Uuid::new_v4();
        let total = input.unit_price * input.quantity as f64;

        let item = CartItemData {
            id: item_id,
            cart_id: uuid,
            sku: input.sku,
            name: input.name,
            description: input.description,
            quantity: input.quantity,
            unit_price: input.unit_price,
            total,
        };

        let js_item: JsCartItem = (&item).into();

        // Add item to cart items collection first
        store.cart_items.entry(uuid).or_default().push(item);

        // Now update cart totals
        let cart = store.carts.get_mut(&uuid).unwrap();
        cart.subtotal += total;
        cart.grand_total = cart.subtotal + cart.tax_amount + cart.shipping_amount - cart.discount_amount;
        cart.updated_at = Utc::now().to_rfc3339();

        serde_wasm_bindgen::to_value(&js_item).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Remove an item from the cart.
    #[wasm_bindgen(js_name = removeItem)]
    pub fn remove_item(&self, cart_id: &str, item_id: &str) -> Result<(), JsValue> {
        let cart_uuid = Uuid::parse_str(cart_id).map_err(|_| JsValue::from_str("Invalid cart UUID"))?;
        let item_uuid = Uuid::parse_str(item_id).map_err(|_| JsValue::from_str("Invalid item UUID"))?;

        let mut store = self.store.borrow_mut();

        // Check if cart exists
        if !store.carts.contains_key(&cart_uuid) {
            return Err(JsValue::from_str("Cart not found"));
        }

        // Remove item and get its total
        let item_total = if let Some(items) = store.cart_items.get_mut(&cart_uuid) {
            if let Some(pos) = items.iter().position(|i| i.id == item_uuid) {
                let item = items.remove(pos);
                Some(item.total)
            } else {
                None
            }
        } else {
            None
        };

        // Update cart totals if item was removed
        if let Some(total) = item_total {
            let cart = store.carts.get_mut(&cart_uuid).unwrap();
            cart.subtotal -= total;
            cart.grand_total = cart.subtotal + cart.tax_amount + cart.shipping_amount - cart.discount_amount;
            cart.updated_at = Utc::now().to_rfc3339();
        }

        Ok(())
    }

    /// Clear all items from the cart.
    #[wasm_bindgen(js_name = clearItems)]
    pub fn clear_items(&self, cart_id: &str) -> Result<(), JsValue> {
        let uuid = Uuid::parse_str(cart_id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        // Check if cart exists
        if !store.carts.contains_key(&uuid) {
            return Err(JsValue::from_str("Cart not found"));
        }

        // Clear items first
        store.cart_items.insert(uuid, Vec::new());

        // Now update cart totals
        let cart = store.carts.get_mut(&uuid).unwrap();
        cart.subtotal = 0.0;
        cart.grand_total = cart.tax_amount + cart.shipping_amount - cart.discount_amount;
        cart.updated_at = Utc::now().to_rfc3339();

        Ok(())
    }

    /// Set payment method.
    #[wasm_bindgen(js_name = setPayment)]
    pub fn set_payment(&self, id: &str, input: JsValue) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let input: SetCartPaymentInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut store = self.store.borrow_mut();

        // Check cart exists
        if !store.carts.contains_key(&uuid) {
            return Err(JsValue::from_str("Cart not found"));
        }

        // Get items first (immutable borrow)
        let items: Vec<JsCartItem> = store
            .cart_items
            .get(&uuid)
            .map(|items| items.iter().map(|i| i.into()).collect())
            .unwrap_or_default();

        // Now update the cart (mutable borrow)
        let cart = store.carts.get_mut(&uuid).unwrap();
        cart.payment_method = Some(input.payment_method);
        cart.updated_at = Utc::now().to_rfc3339();

        let js_cart = JsCart {
            id: cart.id.to_string(),
            cart_number: cart.cart_number.clone(),
            customer_id: cart.customer_id.map(|id| id.to_string()),
            status: cart.status.clone(),
            currency: cart.currency.clone(),
            subtotal: cart.subtotal,
            tax_amount: cart.tax_amount,
            shipping_amount: cart.shipping_amount,
            discount_amount: cart.discount_amount,
            grand_total: cart.grand_total,
            customer_email: cart.customer_email.clone(),
            customer_name: cart.customer_name.clone(),
            payment_method: cart.payment_method.clone(),
            payment_status: cart.payment_status.clone(),
            fulfillment_type: cart.fulfillment_type.clone(),
            shipping_method: cart.shipping_method.clone(),
            coupon_code: cart.coupon_code.clone(),
            notes: cart.notes.clone(),
            item_count: items.len(),
            items,
            created_at: cart.created_at.clone(),
            updated_at: cart.updated_at.clone(),
            expires_at: cart.expires_at.clone(),
        };

        serde_wasm_bindgen::to_value(&js_cart).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Complete the checkout and create an order.
    pub fn complete(&self, id: &str) -> Result<JsValue, JsValue> {
        let cart_uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;

        let mut store = self.store.borrow_mut();

        // First, extract all data we need from the cart (immutable borrow)
        let (cart_status, customer_id, grand_total, currency, notes) = {
            let cart = store
                .carts
                .get(&cart_uuid)
                .ok_or_else(|| JsValue::from_str("Cart not found"))?;

            // Check cart status
            if cart.status != "active" && cart.status != "ready_for_payment" {
                return Err(JsValue::from_str("Cart is not in a valid state for checkout"));
            }

            let customer_id = cart.customer_id.ok_or_else(|| JsValue::from_str("Cart has no customer"))?;

            (
                cart.status.clone(),
                customer_id,
                cart.grand_total,
                cart.currency.clone(),
                cart.notes.clone(),
            )
        };

        // Get cart items
        let items = store.cart_items.get(&cart_uuid).cloned().unwrap_or_default();
        if items.is_empty() {
            return Err(JsValue::from_str("Cannot complete checkout with empty cart"));
        }

        // Create order
        let now = Utc::now().to_rfc3339();
        let order_id = Uuid::new_v4();
        store.next_order_number += 1;
        let order_number = format!("ORD-{}", store.next_order_number);

        let order_items: Vec<OrderItemData> = items
            .iter()
            .map(|i| OrderItemData {
                id: Uuid::new_v4(),
                order_id,
                sku: i.sku.clone(),
                name: i.name.clone(),
                quantity: i.quantity,
                unit_price: i.unit_price,
                total: i.total,
            })
            .collect();

        let order = OrderData {
            id: order_id,
            order_number: order_number.clone(),
            customer_id,
            status: "pending".to_string(),
            total_amount: grand_total,
            currency: currency.clone(),
            payment_status: "paid".to_string(),
            fulfillment_status: "unfulfilled".to_string(),
            tracking_number: None,
            notes,
            version: 1,
            created_at: now.clone(),
            updated_at: now,
        };

        store.orders.insert(order_id, order);
        store.order_items.insert(order_id, order_items);

        // Update cart status (now we can get mutable borrow)
        if let Some(cart) = store.carts.get_mut(&cart_uuid) {
            cart.status = "completed".to_string();
            cart.payment_status = "paid".to_string();
            cart.updated_at = Utc::now().to_rfc3339();
        }

        let result = JsCheckoutResult {
            order_id: order_id.to_string(),
            order_number,
            cart_id: cart_uuid.to_string(),
            total_charged: grand_total,
            currency,
        };

        serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Cancel the cart.
    pub fn cancel(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        // Check cart exists
        if !store.carts.contains_key(&uuid) {
            return Err(JsValue::from_str("Cart not found"));
        }

        // Get items first (immutable borrow)
        let items: Vec<JsCartItem> = store
            .cart_items
            .get(&uuid)
            .map(|items| items.iter().map(|i| i.into()).collect())
            .unwrap_or_default();

        // Now update the cart (mutable borrow)
        let cart = store.carts.get_mut(&uuid).unwrap();
        cart.status = "cancelled".to_string();
        cart.updated_at = Utc::now().to_rfc3339();

        let js_cart = JsCart {
            id: cart.id.to_string(),
            cart_number: cart.cart_number.clone(),
            customer_id: cart.customer_id.map(|id| id.to_string()),
            status: cart.status.clone(),
            currency: cart.currency.clone(),
            subtotal: cart.subtotal,
            tax_amount: cart.tax_amount,
            shipping_amount: cart.shipping_amount,
            discount_amount: cart.discount_amount,
            grand_total: cart.grand_total,
            customer_email: cart.customer_email.clone(),
            customer_name: cart.customer_name.clone(),
            payment_method: cart.payment_method.clone(),
            payment_status: cart.payment_status.clone(),
            fulfillment_type: cart.fulfillment_type.clone(),
            shipping_method: cart.shipping_method.clone(),
            coupon_code: cart.coupon_code.clone(),
            notes: cart.notes.clone(),
            item_count: items.len(),
            items,
            created_at: cart.created_at.clone(),
            updated_at: cart.updated_at.clone(),
            expires_at: cart.expires_at.clone(),
        };

        serde_wasm_bindgen::to_value(&js_cart).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all carts.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let carts: Vec<JsCart> = store
            .carts
            .values()
            .map(|data| {
                let items: Vec<JsCartItem> = store
                    .cart_items
                    .get(&data.id)
                    .map(|items| items.iter().map(|i| i.into()).collect())
                    .unwrap_or_default();

                JsCart {
                    id: data.id.to_string(),
                    cart_number: data.cart_number.clone(),
                    customer_id: data.customer_id.map(|id| id.to_string()),
                    status: data.status.clone(),
                    currency: data.currency.clone(),
                    subtotal: data.subtotal,
                    tax_amount: data.tax_amount,
                    shipping_amount: data.shipping_amount,
                    discount_amount: data.discount_amount,
                    grand_total: data.grand_total,
                    customer_email: data.customer_email.clone(),
                    customer_name: data.customer_name.clone(),
                    payment_method: data.payment_method.clone(),
                    payment_status: data.payment_status.clone(),
                    fulfillment_type: data.fulfillment_type.clone(),
                    shipping_method: data.shipping_method.clone(),
                    coupon_code: data.coupon_code.clone(),
                    notes: data.notes.clone(),
                    item_count: items.len(),
                    items,
                    created_at: data.created_at.clone(),
                    updated_at: data.updated_at.clone(),
                    expires_at: data.expires_at.clone(),
                }
            })
            .collect();

        serde_wasm_bindgen::to_value(&carts).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count carts.
    pub fn count(&self) -> u32 {
        self.store.borrow().carts.len() as u32
    }

    /// Delete a cart.
    pub fn delete(&self, id: &str) -> Result<(), JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        store.carts.remove(&uuid);
        store.cart_items.remove(&uuid);

        Ok(())
    }
}
