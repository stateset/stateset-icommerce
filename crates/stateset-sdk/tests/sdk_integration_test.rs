//! Integration tests for the stateset-sdk unified facade crate.
//!
//! These tests validate that the SDK properly re-exports all sub-crates,
//! that the prelude gives access to commonly-needed types, and that end-to-end
//! workflows function through the SDK surface.

use rust_decimal::Decimal;
use stateset_sdk::prelude::*;

// ---------------------------------------------------------------------------
// Prelude type accessibility
// ---------------------------------------------------------------------------

#[test]
fn prelude_money_zero() {
    let m = Money::zero(CurrencyCode::USD);
    assert_eq!(m.amount(), Decimal::ZERO);
    assert_eq!(m.currency(), CurrencyCode::USD);
}

#[test]
fn prelude_money_new() {
    let m = Money::new(Decimal::new(1999, 2), CurrencyCode::EUR);
    assert_eq!(m.amount(), Decimal::new(1999, 2));
    assert_eq!(m.currency(), CurrencyCode::EUR);
}

#[test]
fn prelude_typed_ids_are_unique() {
    let a = OrderId::new();
    let b = OrderId::new();
    assert_ne!(a, b);
}

#[test]
fn prelude_customer_id_roundtrip() {
    let id = CustomerId::new();
    let s = id.to_string();
    assert!(!s.is_empty());
}

#[test]
fn prelude_product_id_roundtrip() {
    let id = ProductId::new();
    assert!(!id.to_string().is_empty());
}

#[test]
fn prelude_return_id_roundtrip() {
    let id = ReturnId::new();
    assert!(!id.to_string().is_empty());
}

#[test]
fn prelude_payment_id_roundtrip() {
    let id = PaymentId::new();
    assert!(!id.to_string().is_empty());
}

#[test]
fn prelude_shipment_id_roundtrip() {
    let id = ShipmentId::new();
    assert!(!id.to_string().is_empty());
}

#[test]
fn prelude_fulfillment_id_roundtrip() {
    let id = FulfillmentId::new();
    assert!(!id.to_string().is_empty());
}

#[test]
fn prelude_order_item_id_roundtrip() {
    let id = OrderItemId::new();
    assert!(!id.to_string().is_empty());
}

// ---------------------------------------------------------------------------
// Currency codes
// ---------------------------------------------------------------------------

#[test]
fn currency_code_variants() {
    let _ = CurrencyCode::USD;
    let _ = CurrencyCode::EUR;
    let _ = CurrencyCode::GBP;
    let _ = CurrencyCode::JPY;
    let _ = CurrencyCode::CAD;
}

#[test]
fn sku_creation() {
    let sku = Sku::new("TEST-SKU-001").expect("valid sku");
    assert_eq!(sku.as_str(), "TEST-SKU-001");
}

#[test]
fn sku_display() {
    let sku = Sku::new("WIDGET-42").expect("valid sku");
    assert_eq!(format!("{sku}"), "WIDGET-42");
}

// ---------------------------------------------------------------------------
// Enum variants accessible through prelude
// ---------------------------------------------------------------------------

#[test]
fn order_status_variants() {
    let _ = OrderStatus::Pending;
    let _ = OrderStatus::Confirmed;
    let _ = OrderStatus::Cancelled;
}

#[test]
fn customer_status_variants() {
    let _ = CustomerStatus::Active;
    let _ = CustomerStatus::Inactive;
}

#[test]
fn payment_status_variants() {
    let _ = PaymentStatus::Pending;
    let _ = PaymentStatus::Paid;
    let _ = PaymentStatus::Failed;
}

#[test]
fn product_status_variants() {
    let _ = ProductStatus::Active;
    let _ = ProductStatus::Draft;
    let _ = ProductStatus::Archived;
}

#[test]
fn return_status_variants() {
    let _ = ReturnStatus::Requested;
    let _ = ReturnStatus::Approved;
    let _ = ReturnStatus::Rejected;
}

// ---------------------------------------------------------------------------
// CommerceError re-export
// ---------------------------------------------------------------------------

#[test]
fn commerce_error_not_found() {
    let err = CommerceError::NotFound;
    let display = format!("{err}");
    assert!(!display.is_empty());
}

#[test]
fn commerce_error_internal() {
    let err = CommerceError::Internal("test error".into());
    let display = format!("{err}");
    assert!(display.contains("test"));
}

#[test]
fn commerce_error_is_not_found() {
    assert!(CommerceError::NotFound.is_not_found());
    assert!(!CommerceError::Internal("x".into()).is_not_found());
}

// ---------------------------------------------------------------------------
// Commerce events
// ---------------------------------------------------------------------------

#[test]
fn commerce_event_order_created() {
    let event = CommerceEvent::OrderCreated {
        order_id: OrderId::new(),
        customer_id: CustomerId::new(),
        total_amount: Decimal::new(9999, 2),
        item_count: 3,
        timestamp: chrono::Utc::now(),
    };
    assert_eq!(event.event_type(), "order_created");
}

#[test]
fn commerce_event_customer_created() {
    let event = CommerceEvent::CustomerCreated {
        customer_id: CustomerId::new(),
        email: "test@example.com".into(),
        timestamp: chrono::Utc::now(),
    };
    assert_eq!(event.event_type(), "customer_created");
}

#[test]
fn commerce_event_serialization_roundtrip() {
    let event = CommerceEvent::OrderCreated {
        order_id: OrderId::new(),
        customer_id: CustomerId::new(),
        total_amount: Decimal::new(4200, 2),
        item_count: 1,
        timestamp: chrono::Utc::now(),
    };
    let json = event.to_json().expect("serialize");
    let deserialized = CommerceEvent::from_json(&json).expect("deserialize");
    assert_eq!(deserialized.event_type(), "order_created");
}

#[test]
fn commerce_event_timestamp() {
    let now = chrono::Utc::now();
    let event = CommerceEvent::CustomerCreated {
        customer_id: CustomerId::new(),
        email: "ts@example.com".into(),
        timestamp: now,
    };
    assert_eq!(event.timestamp(), now);
}

// ---------------------------------------------------------------------------
// Create structs available through prelude
// ---------------------------------------------------------------------------

#[test]
fn create_customer_default() {
    let input = CreateCustomer {
        email: "alice@example.com".into(),
        first_name: "Alice".into(),
        last_name: "Smith".into(),
        ..Default::default()
    };
    assert_eq!(input.email, "alice@example.com");
    assert_eq!(input.first_name, "Alice");
}

#[test]
fn create_product_default() {
    let input = CreateProduct { name: "Widget".into(), ..Default::default() };
    assert_eq!(input.name, "Widget");
}

#[test]
fn create_order_with_items() {
    let input = CreateOrder {
        customer_id: CustomerId::new(),
        items: vec![CreateOrderItem {
            sku: "SKU-001".into(),
            name: "Widget".into(),
            quantity: 2,
            unit_price: Decimal::new(2999, 2),
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_eq!(input.items.len(), 1);
    assert_eq!(input.items[0].quantity, 2);
}

#[test]
fn create_return_default() {
    let input = CreateReturn { order_id: OrderId::new(), ..Default::default() };
    assert!(!input.order_id.to_string().is_empty());
}

// ---------------------------------------------------------------------------
// Filter structs
// ---------------------------------------------------------------------------

#[test]
fn customer_filter_default_is_unfiltered() {
    let filter = CustomerFilter::default();
    assert!(filter.status.is_none());
}

#[test]
fn order_filter_default_is_unfiltered() {
    let filter = OrderFilter::default();
    assert!(filter.status.is_none());
}

#[test]
fn product_filter_default_is_unfiltered() {
    let filter = ProductFilter::default();
    assert!(filter.status.is_none());
}

#[test]
fn payment_filter_default_is_unfiltered() {
    let filter = PaymentFilter::default();
    assert!(filter.status.is_none());
}

#[test]
fn return_filter_default_is_unfiltered() {
    let filter = ReturnFilter::default();
    assert!(filter.status.is_none());
}

// ---------------------------------------------------------------------------
// Re-export accessibility (non-prelude)
// ---------------------------------------------------------------------------

#[test]
fn core_reexport() {
    let _ = stateset_sdk::core::CommerceError::NotFound;
}

#[test]
fn primitives_reexport() {
    let _id = stateset_sdk::primitives::OrderId::new();
    let _money = stateset_sdk::primitives::Money::zero(stateset_sdk::primitives::CurrencyCode::USD);
}

#[test]
fn db_reexport_config() {
    let _cfg = stateset_sdk::db::DatabaseConfig::in_memory();
}

#[test]
fn embedded_reexport_type() {
    let _ = std::any::TypeId::of::<stateset_sdk::embedded::Commerce>();
}

#[test]
fn observability_reexport() {
    let _cfg = stateset_sdk::observability::MetricsConfig::default();
}

// ---------------------------------------------------------------------------
// Database config accessible through SDK
// ---------------------------------------------------------------------------

#[test]
fn database_config_in_memory() {
    let cfg = stateset_sdk::db::DatabaseConfig::in_memory();
    let debug = format!("{cfg:?}");
    assert!(debug.contains("memory"));
}

#[test]
fn metrics_config_default() {
    let cfg = MetricsConfig::default();
    let debug = format!("{cfg:?}");
    assert!(!debug.is_empty());
}

// ---------------------------------------------------------------------------
// Commerce instance creation through SDK
// ---------------------------------------------------------------------------

#[test]
fn commerce_new_in_memory() {
    let commerce = Commerce::new(":memory:").expect("in-memory commerce");
    let customers = commerce.customers().list(CustomerFilter::default()).expect("list");
    assert!(customers.is_empty());
}

#[test]
fn commerce_builder_in_memory() {
    let commerce = stateset_sdk::embedded::CommerceBuilder::new()
        .in_memory()
        .build()
        .expect("builder in-memory");
    let products = commerce.products().list(ProductFilter::default()).expect("list");
    assert!(products.is_empty());
}

#[test]
fn commerce_create_and_get_customer() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: "bob@example.com".into(),
            first_name: "Bob".into(),
            last_name: "Jones".into(),
            ..Default::default()
        })
        .expect("create customer");
    assert_eq!(customer.email, "bob@example.com");

    let fetched = commerce.customers().get(customer.id).expect("get").expect("found");
    assert_eq!(fetched.email, "bob@example.com");
}

#[test]
fn commerce_create_product_and_list() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    commerce
        .products()
        .create(CreateProduct { name: "Widget".into(), ..Default::default() })
        .expect("create product");

    let products = commerce.products().list(ProductFilter::default()).expect("list");
    assert_eq!(products.len(), 1);
    assert_eq!(products[0].name, "Widget");
}

#[test]
fn commerce_full_order_lifecycle() {
    let commerce = Commerce::new(":memory:").expect("commerce");

    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: "lifecycle@test.com".into(),
            first_name: "Life".into(),
            last_name: "Cycle".into(),
            ..Default::default()
        })
        .expect("create customer");

    let product = commerce
        .products()
        .create(CreateProduct { name: "Lifecycle Widget".into(), ..Default::default() })
        .expect("create product");

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: product.id,
                sku: "LC-001".into(),
                name: "Lifecycle Widget".into(),
                quantity: 1,
                unit_price: Decimal::new(4999, 2),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create order");

    assert_eq!(order.status, OrderStatus::Pending);

    let fetched = commerce.orders().get(order.id).expect("get").expect("found");
    assert_eq!(fetched.id, order.id);
}

#[test]
fn commerce_get_nonexistent_customer_returns_none() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let result = commerce.customers().get(CustomerId::new()).expect("get");
    assert!(result.is_none());
}

#[test]
fn commerce_list_empty_orders() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let orders = commerce.orders().list(OrderFilter::default()).expect("list");
    assert!(orders.is_empty());
}

#[test]
fn commerce_create_multiple_customers() {
    let commerce = Commerce::new(":memory:").expect("commerce");

    for i in 0..5 {
        commerce
            .customers()
            .create(CreateCustomer {
                email: format!("user{i}@example.com"),
                first_name: format!("User{i}"),
                last_name: "Test".into(),
                ..Default::default()
            })
            .expect("create customer");
    }

    let all = commerce.customers().list(CustomerFilter::default()).expect("list");
    assert_eq!(all.len(), 5);
}

#[test]
fn commerce_inventory_create_and_get() {
    let commerce = Commerce::new(":memory:").expect("commerce");

    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "INV-001".into(),
            name: "Test Item".into(),
            initial_quantity: Some(Decimal::new(100, 0)),
            ..Default::default()
        })
        .expect("create inventory item");

    let item = commerce.inventory().get_item_by_sku("INV-001").expect("get").expect("found");
    assert_eq!(item.sku, "INV-001");
}

// ---------------------------------------------------------------------------
// Address struct
// ---------------------------------------------------------------------------

#[test]
fn address_construction() {
    let addr = Address {
        line1: "123 Main St".into(),
        line2: None,
        city: "Springfield".into(),
        state: Some("IL".into()),
        postal_code: "62704".into(),
        country: "US".into(),
    };
    assert_eq!(addr.line1, "123 Main St");
    assert_eq!(addr.country, "US");
}

// ---------------------------------------------------------------------------
// Commerce health / introspection through SDK
// ---------------------------------------------------------------------------

#[test]
fn commerce_health_check() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let health = commerce.health_check();
    let debug = format!("{health:?}");
    assert!(!debug.is_empty());
}

#[test]
fn commerce_backend_is_sqlite() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    assert_eq!(commerce.backend(), stateset_sdk::embedded::CommerceBackend::Sqlite);
}
