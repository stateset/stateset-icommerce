#[cfg(feature = "postgres")]
use rust_decimal_macros::dec;
#[cfg(feature = "postgres")]
use stateset_core::{
    AddCartItem, CartAddress, CartStatus, CreateCart, CreateCustomer, CreateInventoryItem,
    CreateOrder, CreateOrderItem, CreateProduct, OrderStatus, PaymentStatus, ReservationStatus,
    SetCartPayment,
};
#[cfg(feature = "postgres")]
use stateset_db::PostgresDatabase;
#[cfg(feature = "postgres")]
use stateset_embedded::AsyncCommerce;
#[cfg(feature = "postgres")]
use std::env;
#[cfg(feature = "postgres")]
use std::sync::Arc;
#[cfg(feature = "postgres")]
use std::time::Duration;
#[cfg(feature = "postgres")]
use uuid::Uuid;

#[cfg(feature = "postgres")]
fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

#[cfg(feature = "postgres")]
fn test_address() -> CartAddress {
    CartAddress {
        first_name: "Test".into(),
        last_name: "User".into(),
        company: None,
        line1: "123 Main St".into(),
        line2: None,
        city: "San Francisco".into(),
        state: Some("CA".into()),
        postal_code: "94102".into(),
        country: "US".into(),
        phone: Some("555-1234".into()),
        email: Some("test@example.com".into()),
    }
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_cart_checkout_creates_order() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres cart checkout test");
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
            first_name: "Cart".into(),
            last_name: "Checkout".into(),
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

    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_id: Some(customer.id),
            customer_email: Some(customer.email.clone()),
            customer_name: Some(format!("{} {}", customer.first_name, customer.last_name)),
            ..Default::default()
        })
        .await
        .expect("create cart");

    commerce
        .carts()
        .add_item(
            cart.id.into_uuid(),
            AddCartItem {
                product_id: Some(product.id),
                sku: sku.clone(),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(9.99),
                ..Default::default()
            },
        )
        .await
        .expect("add cart item");

    commerce
        .carts()
        .set_shipping_address(cart.id.into_uuid(), test_address())
        .await
        .expect("set shipping address");

    commerce
        .carts()
        .set_payment(
            cart.id.into_uuid(),
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_test".into()),
                billing_address: None,
            },
        )
        .await
        .expect("set payment");

    let result = commerce.carts().complete(cart.id.into_uuid()).await.expect("complete checkout");

    let second = commerce
        .carts()
        .complete(cart.id.into_uuid())
        .await
        .expect("checkout should be idempotent");
    assert_eq!(second.order_id, result.order_id);
    assert_eq!(second.order_number, result.order_number);

    assert!(!result.order_id.is_nil());
    assert!(!result.order_number.is_empty());
    assert!(result.order_number.starts_with("ORD-"));
    assert!(result.total_charged > dec!(0));

    let updated_cart =
        commerce.carts().get(cart.id.into_uuid()).await.expect("get cart").expect("cart row");
    assert_eq!(updated_cart.status, CartStatus::Completed);
    assert_eq!(updated_cart.order_id, Some(result.order_id));
    assert_eq!(updated_cart.order_number.as_deref(), Some(result.order_number.as_str()));

    let order = commerce
        .orders()
        .get(result.order_id.into_uuid())
        .await
        .expect("get order")
        .expect("order row");
    assert_eq!(order.customer_id, customer.id);
    assert_eq!(order.status, OrderStatus::Confirmed);
    assert_eq!(order.payment_status, PaymentStatus::Pending);
    assert_eq!(order.items.len(), 1);

    let reservations = commerce
        .inventory()
        .list_reservations_by_reference("order", &result.order_id.to_string())
        .await
        .expect("list reservations for order");
    assert!(!reservations.is_empty());
    assert!(reservations.iter().all(|r| r.status == ReservationStatus::Pending));
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_cart_checkout_retry_completes_existing_order() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres cart retry test");
            return;
        }
    };

    let db = Arc::new(
        PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"),
    );
    let commerce = AsyncCommerce::from_database(db.clone());

    let unique = Uuid::new_v4().to_string();
    let sku = format!("SKU-{}", unique.replace('-', ""));

    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("test-{}@example.com", unique),
            first_name: "Cart".into(),
            last_name: "Retry".into(),
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

    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_id: Some(customer.id),
            customer_email: Some(customer.email.clone()),
            customer_name: Some(format!("{} {}", customer.first_name, customer.last_name)),
            ..Default::default()
        })
        .await
        .expect("create cart");

    commerce
        .carts()
        .add_item(
            cart.id.into_uuid(),
            AddCartItem {
                product_id: Some(product.id),
                sku: sku.clone(),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(9.99),
                ..Default::default()
            },
        )
        .await
        .expect("add cart item");

    commerce
        .carts()
        .set_shipping_address(cart.id.into_uuid(), test_address())
        .await
        .expect("set shipping address");

    commerce
        .carts()
        .set_payment(
            cart.id.into_uuid(),
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_test".into()),
                billing_address: None,
            },
        )
        .await
        .expect("set payment");

    // Simulate a partial failure: order created but cart not updated.
    let order = db
        .orders()
        .create_from_cart_async(
            cart.id.into_uuid(),
            CreateOrder {
                customer_id: customer.id,
                items: vec![CreateOrderItem {
                    product_id: product.id,
                    variant_id: None,
                    sku: sku.clone(),
                    name: "Widget".into(),
                    quantity: 2,
                    unit_price: dec!(9.99),
                    ..Default::default()
                }],
                currency: Some(cart.currency),
                shipping_address: Some(test_address().into()),
                billing_address: None,
                notes: None,
                payment_method: Some("credit_card".into()),
                shipping_method: None,
                stock_policy: Default::default(),
            },
        )
        .await
        .expect("create order for cart");

    let result = commerce
        .carts()
        .complete(cart.id.into_uuid())
        .await
        .expect("checkout should complete the cart and reuse the existing order");
    assert_eq!(result.order_id, order.id);
    assert_eq!(result.order_number, order.order_number);

    let updated_cart =
        commerce.carts().get(cart.id.into_uuid()).await.expect("get cart").expect("cart row");
    assert_eq!(updated_cart.status, CartStatus::Completed);
    assert_eq!(updated_cart.order_id, Some(order.id));

    let updated_order =
        commerce.orders().get(order.id.into_uuid()).await.expect("get order").expect("order row");
    assert_eq!(updated_order.status, OrderStatus::Confirmed);
    assert_eq!(updated_order.payment_status, PaymentStatus::Pending);
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_complete_settled_externally_marks_order_paid() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping settled-externally test");
            return;
        }
    };

    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");

    let unique = Uuid::new_v4().to_string();
    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_email: Some(format!("settled-{}@example.com", unique)),
            customer_name: Some("Settled Externally".into()),
            ..Default::default()
        })
        .await
        .expect("create cart");

    commerce
        .carts()
        .add_item(
            cart.id.into_uuid(),
            AddCartItem {
                sku: format!("SKU-{}", unique.replace('-', "")),
                name: "Widget".into(),
                quantity: 1,
                unit_price: dec!(9.99),
                ..Default::default()
            },
        )
        .await
        .expect("add cart item");

    commerce
        .carts()
        .set_shipping_address(cart.id.into_uuid(), test_address())
        .await
        .expect("set shipping address");

    let result = commerce
        .carts()
        .complete_settled_externally(cart.id.into_uuid())
        .await
        .expect("settled-externally checkout should succeed");

    let order = commerce
        .orders()
        .get(result.order_id.into_uuid())
        .await
        .expect("get order")
        .expect("order row");
    assert_eq!(order.status, OrderStatus::Confirmed);
    assert_eq!(order.payment_status, PaymentStatus::Paid);

    let updated_cart =
        commerce.carts().get(cart.id.into_uuid()).await.expect("get cart").expect("cart row");
    assert_eq!(updated_cart.status, CartStatus::Completed);
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_cart_checkout_concurrent_complete_is_safe() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!(
                "POSTGRES_URL or DATABASE_URL not set; skipping postgres cart concurrency test"
            );
            return;
        }
    };

    let db = Arc::new(
        PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"),
    );
    let commerce = AsyncCommerce::from_database(db.clone());

    let unique = Uuid::new_v4().to_string();
    let sku = format!("SKU-{}", unique.replace('-', ""));

    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("test-{}@example.com", unique),
            first_name: "Cart".into(),
            last_name: "Concurrent".into(),
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

    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_id: Some(customer.id),
            customer_email: Some(customer.email.clone()),
            customer_name: Some(format!("{} {}", customer.first_name, customer.last_name)),
            ..Default::default()
        })
        .await
        .expect("create cart");

    commerce
        .carts()
        .add_item(
            cart.id.into_uuid(),
            AddCartItem {
                product_id: Some(product.id),
                sku: sku.clone(),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(9.99),
                ..Default::default()
            },
        )
        .await
        .expect("add cart item");

    commerce
        .carts()
        .set_shipping_address(cart.id.into_uuid(), test_address())
        .await
        .expect("set shipping address");

    commerce
        .carts()
        .set_payment(
            cart.id.into_uuid(),
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_test".into()),
                billing_address: None,
            },
        )
        .await
        .expect("set payment");

    let carts1 = commerce.carts();
    let carts2 = commerce.carts();
    let (r1, r2) = tokio::join!(
        tokio::time::timeout(Duration::from_secs(10), carts1.complete(cart.id.into_uuid())),
        tokio::time::timeout(Duration::from_secs(10), carts2.complete(cart.id.into_uuid())),
    );

    let r1 = r1.expect("timeout waiting for first checkout").expect("first checkout succeeds");
    let r2 = r2.expect("timeout waiting for second checkout").expect("second checkout succeeds");

    assert_eq!(r1.order_id, r2.order_id);
    assert_eq!(r1.order_number, r2.order_number);

    let order = db
        .orders()
        .get_by_cart_id_async(cart.id.into_uuid())
        .await
        .expect("get order by cart_id")
        .expect("order should exist for cart_id");
    assert_eq!(order.id, r1.order_id);

    let updated_cart =
        commerce.carts().get(cart.id.into_uuid()).await.expect("get cart").expect("cart row");
    assert_eq!(updated_cart.status, CartStatus::Completed);
    assert_eq!(updated_cart.order_id, Some(order.id));
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_guest_cart_checkout_creates_customer() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!(
                "POSTGRES_URL or DATABASE_URL not set; skipping postgres guest checkout test"
            );
            return;
        }
    };

    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");

    let unique = Uuid::new_v4().to_string();
    let sku = format!("SKU-{}", unique.replace('-', ""));
    let email = format!("guest-{}@example.com", unique);

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

    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_id: None,
            customer_email: Some(email.clone()),
            customer_name: Some("Guest User".into()),
            ..Default::default()
        })
        .await
        .expect("create cart");

    commerce
        .carts()
        .add_item(
            cart.id.into_uuid(),
            AddCartItem {
                product_id: Some(product.id),
                sku: sku.clone(),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(9.99),
                ..Default::default()
            },
        )
        .await
        .expect("add cart item");

    commerce
        .carts()
        .set_shipping_address(cart.id.into_uuid(), test_address())
        .await
        .expect("set shipping address");

    commerce
        .carts()
        .set_payment(
            cart.id.into_uuid(),
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_test".into()),
                billing_address: None,
            },
        )
        .await
        .expect("set payment");

    let result = commerce.carts().complete(cart.id.into_uuid()).await.expect("complete checkout");

    let updated_cart =
        commerce.carts().get(cart.id.into_uuid()).await.expect("get cart").expect("cart row");
    assert_eq!(updated_cart.status, CartStatus::Completed);

    let customer_id = updated_cart.customer_id.expect("guest checkout should attach a customer_id");

    let order = commerce
        .orders()
        .get(result.order_id.into_uuid())
        .await
        .expect("get order")
        .expect("order row");
    assert_eq!(order.customer_id, customer_id);

    let customer = commerce
        .customers()
        .get_by_email(&email)
        .await
        .expect("get customer by email")
        .expect("customer row");
    assert_eq!(customer.id, customer_id);
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_cart_checkout_cancel_releases_reservations() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres cart cancel test");
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
            first_name: "Cart".into(),
            last_name: "Cancel".into(),
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

    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_id: Some(customer.id),
            customer_email: Some(customer.email.clone()),
            customer_name: Some(format!("{} {}", customer.first_name, customer.last_name)),
            ..Default::default()
        })
        .await
        .expect("create cart");

    commerce
        .carts()
        .add_item(
            cart.id.into_uuid(),
            AddCartItem {
                product_id: Some(product.id),
                sku: sku.clone(),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(9.99),
                ..Default::default()
            },
        )
        .await
        .expect("add cart item");

    commerce
        .carts()
        .set_shipping_address(cart.id.into_uuid(), test_address())
        .await
        .expect("set shipping address");

    commerce
        .carts()
        .set_payment(
            cart.id.into_uuid(),
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_test".into()),
                billing_address: None,
            },
        )
        .await
        .expect("set payment");

    let result = commerce.carts().complete(cart.id.into_uuid()).await.expect("complete checkout");

    let reservations = commerce
        .inventory()
        .list_reservations_by_reference("order", &result.order_id.to_string())
        .await
        .expect("list reservations for order");
    assert!(!reservations.is_empty());
    assert!(reservations.iter().all(|r| r.status == ReservationStatus::Pending));

    let cancelled =
        commerce.orders().cancel(result.order_id.into_uuid()).await.expect("cancel order");
    assert_eq!(cancelled.status, OrderStatus::Cancelled);

    let reservations = commerce
        .inventory()
        .list_reservations_by_reference("order", &result.order_id.to_string())
        .await
        .expect("list reservations for order after cancel");
    assert!(
        reservations.iter().all(|r| r.status == ReservationStatus::Released),
        "expected reservations to be released after cancel"
    );
}
