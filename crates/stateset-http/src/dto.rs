//! Data Transfer Objects for HTTP request/response bodies.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_core::StockPolicy;
use stateset_primitives::{CustomerId, OrderId, ProductId, ReturnId};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// ============================================================================
// Pagination
// ============================================================================

/// Query parameters for paginated list endpoints.
#[derive(Debug, Clone, Deserialize, Serialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PaginationParams {
    /// Maximum number of results to return (default: 50).
    pub limit: Option<u32>,
    /// Number of results to skip (default: 0).
    pub offset: Option<u32>,
}

impl PaginationParams {
    /// Default page size.
    pub const DEFAULT_LIMIT: u32 = 50;
    /// Maximum allowed page size.
    pub const MAX_LIMIT: u32 = 200;

    /// Resolved limit with bounds checking.
    #[must_use]
    pub fn resolved_limit(&self) -> u32 {
        resolve_limit(self.limit)
    }

    /// Resolved offset.
    #[must_use]
    pub fn resolved_offset(&self) -> u32 {
        self.offset.unwrap_or(0)
    }
}

#[must_use]
fn resolve_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(PaginationParams::DEFAULT_LIMIT).clamp(1, PaginationParams::MAX_LIMIT)
}

/// Request one extra row to detect whether another page exists.
#[must_use]
pub const fn overfetch_limit(limit: u32) -> u32 {
    limit.saturating_add(1)
}

/// Trim an overfetched page back to the requested size and return `has_more`.
pub fn finalize_page<T>(items: &mut Vec<T>, requested_limit: u32) -> bool {
    let requested_limit = requested_limit as usize;
    let has_more = items.len() > requested_limit;
    if has_more {
        items.truncate(requested_limit);
    }
    has_more
}

// ============================================================================
// Cursor helpers
// ============================================================================

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

/// Encode a keyset cursor from `(sort_key, id)`.
#[must_use]
pub fn encode_cursor(sort_key: &str, id: &str) -> String {
    let payload = format!("{sort_key}\x00{id}");
    URL_SAFE_NO_PAD.encode(payload.as_bytes())
}

/// Decode a keyset cursor into `(sort_key, id)`.
#[must_use]
pub fn decode_cursor(cursor: &str) -> Option<(String, String)> {
    let bytes = URL_SAFE_NO_PAD.decode(cursor).ok()?;
    let s = String::from_utf8(bytes).ok()?;
    let (sort_key, id) = s.split_once('\x00')?;
    Some((sort_key.to_string(), id.to_string()))
}

// ============================================================================
// Filter query parameters
// ============================================================================

/// Query parameters for `GET /api/v1/orders` with filtering.
#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OrderFilterParams {
    /// Maximum number of results to return (default: 50).
    pub limit: Option<u32>,
    /// Number of results to skip (default: 0). Ignored when `after` cursor is set.
    pub offset: Option<u32>,
    /// Cursor for keyset pagination (opaque token from `next_cursor`).
    pub after: Option<String>,
    /// Filter by customer ID (UUID).
    pub customer_id: Option<String>,
    /// Filter by order status (pending, confirmed, shipped, delivered, cancelled).
    pub status: Option<String>,
    /// Filter by payment status (pending, paid, `partially_refunded`, refunded).
    pub payment_status: Option<String>,
    /// Filter by fulfillment status (unfulfilled, partial, fulfilled).
    pub fulfillment_status: Option<String>,
    /// Orders created on or after this date (RFC 3339, e.g. 2024-01-15T00:00:00Z).
    pub from_date: Option<String>,
    /// Orders created on or before this date (RFC 3339, e.g. 2024-12-31T23:59:59Z).
    pub to_date: Option<String>,
}

impl OrderFilterParams {
    /// Resolved limit with bounds checking.
    #[must_use]
    pub fn resolved_limit(&self) -> u32 {
        resolve_limit(self.limit)
    }

    /// Resolved offset.
    #[must_use]
    pub fn resolved_offset(&self) -> u32 {
        self.offset.unwrap_or(0)
    }
}

/// Query parameters for `GET /api/v1/customers` with filtering.
#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CustomerFilterParams {
    /// Maximum number of results to return (default: 50).
    pub limit: Option<u32>,
    /// Number of results to skip (default: 0). Ignored when `after` cursor is set.
    pub offset: Option<u32>,
    /// Cursor for keyset pagination (opaque token from `next_cursor`).
    pub after: Option<String>,
    /// Filter by email address (exact match).
    pub email: Option<String>,
    /// Filter by customer status (active, inactive, deleted).
    pub status: Option<String>,
    /// Filter by tag.
    pub tag: Option<String>,
    /// Filter by marketing opt-in.
    pub accepts_marketing: Option<bool>,
}

impl CustomerFilterParams {
    /// Resolved limit with bounds checking.
    #[must_use]
    pub fn resolved_limit(&self) -> u32 {
        resolve_limit(self.limit)
    }

    /// Resolved offset.
    #[must_use]
    pub fn resolved_offset(&self) -> u32 {
        self.offset.unwrap_or(0)
    }
}

/// Query parameters for `GET /api/v1/products` with filtering.
#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ProductFilterParams {
    /// Maximum number of results to return (default: 50).
    pub limit: Option<u32>,
    /// Number of results to skip (default: 0). Ignored when `after` cursor is set.
    pub offset: Option<u32>,
    /// Cursor for keyset pagination (opaque token from `next_cursor`).
    pub after: Option<String>,
    /// Filter by product status (draft, active, archived).
    pub status: Option<String>,
    /// Filter by product type (simple, digital, bundle, subscription, service).
    pub product_type: Option<String>,
    /// Full-text search across name and description.
    pub search: Option<String>,
    /// Filter by category.
    pub category: Option<String>,
    /// Minimum price filter.
    pub min_price: Option<String>,
    /// Maximum price filter.
    pub max_price: Option<String>,
    /// Filter by stock availability.
    pub in_stock: Option<bool>,
}

impl ProductFilterParams {
    /// Resolved limit with bounds checking.
    #[must_use]
    pub fn resolved_limit(&self) -> u32 {
        resolve_limit(self.limit)
    }

    /// Resolved offset.
    #[must_use]
    pub fn resolved_offset(&self) -> u32 {
        self.offset.unwrap_or(0)
    }
}

/// Query parameters for `GET /api/v1/returns` with filtering.
#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ReturnFilterParams {
    /// Maximum number of results to return (default: 50).
    pub limit: Option<u32>,
    /// Number of results to skip (default: 0). Ignored when `after` cursor is set.
    pub offset: Option<u32>,
    /// Cursor for keyset pagination (opaque token from `next_cursor`).
    pub after: Option<String>,
    /// Filter by order ID (UUID).
    pub order_id: Option<String>,
    /// Filter by customer ID (UUID).
    pub customer_id: Option<String>,
    /// Filter by return status (requested, approved, rejected, received, refunded, closed).
    pub status: Option<String>,
    /// Filter by return reason (defective, `wrong_item`, `not_as_described`, `changed_mind`, other).
    pub reason: Option<String>,
    /// Returns created on or after this date (RFC 3339).
    pub from_date: Option<String>,
    /// Returns created on or before this date (RFC 3339).
    pub to_date: Option<String>,
}

impl ReturnFilterParams {
    /// Resolved limit with bounds checking.
    #[must_use]
    pub fn resolved_limit(&self) -> u32 {
        resolve_limit(self.limit)
    }

    /// Resolved offset.
    #[must_use]
    pub fn resolved_offset(&self) -> u32 {
        self.offset.unwrap_or(0)
    }
}

// ============================================================================
// Orders
// ============================================================================

/// Request body for `POST /api/v1/orders`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateOrderRequest {
    /// The customer placing the order.
    #[schema(value_type = String, format = "uuid")]
    pub customer_id: CustomerId,
    /// Line items for the order.
    pub items: Vec<CreateOrderItemRequest>,
    /// ISO currency code (default: USD).
    pub currency: Option<String>,
    /// Shipping address.
    pub shipping_address: Option<AddressDto>,
    /// Billing address.
    pub billing_address: Option<AddressDto>,
    /// Optional notes.
    pub notes: Option<String>,
    /// Payment method identifier.
    pub payment_method: Option<String>,
    /// Shipping method identifier.
    pub shipping_method: Option<String>,
    /// What to do when a line cannot be fully reserved from stock.
    ///
    /// `allow_backorder` (default) reserves what is available and backorders
    /// the remainder; `reject_if_insufficient` fails the whole request with
    /// HTTP 400 and creates nothing.
    #[serde(default)]
    #[schema(value_type = Option<StockPolicyDto>)]
    pub stock_policy: StockPolicy,
}

/// Stock policy accepted by `POST /api/v1/orders` (`stock_policy`).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StockPolicyDto {
    /// Reserve what is available; backorder the remainder (default).
    AllowBackorder,
    /// Reject the order with `InsufficientStock` if any line is short.
    RejectIfInsufficient,
}

/// A single line item in a create-order request.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateOrderItemRequest {
    #[schema(value_type = String, format = "uuid")]
    pub product_id: ProductId,
    pub variant_id: Option<Uuid>,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    #[schema(value_type = String)]
    pub unit_price: Decimal,
    #[schema(value_type = Option<String>)]
    pub discount: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub tax_amount: Option<Decimal>,
}

/// Address DTO shared by orders and customers.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct AddressDto {
    pub line1: String,
    pub line2: Option<String>,
    pub city: String,
    pub state: Option<String>,
    pub postal_code: String,
    pub country: String,
}

/// Response body for a single order.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrderResponse {
    #[schema(value_type = String, format = "uuid")]
    pub id: OrderId,
    pub order_number: String,
    #[schema(value_type = String, format = "uuid")]
    pub customer_id: CustomerId,
    pub status: String,
    #[schema(value_type = String)]
    pub total_amount: Decimal,
    pub currency: String,
    pub payment_status: String,
    pub fulfillment_status: String,
    pub items: Vec<OrderItemResponse>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single line item in an order response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrderItemResponse {
    pub id: Uuid,
    #[schema(value_type = String, format = "uuid")]
    pub product_id: ProductId,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    #[schema(value_type = String)]
    pub unit_price: Decimal,
    #[schema(value_type = String)]
    pub total: Decimal,
}

/// Response body for `GET /api/v1/orders` (list).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OrderListResponse {
    pub orders: Vec<OrderResponse>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
    /// Opaque cursor for fetching the next page (keyset pagination).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Whether more results are available after this page.
    pub has_more: bool,
}

// ============================================================================
// Customers
// ============================================================================

/// Request body for `POST /api/v1/customers`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateCustomerRequest {
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    pub accepts_marketing: Option<bool>,
    pub tags: Option<Vec<String>>,
    #[schema(value_type = Option<Object>)]
    pub metadata: Option<serde_json::Value>,
}

/// Response body for a single customer.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CustomerResponse {
    #[schema(value_type = String, format = "uuid")]
    pub id: CustomerId,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    pub status: String,
    pub accepts_marketing: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request body for `PATCH /api/v1/customers/:id`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, ToSchema)]
pub struct UpdateCustomerRequest {
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub status: Option<String>,
    pub accepts_marketing: Option<bool>,
    pub tags: Option<Vec<String>>,
    #[schema(value_type = Option<Object>)]
    pub metadata: Option<serde_json::Value>,
}

/// Response body for `GET /api/v1/customers` (list).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CustomerListResponse {
    pub customers: Vec<CustomerResponse>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
    /// Opaque cursor for fetching the next page (keyset pagination).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Whether more results are available after this page.
    pub has_more: bool,
}

// ============================================================================
// Products
// ============================================================================

/// Request body for `POST /api/v1/products`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateProductRequest {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub product_type: Option<String>,
}

/// Response body for a single product.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProductResponse {
    #[schema(value_type = String, format = "uuid")]
    pub id: ProductId,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub status: String,
    pub product_type: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request body for `PATCH /api/v1/products/:id`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, ToSchema)]
pub struct UpdateProductRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub product_type: Option<String>,
}

/// Response body for `GET /api/v1/products` (list).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProductListResponse {
    pub products: Vec<ProductResponse>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
    /// Opaque cursor for fetching the next page (keyset pagination).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Whether more results are available after this page.
    pub has_more: bool,
}

// ============================================================================
// Inventory
// ============================================================================

/// Request body for `POST /api/v1/inventory/:sku/adjust`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct InventoryAdjustRequest {
    /// Signed quantity change (+/−).
    #[schema(value_type = String)]
    pub quantity: Decimal,
    /// Reason for the adjustment.
    pub reason: String,
    /// Deprecated: location-scoped adjustments are not supported by this endpoint.
    ///
    /// Supplying this field will return a validation error.
    pub location_id: Option<i32>,
}

/// Response body for inventory stock levels.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InventoryResponse {
    pub sku: String,
    pub name: String,
    #[schema(value_type = String)]
    pub total_on_hand: Decimal,
    #[schema(value_type = String)]
    pub total_allocated: Decimal,
    #[schema(value_type = String)]
    pub total_available: Decimal,
}

// ============================================================================
// Inventory (list)
// ============================================================================

/// Query parameters for `GET /api/v1/inventory` with filtering.
#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct InventoryFilterParams {
    /// Maximum number of results to return (default: 50).
    pub limit: Option<u32>,
    /// Number of results to skip (default: 0).
    pub offset: Option<u32>,
    /// Filter by SKU (exact match).
    pub sku: Option<String>,
    /// Filter by items below reorder point.
    pub below_reorder_point: Option<bool>,
    /// Filter by active status.
    pub is_active: Option<bool>,
}

impl InventoryFilterParams {
    /// Resolved limit with bounds checking.
    #[must_use]
    pub fn resolved_limit(&self) -> u32 {
        resolve_limit(self.limit)
    }

    /// Resolved offset.
    #[must_use]
    pub fn resolved_offset(&self) -> u32 {
        self.offset.unwrap_or(0)
    }
}

/// Response body for a single inventory item (list view).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InventoryItemResponse {
    pub id: i64,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub unit_of_measure: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Response body for `GET /api/v1/inventory` (list).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct InventoryListResponse {
    pub items: Vec<InventoryItemResponse>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
    pub has_more: bool,
}

// ============================================================================
// Shipments
// ============================================================================

/// Query parameters for `GET /api/v1/shipments` with filtering.
#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ShipmentFilterParams {
    /// Maximum number of results to return (default: 50).
    pub limit: Option<u32>,
    /// Number of results to skip (default: 0).
    pub offset: Option<u32>,
    /// Filter by order ID (UUID).
    pub order_id: Option<String>,
    /// Filter by shipment status (pending, shipped, `in_transit`, delivered, returned, cancelled).
    pub status: Option<String>,
    /// Filter by carrier (usps, ups, fedex, dhl, etc.).
    pub carrier: Option<String>,
    /// Filter by tracking number.
    pub tracking_number: Option<String>,
}

impl ShipmentFilterParams {
    /// Resolved limit with bounds checking.
    #[must_use]
    pub fn resolved_limit(&self) -> u32 {
        resolve_limit(self.limit)
    }

    /// Resolved offset.
    #[must_use]
    pub fn resolved_offset(&self) -> u32 {
        self.offset.unwrap_or(0)
    }
}

/// Request body for `POST /api/v1/shipments`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateShipmentRequest {
    #[schema(value_type = String, format = "uuid")]
    pub order_id: stateset_primitives::OrderId,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub shipping_method: Option<String>,
    pub recipient_name: Option<String>,
    pub notes: Option<String>,
}

/// Response body for a single shipment.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ShipmentResponse {
    #[schema(value_type = String, format = "uuid")]
    pub id: stateset_primitives::ShipmentId,
    pub shipment_number: String,
    #[schema(value_type = String, format = "uuid")]
    pub order_id: OrderId,
    pub status: String,
    pub carrier: String,
    pub shipping_method: String,
    pub tracking_number: Option<String>,
    pub tracking_url: Option<String>,
    pub recipient_name: String,
    #[schema(value_type = Option<String>)]
    pub shipping_cost: Option<Decimal>,
    pub shipped_at: Option<DateTime<Utc>>,
    pub estimated_delivery: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Response body for `GET /api/v1/shipments` (list).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ShipmentListResponse {
    pub shipments: Vec<ShipmentResponse>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
    pub has_more: bool,
}

// ============================================================================
// Payments
// ============================================================================

/// Query parameters for `GET /api/v1/payments` with filtering.
#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PaymentFilterParams {
    /// Maximum number of results to return (default: 50).
    pub limit: Option<u32>,
    /// Number of results to skip (default: 0).
    pub offset: Option<u32>,
    /// Filter by order ID (UUID).
    pub order_id: Option<String>,
    /// Filter by customer ID (UUID).
    pub customer_id: Option<String>,
    /// Filter by payment status (pending, authorized, captured, failed, refunded, `partially_refunded`, voided).
    pub status: Option<String>,
    /// Filter by payment method (`credit_card`, `debit_card`, `bank_transfer`, etc.).
    pub payment_method: Option<String>,
    /// Filter by processor name.
    pub processor: Option<String>,
    /// Minimum payment amount.
    pub min_amount: Option<String>,
    /// Maximum payment amount.
    pub max_amount: Option<String>,
    /// Payments created on or after this date (RFC 3339).
    pub from_date: Option<String>,
    /// Payments created on or before this date (RFC 3339).
    pub to_date: Option<String>,
}

impl PaymentFilterParams {
    /// Resolved limit with bounds checking.
    #[must_use]
    pub fn resolved_limit(&self) -> u32 {
        resolve_limit(self.limit)
    }

    /// Resolved offset.
    #[must_use]
    pub fn resolved_offset(&self) -> u32 {
        self.offset.unwrap_or(0)
    }
}

/// Request body for `POST /api/v1/payments`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreatePaymentRequest {
    #[schema(value_type = String, format = "uuid")]
    pub order_id: stateset_primitives::OrderId,
    #[schema(value_type = Option<String>, format = "uuid")]
    pub customer_id: Option<stateset_primitives::CustomerId>,
    pub payment_method: Option<String>,
    #[schema(value_type = String)]
    pub amount: Decimal,
    pub currency: Option<String>,
    pub external_id: Option<String>,
    pub description: Option<String>,
}

/// Request body for `POST /api/v1/payments/:id/refund`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateRefundRequest {
    #[schema(value_type = String)]
    pub amount: Decimal,
    pub reason: Option<String>,
    pub notes: Option<String>,
}

/// Response body for a single payment.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaymentResponse {
    #[schema(value_type = String, format = "uuid")]
    pub id: stateset_primitives::PaymentId,
    pub payment_number: String,
    #[schema(value_type = Option<String>, format = "uuid")]
    pub order_id: Option<OrderId>,
    pub customer_id: Option<String>,
    pub status: String,
    pub payment_method: String,
    #[schema(value_type = String)]
    pub amount: Decimal,
    pub currency: String,
    #[schema(value_type = String)]
    pub amount_refunded: Decimal,
    pub external_id: Option<String>,
    pub processor: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Response body for `GET /api/v1/payments` (list).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PaymentListResponse {
    pub payments: Vec<PaymentResponse>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
    pub has_more: bool,
}

// ============================================================================
// Invoices
// ============================================================================

/// Query parameters for `GET /api/v1/invoices` with filtering.
#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct InvoiceFilterParams {
    /// Maximum number of results to return (default: 50).
    pub limit: Option<u32>,
    /// Number of results to skip (default: 0).
    pub offset: Option<u32>,
    /// Filter by customer ID (UUID).
    pub customer_id: Option<String>,
    /// Filter by order ID (UUID).
    pub order_id: Option<String>,
    /// Filter by invoice status (draft, sent, viewed, paid, overdue, voided).
    pub status: Option<String>,
    /// Filter by invoice type (standard, `credit_note`, `pro_forma`, recurring).
    pub invoice_type: Option<String>,
    /// Filter overdue invoices only.
    pub overdue_only: Option<bool>,
    /// Invoices created on or after this date (RFC 3339).
    pub from_date: Option<String>,
    /// Invoices created on or before this date (RFC 3339).
    pub to_date: Option<String>,
}

impl InvoiceFilterParams {
    /// Resolved limit with bounds checking.
    #[must_use]
    pub fn resolved_limit(&self) -> u32 {
        resolve_limit(self.limit)
    }

    /// Resolved offset.
    #[must_use]
    pub fn resolved_offset(&self) -> u32 {
        self.offset.unwrap_or(0)
    }
}

/// Request body for `POST /api/v1/invoices`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateInvoiceRequest {
    #[schema(value_type = String, format = "uuid")]
    pub customer_id: stateset_primitives::CustomerId,
    #[schema(value_type = Option<String>, format = "uuid")]
    pub order_id: Option<stateset_primitives::OrderId>,
    pub invoice_type: Option<String>,
    pub due_date: Option<String>,
    pub payment_terms: Option<String>,
    pub currency: Option<String>,
    pub notes: Option<String>,
}

/// Request body for `POST /api/v1/invoices/:id/payments`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct RecordInvoicePaymentRequest {
    #[schema(value_type = String)]
    pub amount: Decimal,
    pub payment_method: Option<String>,
    pub reference: Option<String>,
    pub notes: Option<String>,
}

/// Response body for a single invoice.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InvoiceResponse {
    #[schema(value_type = String, format = "uuid")]
    pub id: stateset_primitives::InvoiceId,
    pub invoice_number: String,
    #[schema(value_type = String, format = "uuid")]
    pub customer_id: CustomerId,
    #[schema(value_type = Option<String>, format = "uuid")]
    pub order_id: Option<OrderId>,
    pub status: String,
    pub invoice_type: String,
    pub invoice_date: DateTime<Utc>,
    pub due_date: DateTime<Utc>,
    pub currency: String,
    #[schema(value_type = String)]
    pub subtotal: Decimal,
    #[schema(value_type = String)]
    pub tax_amount: Decimal,
    #[schema(value_type = String)]
    pub total: Decimal,
    #[schema(value_type = String)]
    pub amount_paid: Decimal,
    #[schema(value_type = String)]
    pub balance_due: Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Response body for `GET /api/v1/invoices` (list).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct InvoiceListResponse {
    pub invoices: Vec<InvoiceResponse>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
    pub has_more: bool,
}

// ============================================================================
// Returns
// ============================================================================

/// Request body for `POST /api/v1/returns`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateReturnRequest {
    #[schema(value_type = String, format = "uuid")]
    pub order_id: OrderId,
    pub reason: String,
    pub reason_details: Option<String>,
    pub items: Vec<CreateReturnItemRequest>,
    pub notes: Option<String>,
}

/// A single item in a create-return request.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateReturnItemRequest {
    pub order_item_id: Uuid,
    pub quantity: i32,
    pub condition: Option<String>,
}

/// Response body for a single return.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReturnResponse {
    #[schema(value_type = String, format = "uuid")]
    pub id: ReturnId,
    #[schema(value_type = String, format = "uuid")]
    pub order_id: OrderId,
    #[schema(value_type = String, format = "uuid")]
    pub customer_id: CustomerId,
    pub status: String,
    pub reason: String,
    #[schema(value_type = Option<String>)]
    pub refund_amount: Option<Decimal>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Response body for `GET /api/v1/returns` (list).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReturnListResponse {
    pub returns: Vec<ReturnResponse>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
    /// Opaque cursor for fetching the next page (keyset pagination).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Whether more results are available after this page.
    pub has_more: bool,
}

// ============================================================================
// Health
// ============================================================================

/// Tenant cache status included in health and readiness responses.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TenantCacheResponse {
    pub enabled: bool,
    pub max_cached_dbs: usize,
    pub cached_dbs: usize,
    pub in_use_cached_dbs: usize,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub rejections: u64,
}

/// Response body for `GET /health`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_cache: Option<TenantCacheResponse>,
}

/// Response body for `GET /health/ready`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReadyResponse {
    pub status: &'static str,
    pub database: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_cache: Option<TenantCacheResponse>,
}

/// Response body for `GET /version` — build & release metadata.
///
/// All fields except `version` are best-effort: they're set at compile
/// time from environment variables the release pipeline injects. When a
/// build runs without those variables set (e.g. local `cargo build`),
/// the corresponding fields are `None` and operators can interpret that
/// as "this binary did not come from a verified release pipeline".
///
/// The companion admin route (Phase 4.4) consumes this endpoint to
/// display sigstore-verified release information so operators can audit
/// what's actually running in production.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct VersionResponse {
    /// Package version from `Cargo.toml` (always present).
    pub version: &'static str,

    /// Git commit SHA (full or short) of the build, if injected via
    /// `GITHUB_SHA` at compile time. `None` for unverified local builds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_commit: Option<&'static str>,

    /// Git branch or tag name, if injected via `GITHUB_REF_NAME`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_ref: Option<&'static str>,

    /// Release tag (e.g. `v1.0.3`) if this binary came from a tagged
    /// release. Distinct from `git_ref` because release builds set this
    /// explicitly via `STATESET_RELEASE_TAG`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_tag: Option<&'static str>,

    /// RFC 3339 build timestamp, if injected via
    /// `STATESET_BUILD_TIMESTAMP` at compile time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub built_at: Option<&'static str>,

    /// Whether this binary's release artifacts were signed via sigstore.
    /// `true` when the release pipeline injected `STATESET_SIGNED=true`;
    /// `false` (the default) for local builds, dev builds, and any
    /// release where signing was skipped or failed.
    pub signed: bool,
}

// ============================================================================
// Events
// ============================================================================

/// Query parameters for the SSE event stream endpoint.
#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct EventStreamParams {
    /// Optional event type filter (e.g. `order.*`).
    pub filter: Option<String>,
    /// Resume the stream after this event id. Equivalent to the standard SSE
    /// `Last-Event-ID` request header; the header takes precedence when both
    /// are supplied. Events with a greater id are replayed from the bounded
    /// server-side buffer before the live stream resumes.
    pub last_event_id: Option<u64>,
}

// ============================================================================
// Conversion helpers
// ============================================================================

impl From<stateset_core::Order> for OrderResponse {
    fn from(o: stateset_core::Order) -> Self {
        Self {
            id: o.id,
            order_number: o.order_number,
            customer_id: o.customer_id,
            status: o.status.to_string(),
            total_amount: o.total_amount,
            currency: o.currency.to_string(),
            payment_status: o.payment_status.to_string(),
            fulfillment_status: o.fulfillment_status.to_string(),
            items: o.items.into_iter().map(OrderItemResponse::from).collect(),
            created_at: o.created_at,
            updated_at: o.updated_at,
        }
    }
}

impl From<stateset_core::OrderItem> for OrderItemResponse {
    fn from(i: stateset_core::OrderItem) -> Self {
        Self {
            id: *i.id.as_uuid(),
            product_id: i.product_id,
            sku: i.sku,
            name: i.name,
            quantity: i.quantity,
            unit_price: i.unit_price,
            total: i.total,
        }
    }
}

impl From<stateset_core::Customer> for CustomerResponse {
    fn from(c: stateset_core::Customer) -> Self {
        Self {
            id: c.id,
            email: c.email,
            first_name: c.first_name,
            last_name: c.last_name,
            phone: c.phone,
            status: c.status.to_string(),
            accepts_marketing: c.accepts_marketing,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

impl From<stateset_core::Product> for ProductResponse {
    fn from(p: stateset_core::Product) -> Self {
        Self {
            id: p.id,
            name: p.name,
            slug: p.slug,
            description: p.description,
            status: p.status.to_string(),
            product_type: p.product_type.to_string(),
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}

impl From<stateset_core::StockLevel> for InventoryResponse {
    fn from(s: stateset_core::StockLevel) -> Self {
        Self {
            sku: s.sku,
            name: s.name,
            total_on_hand: s.total_on_hand,
            total_allocated: s.total_allocated,
            total_available: s.total_available,
        }
    }
}

impl From<stateset_core::InventoryItem> for InventoryItemResponse {
    fn from(i: stateset_core::InventoryItem) -> Self {
        Self {
            id: i.id,
            sku: i.sku,
            name: i.name,
            description: i.description,
            unit_of_measure: i.unit_of_measure,
            is_active: i.is_active,
            created_at: i.created_at,
            updated_at: i.updated_at,
        }
    }
}

impl From<stateset_core::Shipment> for ShipmentResponse {
    fn from(s: stateset_core::Shipment) -> Self {
        Self {
            id: s.id,
            shipment_number: s.shipment_number,
            order_id: s.order_id,
            status: s.status.to_string(),
            carrier: s.carrier.to_string(),
            shipping_method: s.shipping_method.to_string(),
            tracking_number: s.tracking_number,
            tracking_url: s.tracking_url,
            recipient_name: s.recipient_name,
            shipping_cost: s.shipping_cost,
            shipped_at: s.shipped_at,
            estimated_delivery: s.estimated_delivery,
            delivered_at: s.delivered_at,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}

impl From<stateset_core::Payment> for PaymentResponse {
    fn from(p: stateset_core::Payment) -> Self {
        Self {
            id: p.id,
            payment_number: p.payment_number,
            order_id: p.order_id,
            customer_id: p.customer_id.map(|c| c.to_string()),
            status: p.status.to_string(),
            payment_method: p.payment_method.to_string(),
            amount: p.amount,
            currency: p.currency.to_string(),
            amount_refunded: p.amount_refunded,
            external_id: p.external_id,
            processor: p.processor,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}

impl From<stateset_core::Invoice> for InvoiceResponse {
    fn from(i: stateset_core::Invoice) -> Self {
        Self {
            id: i.id,
            invoice_number: i.invoice_number,
            customer_id: i.customer_id,
            order_id: i.order_id,
            status: i.status.to_string(),
            invoice_type: i.invoice_type.to_string(),
            invoice_date: i.invoice_date,
            due_date: i.due_date,
            currency: i.currency.to_string(),
            subtotal: i.subtotal,
            tax_amount: i.tax_amount,
            total: i.total,
            amount_paid: i.amount_paid,
            balance_due: i.balance_due,
            created_at: i.created_at,
            updated_at: i.updated_at,
        }
    }
}

impl From<stateset_core::Return> for ReturnResponse {
    fn from(r: stateset_core::Return) -> Self {
        Self {
            id: r.id,
            order_id: r.order_id,
            customer_id: r.customer_id,
            status: r.status.to_string(),
            reason: r.reason.to_string(),
            refund_amount: r.refund_amount,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

impl From<AddressDto> for stateset_core::Address {
    fn from(a: AddressDto) -> Self {
        Self {
            line1: a.line1,
            line2: a.line2,
            city: a.city,
            state: a.state,
            postal_code: a.postal_code,
            country: a.country,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    // ============================================================================
    // PaginationParams tests
    // ============================================================================

    #[test]
    fn pagination_default_limit() {
        let p = PaginationParams::default();
        assert_eq!(p.resolved_limit(), PaginationParams::DEFAULT_LIMIT);
        assert_eq!(p.resolved_offset(), 0);
    }

    #[test]
    fn pagination_custom_limit() {
        let p = PaginationParams { limit: Some(10), offset: Some(20) };
        assert_eq!(p.resolved_limit(), 10);
        assert_eq!(p.resolved_offset(), 20);
    }

    #[test]
    fn pagination_clamps_to_max() {
        let p = PaginationParams { limit: Some(999), offset: None };
        assert_eq!(p.resolved_limit(), PaginationParams::MAX_LIMIT);
    }

    #[test]
    fn pagination_clamps_zero_to_one() {
        let p = PaginationParams { limit: Some(0), offset: None };
        assert_eq!(p.resolved_limit(), 1);
    }

    // ============================================================================
    // Money DTO precision tests
    //
    // Monetary request fields must deserialize as exact decimals (string form),
    // matching PaymentResponse/InvoiceResponse serialization, while continuing
    // to accept plain JSON numbers for wire compatibility.
    // ============================================================================

    #[test]
    fn payment_amount_accepts_exact_decimal_string() {
        let req: CreatePaymentRequest = serde_json::from_str(
            r#"{"order_id":"01234567-89ab-cdef-0123-456789abcdef","amount":"123.456789012345678901"}"#,
        )
        .expect("string-encoded amount must deserialize exactly");
        assert_eq!(req.amount.to_string(), "123.456789012345678901");
    }

    #[test]
    fn refund_amount_accepts_exact_decimal_string() {
        let req: CreateRefundRequest = serde_json::from_str(r#"{"amount":"0.30"}"#)
            .expect("string-encoded amount must deserialize exactly");
        assert_eq!(req.amount.to_string(), "0.30");
    }

    #[test]
    fn refund_amount_still_accepts_json_number() {
        let req: CreateRefundRequest = serde_json::from_str(r#"{"amount":49.99}"#)
            .expect("plain JSON number amount must keep deserializing");
        assert_eq!(req.amount.to_string(), "49.99");
    }

    #[test]
    fn invoice_payment_amount_accepts_exact_decimal_string() {
        let req: RecordInvoicePaymentRequest =
            serde_json::from_str(r#"{"amount":"1000000000000.000001"}"#)
                .expect("string-encoded amount must deserialize exactly");
        assert_eq!(req.amount.to_string(), "1000000000000.000001");
    }

    // ============================================================================
    // DTO serialization tests
    // ============================================================================

    #[test]
    fn create_order_request_roundtrip() {
        let req = CreateOrderRequest {
            customer_id: CustomerId::new(),
            items: vec![CreateOrderItemRequest {
                product_id: ProductId::new(),
                variant_id: None,
                sku: "SKU-1".into(),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(29.99),
                discount: None,
                tax_amount: None,
            }],
            currency: Some("USD".into()),
            shipping_address: Some(AddressDto {
                line1: "123 Main".into(),
                line2: None,
                city: "NYC".into(),
                state: Some("NY".into()),
                postal_code: "10001".into(),
                country: "US".into(),
            }),
            billing_address: None,
            notes: None,
            payment_method: None,
            shipping_method: None,
            stock_policy: StockPolicy::AllowBackorder,
        };
        let json = serde_json::to_string(&req).unwrap();
        let deser: CreateOrderRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.items.len(), 1);
        assert_eq!(deser.items[0].sku, "SKU-1");
    }

    #[test]
    fn create_customer_request_roundtrip() {
        let req = CreateCustomerRequest {
            email: "test@example.com".into(),
            first_name: "John".into(),
            last_name: "Doe".into(),
            phone: None,
            accepts_marketing: Some(true),
            tags: None,
            metadata: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let deser: CreateCustomerRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.email, "test@example.com");
    }

    #[test]
    fn create_product_request_roundtrip() {
        let req = CreateProductRequest {
            name: "Widget".into(),
            slug: Some("widget".into()),
            description: Some("A fine widget".into()),
            product_type: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let deser: CreateProductRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.name, "Widget");
    }

    #[test]
    fn inventory_adjust_request_roundtrip() {
        let req = InventoryAdjustRequest {
            quantity: dec!(-5),
            reason: "Damaged".into(),
            location_id: Some(1),
        };
        let json = serde_json::to_string(&req).unwrap();
        let deser: InventoryAdjustRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.quantity, dec!(-5));
    }

    #[test]
    fn create_return_request_roundtrip() {
        let req = CreateReturnRequest {
            order_id: OrderId::new(),
            reason: "defective".into(),
            reason_details: None,
            items: vec![CreateReturnItemRequest {
                order_item_id: Uuid::new_v4(),
                quantity: 1,
                condition: Some("damaged".into()),
            }],
            notes: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let deser: CreateReturnRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.items.len(), 1);
    }

    #[test]
    fn health_response_serialization() {
        let resp = HealthResponse {
            status: "ok",
            tenant_cache: Some(TenantCacheResponse {
                enabled: true,
                max_cached_dbs: 256,
                cached_dbs: 3,
                in_use_cached_dbs: 1,
                hits: 20,
                misses: 4,
                evictions: 2,
                rejections: 1,
            }),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["tenant_cache"]["cached_dbs"], 3);
    }

    #[test]
    fn ready_response_serialization() {
        let resp = ReadyResponse {
            status: "ok",
            database: "connected",
            tenant_cache: Some(TenantCacheResponse {
                enabled: false,
                max_cached_dbs: 256,
                cached_dbs: 0,
                in_use_cached_dbs: 0,
                hits: 0,
                misses: 0,
                evictions: 0,
                rejections: 0,
            }),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["database"], "connected");
        assert_eq!(json["tenant_cache"]["enabled"], false);
    }

    #[test]
    fn order_response_serialization() {
        let resp = OrderResponse {
            id: OrderId::new(),
            order_number: "ORD-001".into(),
            customer_id: CustomerId::new(),
            status: "pending".into(),
            total_amount: dec!(59.98),
            currency: "USD".into(),
            payment_status: "pending".into(),
            fulfillment_status: "unfulfilled".into(),
            items: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["status"], "pending");
    }

    #[test]
    fn customer_response_serialization() {
        let resp = CustomerResponse {
            id: CustomerId::new(),
            email: "a@b.com".into(),
            first_name: "A".into(),
            last_name: "B".into(),
            phone: None,
            status: "active".into(),
            accepts_marketing: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["email"], "a@b.com");
    }

    #[test]
    fn product_response_serialization() {
        let resp = ProductResponse {
            id: ProductId::new(),
            name: "W".into(),
            slug: "w".into(),
            description: "d".into(),
            status: "draft".into(),
            product_type: "simple".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["name"], "W");
    }

    #[test]
    fn inventory_response_serialization() {
        let resp = InventoryResponse {
            sku: "SKU-1".into(),
            name: "Widget".into(),
            total_on_hand: dec!(100),
            total_allocated: dec!(10),
            total_available: dec!(90),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["sku"], "SKU-1");
    }

    #[test]
    fn return_response_serialization() {
        let resp = ReturnResponse {
            id: ReturnId::new(),
            order_id: OrderId::new(),
            customer_id: CustomerId::new(),
            status: "requested".into(),
            reason: "defective".into(),
            refund_amount: Some(dec!(29.99)),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["status"], "requested");
    }

    #[test]
    fn address_dto_converts_to_core() {
        let dto = AddressDto {
            line1: "123 Main".into(),
            line2: None,
            city: "NYC".into(),
            state: Some("NY".into()),
            postal_code: "10001".into(),
            country: "US".into(),
        };
        let core: stateset_core::Address = dto.into();
        assert_eq!(core.city, "NYC");
    }

    #[test]
    fn event_stream_params_default() {
        let p = EventStreamParams::default();
        assert!(p.filter.is_none());
    }

    #[test]
    fn order_list_response_serialization() {
        let resp = OrderListResponse {
            orders: vec![],
            total: 0,
            limit: 50,
            offset: 0,
            next_cursor: None,
            has_more: false,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["total"], 0);
        assert_eq!(json["has_more"], false);
        assert!(json.get("next_cursor").is_none()); // skipped when None
    }

    #[test]
    fn customer_list_response_serialization() {
        let resp = CustomerListResponse {
            customers: vec![],
            total: 0,
            limit: 50,
            offset: 0,
            next_cursor: None,
            has_more: false,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["total"], 0);
        assert_eq!(json["has_more"], false);
    }

    #[test]
    fn product_list_response_serialization() {
        let resp = ProductListResponse {
            products: vec![],
            total: 0,
            limit: 50,
            offset: 0,
            next_cursor: None,
            has_more: false,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["total"], 0);
        assert_eq!(json["has_more"], false);
    }

    #[test]
    fn return_list_response_serialization() {
        let resp = ReturnListResponse {
            returns: vec![],
            total: 0,
            limit: 50,
            offset: 0,
            next_cursor: None,
            has_more: false,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["total"], 0);
        assert_eq!(json["has_more"], false);
    }

    // ============================================================================
    // Cursor helpers tests
    // ============================================================================

    #[test]
    fn cursor_encode_decode_roundtrip() {
        let cursor = encode_cursor("2024-01-15T10:30:00Z", "550e8400-e29b-41d4-a716-446655440000");
        let (sort_key, id) = decode_cursor(&cursor).unwrap();
        assert_eq!(sort_key, "2024-01-15T10:30:00Z");
        assert_eq!(id, "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn cursor_decode_invalid_base64() {
        assert!(decode_cursor("not-valid-base64!!!").is_none());
    }

    #[test]
    fn cursor_decode_missing_separator() {
        let encoded = URL_SAFE_NO_PAD.encode(b"no-separator-here");
        assert!(decode_cursor(&encoded).is_none());
    }

    #[test]
    fn overfetch_limit_adds_one_row() {
        assert_eq!(overfetch_limit(10), 11);
        assert_eq!(overfetch_limit(PaginationParams::MAX_LIMIT), PaginationParams::MAX_LIMIT + 1);
    }

    #[test]
    fn finalize_page_marks_exact_boundary_as_not_has_more() {
        let mut items = vec![1, 2];
        let has_more = finalize_page(&mut items, 2);
        assert!(!has_more);
        assert_eq!(items, vec![1, 2]);
    }

    #[test]
    fn finalize_page_trims_overfetch_and_sets_has_more() {
        let mut items = vec![1, 2, 3];
        let has_more = finalize_page(&mut items, 2);
        assert!(has_more);
        assert_eq!(items, vec![1, 2]);
    }

    #[test]
    fn cursor_next_cursor_serialized_when_present() {
        let resp = OrderListResponse {
            orders: vec![],
            total: 100,
            limit: 10,
            offset: 0,
            next_cursor: Some("abc123".into()),
            has_more: true,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["next_cursor"], "abc123");
        assert_eq!(json["has_more"], true);
    }

    // ============================================================================
    // Filter params deserialization tests
    // ============================================================================

    #[test]
    fn order_filter_params_default() {
        let p = OrderFilterParams::default();
        assert_eq!(p.resolved_limit(), PaginationParams::DEFAULT_LIMIT);
        assert_eq!(p.resolved_offset(), 0);
        assert!(p.customer_id.is_none());
        assert!(p.status.is_none());
    }

    #[test]
    fn order_filter_params_deserialization() {
        let p: OrderFilterParams = serde_json::from_value(serde_json::json!({
            "limit": 10, "offset": 5, "status": "pending", "customer_id": "abc"
        }))
        .unwrap();
        assert_eq!(p.resolved_limit(), 10);
        assert_eq!(p.resolved_offset(), 5);
        assert_eq!(p.status.as_deref(), Some("pending"));
        assert_eq!(p.customer_id.as_deref(), Some("abc"));
    }

    #[test]
    fn customer_filter_params_deserialization() {
        let p: CustomerFilterParams = serde_json::from_value(serde_json::json!({
            "email": "test@example.com", "accepts_marketing": true
        }))
        .unwrap();
        assert_eq!(p.email.as_deref(), Some("test@example.com"));
        assert_eq!(p.accepts_marketing, Some(true));
    }

    #[test]
    fn product_filter_params_deserialization() {
        let p: ProductFilterParams = serde_json::from_value(serde_json::json!({
            "search": "widget", "min_price": "10.00", "max_price": "100", "in_stock": true
        }))
        .unwrap();
        assert_eq!(p.search.as_deref(), Some("widget"));
        assert_eq!(p.min_price.as_deref(), Some("10.00"));
        assert_eq!(p.max_price.as_deref(), Some("100"));
        assert_eq!(p.in_stock, Some(true));
    }

    #[test]
    fn return_filter_params_deserialization() {
        let p: ReturnFilterParams = serde_json::from_value(serde_json::json!({
            "status": "requested", "reason": "defective", "limit": 5
        }))
        .unwrap();
        assert_eq!(p.status.as_deref(), Some("requested"));
        assert_eq!(p.reason.as_deref(), Some("defective"));
        assert_eq!(p.resolved_limit(), 5);
    }

    #[test]
    fn filter_params_limit_clamps_to_max() {
        let p = OrderFilterParams { limit: Some(999), ..Default::default() };
        assert_eq!(p.resolved_limit(), PaginationParams::MAX_LIMIT);
    }

    #[test]
    fn filter_params_limit_clamps_zero_to_one() {
        let p = OrderFilterParams { limit: Some(0), ..Default::default() };
        assert_eq!(p.resolved_limit(), 1);
    }
}
