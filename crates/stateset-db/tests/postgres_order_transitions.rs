#![cfg(feature = "postgres")]

use chrono::{Duration, Utc};
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
    CreatePayment, CreateRefund, CustomerId, OrderId, OrderStatus, PaymentMethodType,
    PaymentStatus, ProductId, ReservationStatus, UpdateOrder,
};
use stateset_db::PostgresDatabase;
use std::collections::HashMap;
use uuid::Uuid;

#[cfg(feature = "postgres")]
fn postgres_url() -> Option<String> {
    stateset_test_utils::postgres_url()
}

#[cfg(feature = "postgres")]
async fn setup_db() -> Option<PostgresDatabase> {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres transition test");
            return None;
        }
    };

    Some(PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"))
}

#[cfg(feature = "postgres")]
async fn create_customer(db: &PostgresDatabase, email: &str) -> stateset_core::Customer {
    db.customers()
        .create_async(CreateCustomer {
            email: email.to_string(),
            first_name: "Test".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .await
        .expect("create customer")
}

#[cfg(feature = "postgres")]
async fn create_order(
    db: &PostgresDatabase,
    customer_id: CustomerId,
    sku: &str,
) -> stateset_core::Order {
    db.orders()
        .create_async(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: sku.to_string(),
                name: "Widget".to_string(),
                quantity: 1,
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create order")
}

/// Record a real payment for the order's whole total (create + complete), so
/// the order's derived payment status becomes `paid`.
#[cfg(feature = "postgres")]
async fn pay_in_full(db: &PostgresDatabase, order_id: OrderId) -> stateset_core::Payment {
    let order = db.orders().get_async(order_id.into()).await.expect("get order").expect("order");
    let payment = db
        .payments()
        .create_async(CreatePayment {
            order_id: Some(order_id),
            payment_method: PaymentMethodType::CreditCard,
            amount: order.total_amount,
            currency: Some(order.currency),
            ..Default::default()
        })
        .await
        .expect("create payment");
    db.payments().mark_completed_async(payment.id.into()).await.expect("complete payment")
}

#[cfg(feature = "postgres")]
async fn create_inventory_item(db: &PostgresDatabase, sku: &str) -> String {
    db.inventory()
        .create_item_async(CreateInventoryItem {
            sku: sku.to_string(),
            name: "Inventory Widget".to_string(),
            description: None,
            unit_of_measure: None,
            initial_quantity: Some(dec!(1)),
            location_id: None,
            reorder_point: None,
            safety_stock: None,
        })
        .await
        .expect("create inventory item");
    sku.to_string()
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_rejects_invalid_order_status_transition() {
    let Some(db) = setup_db().await else {
        return;
    };

    let customer =
        create_customer(&db, &format!("transition-{}@example.com", Uuid::new_v4())).await;
    let order = create_order(&db, customer.id, "TRANS-001").await;

    let result = db
        .orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Delivered), ..Default::default() },
        )
        .await;

    assert!(matches!(result, Err(CommerceError::InvalidOrderStatusTransition { .. })));
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_rejects_cancel_after_shipped() {
    let Some(db) = setup_db().await else {
        return;
    };

    let customer = create_customer(&db, &format!("cancel-{}@example.com", Uuid::new_v4())).await;
    let order = create_order(&db, customer.id, "SHIP-001").await;

    db.orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Confirmed), ..Default::default() },
        )
        .await
        .expect("update order to confirmed");
    db.orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Processing), ..Default::default() },
        )
        .await
        .expect("update order to processing");
    db.orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Shipped), ..Default::default() },
        )
        .await
        .expect("update order to shipped");

    let result = db
        .orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() },
        )
        .await;

    assert!(matches!(result, Err(CommerceError::OrderCannotBeCancelled(_))));
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_rejects_refund_without_paid_status() {
    let Some(db) = setup_db().await else {
        return;
    };

    let customer = create_customer(&db, &format!("refund-{}@example.com", Uuid::new_v4())).await;
    let order = create_order(&db, customer.id, "REF-001").await;

    db.orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Confirmed), ..Default::default() },
        )
        .await
        .expect("update order to confirmed");
    db.orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Processing), ..Default::default() },
        )
        .await
        .expect("update order to processing");
    db.orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Shipped), ..Default::default() },
        )
        .await
        .expect("update order to shipped");
    db.orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Delivered), ..Default::default() },
        )
        .await
        .expect("update order to delivered");

    let result = db
        .orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Refunded), ..Default::default() },
        )
        .await;

    assert!(matches!(result, Err(CommerceError::OrderCannotBeRefunded(_))));
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_allows_refund_with_paid_status() {
    let Some(db) = setup_db().await else {
        return;
    };

    let customer =
        create_customer(&db, &format!("refund-paid-{}@example.com", Uuid::new_v4())).await;
    let order = create_order(&db, customer.id, "REF-002").await;

    db.orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Confirmed), ..Default::default() },
        )
        .await
        .expect("update order to confirmed");
    db.orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Processing), ..Default::default() },
        )
        .await
        .expect("update order to processing");
    db.orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Shipped), ..Default::default() },
        )
        .await
        .expect("update order to shipped");
    db.orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Delivered), ..Default::default() },
        )
        .await
        .expect("update order to delivered");

    // An order's payment status is derived from its payments and refunds: an
    // update that declares one is refused outright — alone, alongside a
    // refund transition (so an unpaid order cannot be made refundable by
    // declaring it paid), and inside an atomic batch — and nothing is written.
    let before = db.orders().get_async(order.id.into()).await.expect("get").expect("order");
    for declared in [PaymentStatus::Paid, PaymentStatus::Refunded] {
        for status in [None, Some(OrderStatus::Refunded)] {
            let result = db
                .orders()
                .update_async(
                    order.id.into(),
                    UpdateOrder { status, payment_status: Some(declared), ..Default::default() },
                )
                .await;
            assert!(
                matches!(&result, Err(CommerceError::ValidationError(m)) if m.contains("derived")),
                "declaring {declared} must be refused: {result:?}"
            );
        }
    }
    let batch = db
        .orders()
        .update_batch_atomic_async(vec![
            (order.id.into(), UpdateOrder { notes: Some("fine".into()), ..Default::default() }),
            (
                order.id.into(),
                UpdateOrder { payment_status: Some(PaymentStatus::Paid), ..Default::default() },
            ),
        ])
        .await;
    assert!(matches!(batch, Err(CommerceError::ValidationError(_))), "{batch:?}");
    let stored = db.orders().get_async(order.id.into()).await.expect("get").expect("order");
    assert_eq!(stored.status, OrderStatus::Delivered);
    assert_eq!(stored.payment_status, PaymentStatus::Pending);
    assert_eq!(stored.version, before.version, "nothing was written");

    // The supported path: record the payment and the refund; the order's
    // payment status follows them.
    let payment = pay_in_full(&db, order.id).await;
    let paid = db.orders().get_async(order.id.into()).await.expect("get").expect("order");
    assert_eq!(paid.payment_status, PaymentStatus::Paid);
    let refund = db
        .payments()
        .create_refund_async(CreateRefund { payment_id: payment.id, ..Default::default() })
        .await
        .expect("create refund");
    db.payments().complete_refund_async(refund.id).await.expect("complete refund");
    let updated = db
        .orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Refunded), ..Default::default() },
        )
        .await
        .expect("refund order");

    assert_eq!(updated.payment_status, PaymentStatus::Refunded, "derived from the refund");
    assert_eq!(updated.status, OrderStatus::Refunded);
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_ship_fails_when_reservation_expired() {
    let Some(db) = setup_db().await else {
        return;
    };

    let sku = format!("SHIP-EXPIRE-{}", Uuid::new_v4());
    create_inventory_item(&db, &sku).await;

    let customer =
        create_customer(&db, &format!("expired-reservation-{}@example.com", Uuid::new_v4())).await;
    let order = create_order(&db, customer.id, &sku).await;

    db.orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Confirmed), ..Default::default() },
        )
        .await
        .expect("update order to confirmed");
    db.orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Processing), ..Default::default() },
        )
        .await
        .expect("update order to processing");

    let reservations = db
        .inventory()
        .list_reservations_by_reference_async("order", &order.id.to_string())
        .await
        .expect("list reservations");
    assert!(!reservations.is_empty(), "expected reservation for order");
    let reservation = reservations[0].id;

    sqlx::query("UPDATE inventory_reservations SET expires_at = $1 WHERE id = $2")
        .bind(Utc::now() - Duration::minutes(5))
        .bind(reservation)
        .execute(db.pool())
        .await
        .expect("force reservation expiry");

    let result = db
        .orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Shipped), ..Default::default() },
        )
        .await;

    match result {
        Err(CommerceError::ReservationExpired(id)) => assert_eq!(id, reservation),
        other => panic!("expected ReservationExpired, got {other:?}"),
    }
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_ship_does_not_confirm_other_reservations_when_one_expired() {
    let Some(db) = setup_db().await else {
        return;
    };

    let sku_a = format!("PARTIAL-EXPIRE-A-{}", Uuid::new_v4());
    let sku_b = format!("PARTIAL-EXPIRE-B-{}", Uuid::new_v4());
    create_inventory_item(&db, &sku_a).await;
    create_inventory_item(&db, &sku_b).await;

    let customer =
        create_customer(&db, &format!("partial-expire-{}@example.com", Uuid::new_v4())).await;
    let order = db
        .orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![
                CreateOrderItem {
                    product_id: ProductId::new(),
                    sku: sku_a,
                    name: "Expirable Item A".to_string(),
                    quantity: 1,
                    unit_price: dec!(10.00),
                    ..Default::default()
                },
                CreateOrderItem {
                    product_id: ProductId::new(),
                    sku: sku_b,
                    name: "Expirable Item B".to_string(),
                    quantity: 1,
                    unit_price: dec!(12.00),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .await
        .expect("create order");

    db.orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Confirmed), ..Default::default() },
        )
        .await
        .expect("update order to confirmed");
    db.orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Processing), ..Default::default() },
        )
        .await
        .expect("update order to processing");

    let reservations = db
        .inventory()
        .list_reservations_by_reference_async("order", &order.id.to_string())
        .await
        .expect("list reservations");
    assert_eq!(reservations.len(), 2);

    let expired = reservations[0].id;
    let kept = reservations[1].id;

    sqlx::query("UPDATE inventory_reservations SET expires_at = $1 WHERE id = $2")
        .bind(Utc::now() - Duration::minutes(5))
        .bind(expired)
        .execute(db.pool())
        .await
        .expect("force reservation expiry");

    let result = db
        .orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Shipped), ..Default::default() },
        )
        .await;

    match result {
        Err(CommerceError::ReservationExpired(id)) => assert_eq!(id, expired),
        other => panic!("expected ReservationExpired, got {other:?}"),
    }

    let refreshed =
        db.orders().get_async(order.id.into()).await.expect("get order").expect("order exists");
    assert_eq!(refreshed.status, OrderStatus::Processing);

    let updated = db
        .inventory()
        .list_reservations_by_reference_async("order", &order.id.to_string())
        .await
        .expect("list reservations");
    assert_eq!(updated.len(), 2);

    let by_id = updated.into_iter().map(|r| (r.id, r.status)).collect::<HashMap<_, _>>();

    assert_eq!(by_id.get(&expired), Some(&ReservationStatus::Expired));
    assert_eq!(by_id.get(&kept), Some(&ReservationStatus::Pending));
}

/// Twin of the SQLite
/// `sqlite_cart_checkout_refuses_to_revive_a_cancelled_order_minted_for_the_cart`:
/// checkout reuses the order already minted for the cart (by `cart_id`), and
/// promoting it to `confirmed` is a status transition. A cancelled order must
/// stay cancelled — Postgres used to run the promotion with no transition
/// check, reviving it.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_cart_checkout_refuses_to_revive_a_cancelled_order_minted_for_the_cart() {
    use stateset_core::{AddCartItem, CartAddress, CartStatus, CreateCart};
    let Some(db) = setup_db().await else {
        return;
    };
    let email = format!("checkout-revive-{}@example.com", Uuid::new_v4());
    let customer = create_customer(&db, &email).await;
    let carts = db.carts();
    let cart = carts
        .create_async(CreateCart {
            customer_id: Some(customer.id),
            customer_email: Some(email.clone()),
            customer_name: Some("Checkout Revive".into()),
            ..Default::default()
        })
        .await
        .expect("create cart");
    carts
        .add_item_async(
            cart.id.into_uuid(),
            AddCartItem {
                // `cart_items.product_id` is a foreign key on Postgres; a
                // non-catalogue line carries none (checkout mints one).
                product_id: None,
                sku: format!("SKU-REVIVE-{}", Uuid::new_v4().simple()),
                name: "Revive Widget".into(),
                quantity: 1,
                unit_price: dec!(19.99),
                ..Default::default()
            },
        )
        .await
        .expect("add cart item");
    carts
        .set_shipping_address_async(
            cart.id.into_uuid(),
            CartAddress {
                first_name: "John".into(),
                last_name: "Doe".into(),
                company: None,
                line1: "123 Main St".into(),
                line2: None,
                city: "San Francisco".into(),
                state: Some("CA".into()),
                postal_code: "94102".into(),
                country: "US".into(),
                phone: None,
                email: Some(email.clone()),
            },
        )
        .await
        .expect("set shipping address");
    let cart = carts.get_async(cart.id.into_uuid()).await.expect("get cart").expect("cart exists");

    let order = db
        .orders()
        .create_from_cart_async(
            cart.id.into_uuid(),
            CreateOrder {
                customer_id: customer.id,
                items: cart
                    .items
                    .iter()
                    .map(|item| CreateOrderItem {
                        product_id: item.product_id.unwrap_or_else(ProductId::new),
                        variant_id: item.variant_id,
                        sku: item.sku.clone(),
                        name: item.name.clone(),
                        quantity: item.quantity,
                        unit_price: item.unit_price,
                        discount: Some(item.discount_amount),
                        tax_amount: Some(item.tax_amount),
                    })
                    .collect(),
                currency: Some(cart.currency),
                ..Default::default()
            },
        )
        .await
        .expect("create order for cart");
    let cancelled = db
        .orders()
        .update_async(
            order.id.into(),
            UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() },
        )
        .await
        .expect("cancel order");
    assert_eq!(cancelled.status, OrderStatus::Cancelled);

    let err = carts
        .complete_async(cart.id.into_uuid())
        .await
        .expect_err("checkout must not revive a cancelled order");
    assert!(
        matches!(err, CommerceError::InvalidOrderStatusTransition { .. }),
        "unexpected error: {err:?}"
    );
    let stored =
        db.orders().get_async(order.id.into()).await.expect("get order").expect("order exists");
    assert_eq!(stored.status, OrderStatus::Cancelled);
    assert_eq!(stored.version, cancelled.version, "the refused checkout wrote nothing");
    let reloaded =
        carts.get_async(cart.id.into_uuid()).await.expect("reload cart").expect("cart exists");
    assert_ne!(reloaded.status, CartStatus::Completed);
}
