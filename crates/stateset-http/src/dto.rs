//! Data Transfer Objects for HTTP request/response bodies.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CustomerId, OrderId, ProductId, ReturnId};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// ============================================================================
// Pagination
// ============================================================================

/// Query parameters for paginated list endpoints.
#[derive(Debug, Clone, Deserialize, Serialize, Default, IntoParams)]
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
        self.limit.unwrap_or(Self::DEFAULT_LIMIT).min(Self::MAX_LIMIT)
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

/// Response body for `GET /api/v1/customers` (list).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CustomerListResponse {
    pub customers: Vec<CustomerResponse>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
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

/// Response body for `GET /api/v1/products` (list).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProductListResponse {
    pub products: Vec<ProductResponse>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
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

// ============================================================================
// Health
// ============================================================================

/// Response body for `GET /health`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: &'static str,
}

/// Response body for `GET /health/ready`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReadyResponse {
    pub status: &'static str,
    pub database: &'static str,
}

// ============================================================================
// Events
// ============================================================================

/// Query parameters for the SSE event stream endpoint.
#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
pub struct EventStreamParams {
    /// Optional event type filter (e.g. `order.*`).
    pub filter: Option<String>,
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
            currency: o.currency,
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
        let resp = HealthResponse { status: "ok" };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["status"], "ok");
    }

    #[test]
    fn ready_response_serialization() {
        let resp = ReadyResponse { status: "ok", database: "connected" };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["database"], "connected");
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
        let resp = OrderListResponse { orders: vec![], total: 0, limit: 50, offset: 0 };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["total"], 0);
    }

    #[test]
    fn customer_list_response_serialization() {
        let resp = CustomerListResponse { customers: vec![], total: 0, limit: 50, offset: 0 };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["total"], 0);
    }

    #[test]
    fn product_list_response_serialization() {
        let resp = ProductListResponse { products: vec![], total: 0, limit: 50, offset: 0 };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["total"], 0);
    }
}
