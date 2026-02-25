#[cfg(feature = "postgres")]
use rust_decimal_macros::dec;
#[cfg(feature = "postgres")]
use stateset_core::{
    CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem, CreateProduct, OrderStatus,
    ReservationStatus,
};
#[cfg(feature = "postgres")]
use stateset_embedded::AsyncCommerce;
#[cfg(feature = "postgres")]
use std::env;
#[cfg(feature = "postgres")]
use uuid::Uuid;

#[cfg(feature = "postgres")]
fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_async_commerce_smoke() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres async smoke test");
            return;
        }
    };

    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");

    let unique = Uuid::new_v4().to_string();
    let sku = format!("SKU-{}", unique.replace('-', ""));

    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("test-{}@example.com", unique),
            first_name: "Test".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");

    let product = commerce
        .products()
        .create(CreateProduct {
            name: format!("Widget {}", unique),
            slug: Some(format!("widget-{}", unique)),
            description: Some("Test product".into()),
            ..Default::default()
        })
        .await
        .expect("create product");

    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: "Widget".into(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .await
        .expect("create inventory item");

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: product.id,
                variant_id: None,
                sku: sku.clone(),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(9.99),
                discount: None,
                tax_amount: None,
            }],
            ..Default::default()
        })
        .await
        .expect("create order");
    assert_eq!(order.items.len(), 1);

    let reservations = commerce
        .inventory()
        .list_reservations_by_reference("order", &order.id.to_string())
        .await
        .expect("list reservations for order");
    assert!(!reservations.is_empty(), "expected at least one reservation for order");
    assert!(
        reservations.iter().all(|r| r.status == ReservationStatus::Pending),
        "expected reservations to be pending after order create"
    );

    let shipped =
        commerce.orders().ship(order.id.into_uuid(), Some("TRACK-TEST")).await.expect("ship order");
    assert_eq!(shipped.status, OrderStatus::Shipped);

    let reservations = commerce
        .inventory()
        .list_reservations_by_reference("order", &order.id.to_string())
        .await
        .expect("list reservations for order after ship");
    assert!(
        reservations.iter().all(|r| r.status == ReservationStatus::Confirmed),
        "expected reservations to be confirmed after shipping"
    );
}
