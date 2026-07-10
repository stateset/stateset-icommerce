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

/// Regression: the async order-create path must reach parity with the sync
/// `Orders::create` by emitting a `CommerceEvent::OrderCreated` event. Prior to
/// the parity fix the async facade silently dropped this event, so any
/// async subscriber (webhooks, projections) never observed new orders.
#[cfg(all(feature = "postgres", feature = "events"))]
#[tokio::test]
async fn postgres_async_order_create_emits_order_created_event() {
    use stateset_core::CommerceEvent;

    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres async event test");
            return;
        }
    };

    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");

    // Subscribe before creating the order so we capture the emission.
    let mut subscription = commerce.events().subscribe();

    let unique = Uuid::new_v4().to_string();
    let sku = format!("SKU-{}", unique.replace('-', ""));

    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("evt-{}@example.com", unique),
            first_name: "Event".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");

    let product = commerce
        .products()
        .create(CreateProduct {
            name: format!("Evt Widget {}", unique),
            slug: Some(format!("evt-widget-{}", unique)),
            description: Some("Event test product".into()),
            ..Default::default()
        })
        .await
        .expect("create product");

    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: "Evt Widget".into(),
            initial_quantity: Some(dec!(5)),
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
                name: "Evt Widget".into(),
                quantity: 2,
                unit_price: dec!(9.99),
                discount: None,
                tax_amount: None,
            }],
            ..Default::default()
        })
        .await
        .expect("create order");

    // The async create must have emitted an OrderCreated event for this order.
    let mut saw_order_created = false;
    while let Some(event) = subscription.try_recv() {
        if let CommerceEvent::OrderCreated { order_id, customer_id, item_count, .. } = event {
            if order_id == order.id {
                assert_eq!(customer_id, order.customer_id, "event customer_id must match order");
                assert_eq!(item_count, order.items.len(), "event item_count must match order");
                saw_order_created = true;
                break;
            }
        }
    }
    assert!(
        saw_order_created,
        "async orders().create() must emit a CommerceEvent::OrderCreated (sync/async parity)"
    );
}

/// The async facade must expose gift cards, store credits, and loyalty with
/// the DB-layer money guards intact (sync/async parity).
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_async_gift_card_store_credit_loyalty_smoke() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
            return;
        }
    };

    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");
    let unique = Uuid::new_v4().to_string();

    // Gift cards: create, charge, guard, refund.
    let card = commerce
        .gift_cards()
        .create(stateset_core::CreateGiftCard {
            code: None,
            initial_balance: dec!(50.00),
            currency: stateset_core::CurrencyCode::USD,
            recipient_email: None,
            sender_name: None,
            message: None,
            expires_at: None,
        })
        .await
        .expect("create gift card");
    let txn = commerce.gift_cards().charge(card.id, dec!(30.00), None).await.expect("charge 30");
    assert_eq!(txn.balance_after, dec!(20.00));
    assert!(
        commerce.gift_cards().charge(card.id, dec!(-1.00), None).await.is_err(),
        "negative charge must be rejected through the async facade"
    );
    commerce.gift_cards().refund(card.id, dec!(5.00), None).await.expect("refund 5");

    // Store credits: issue against a real customer, apply, guard.
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("async-sc-{unique}@example.com"),
            first_name: "Async".into(),
            last_name: "Smoke".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    let credit = commerce
        .store_credits()
        .create(stateset_core::CreateStoreCredit {
            customer_id: customer.id,
            amount: dec!(40.00),
            currency: stateset_core::CurrencyCode::USD,
            reason: stateset_core::StoreCreditReason::Return,
            reference_id: None,
            note: None,
            expires_at: None,
        })
        .await
        .expect("create store credit");
    let txn = commerce
        .store_credits()
        .apply(credit.id.into_uuid(), dec!(15.00), None)
        .await
        .expect("apply 15");
    assert_eq!(txn.balance_after, dec!(25.00));
    assert!(
        commerce.store_credits().apply(credit.id.into_uuid(), dec!(99.00), None).await.is_err(),
        "overdraft apply must be rejected through the async facade"
    );

    // Loyalty: program, enrollment, earn, overdraft guard.
    let program = commerce
        .loyalty()
        .create_program(stateset_core::CreateLoyaltyProgram {
            name: format!("Async Program {unique}"),
            description: None,
            points_per_dollar: 1,
            tiers: vec![],
        })
        .await
        .expect("create program");
    let account = commerce
        .loyalty()
        .enroll(stateset_core::EnrollCustomer { customer_id: customer.id, program_id: program.id })
        .await
        .expect("enroll");
    commerce
        .loyalty()
        .adjust_points(stateset_core::AdjustPoints {
            account_id: account.id,
            points: 100,
            transaction_type: stateset_core::LoyaltyTransactionType::Earn,
            reference_id: None,
            description: None,
        })
        .await
        .expect("earn 100");
    assert!(
        commerce
            .loyalty()
            .adjust_points(stateset_core::AdjustPoints {
                account_id: account.id,
                points: -500,
                transaction_type: stateset_core::LoyaltyTransactionType::Redeem,
                reference_id: None,
                description: None,
            })
            .await
            .is_err(),
        "overdraft redemption must be rejected through the async facade"
    );
    let fetched =
        commerce.loyalty().get_account(account.id).await.expect("get account").expect("found");
    assert_eq!(fetched.points_balance, 100);
}
