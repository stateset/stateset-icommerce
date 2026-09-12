#![cfg(feature = "sqlite")]

//! Every money-path mutation wired in Phase B task 4 emits a fact whose
//! aggregate identity is usable by a peer.
//!
//! The `outbox_emission_parity` lint proves a `record_outbox_fact` call
//! exists in the method. These tests prove the call says something true: the
//! event type is the one the log's vocabulary uses, the aggregate id locates
//! the thing that changed (the cart, not the line), and the fact commits with
//! the mutation or not at all.

use rust_decimal_macros::dec;
use stateset_core::{
    AccountsPayableRepository, AddCartItem, BillingCycleFilter, BillingCycleStatus,
    BillingInterval, CartAddress, CartRepository, CreateBill, CreateBillItem, CreateCart,
    CreateCreditAccount, CreateCustomer, CreateSubscription, CreateSubscriptionPlan,
    CreditRepository, CreditTransactionType, CurrencyCode, CustomerId, CustomerRepository, OrderId,
    ProductId, RecordCreditTransaction, SetCartPayment,
};
use stateset_db::SqliteDatabase;

/// The recorded fact for `event_type` on `aggregate_id`, as
/// `(aggregate_type, payload, tier)`.
fn fact(
    db: &SqliteDatabase,
    event_type: &str,
    aggregate_id: &str,
) -> Option<(String, serde_json::Value, String)> {
    let conn = db.pool().get().expect("connection");
    conn.query_row(
        "SELECT aggregate_type, payload, tier FROM kernel_outbox
         WHERE event_type = ?1 AND aggregate_id = ?2
         ORDER BY created_at DESC LIMIT 1",
        rusqlite::params![event_type, aggregate_id],
        |row| {
            let payload: String = row.get(1)?;
            Ok((
                row.get::<_, String>(0)?,
                serde_json::from_str(&payload).expect("payload is json"),
                row.get::<_, String>(2)?,
            ))
        },
    )
    .ok()
}

fn outbox_count(db: &SqliteDatabase) -> i64 {
    let conn = db.pool().get().expect("connection");
    conn.query_row("SELECT COUNT(*) FROM kernel_outbox", [], |row| row.get(0)).expect("count")
}

fn test_address() -> CartAddress {
    CartAddress {
        first_name: "Fact".into(),
        last_name: "Checker".into(),
        company: None,
        line1: "1 Ledger Way".into(),
        line2: None,
        city: "San Francisco".into(),
        state: Some("CA".into()),
        postal_code: "94102".into(),
        country: "US".into(),
        phone: None,
        email: Some("fact.checker@example.com".into()),
    }
}

// ============================================================================
// carts.rs — complete_checkout_with_policy_in_tx
// ============================================================================

#[test]
fn checkout_emits_a_fact_naming_the_cart_and_its_order() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let carts = db.carts();
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: "checkout-fact@example.com".into(),
            first_name: "Checkout".into(),
            last_name: "Fact".into(),
            ..Default::default()
        })
        .expect("create customer");

    let cart = carts
        .create(CreateCart {
            customer_id: Some(customer.id),
            customer_email: Some("checkout-fact@example.com".into()),
            customer_name: Some("Checkout Fact".into()),
            ..Default::default()
        })
        .expect("create cart");
    carts
        .add_item(
            cart.id,
            AddCartItem {
                product_id: Some(ProductId::new()),
                sku: "SKU-FACT-1".into(),
                name: "Fact widget".into(),
                quantity: 2,
                unit_price: dec!(10.00),
                ..Default::default()
            },
        )
        .expect("add item");
    carts.set_shipping_address(cart.id, test_address()).expect("set shipping address");
    carts
        .set_payment(
            cart.id,
            SetCartPayment { payment_method: "credit_card".into(), ..Default::default() },
        )
        .expect("set payment");

    let checkout = carts.complete(cart.id).expect("complete checkout");

    let (aggregate_type, payload, tier) =
        fact(&db, "cart.checked_out", &cart.id.to_string()).expect("checkout must emit a fact");
    assert_eq!(aggregate_type, "cart");
    assert_eq!(tier, "recorded");
    assert_eq!(
        payload["order_id"].as_str(),
        Some(checkout.order_id.to_string().as_str()),
        "the fact must name the order the checkout minted"
    );
}

// ============================================================================
// subscriptions.rs — record_event_with_conn, insert_billing_cycle_with_conn,
//                    apply_billing_cycle_status_with_tx
// ============================================================================

fn seeded_subscription(db: &SqliteDatabase, email: &str) -> (uuid::Uuid, uuid::Uuid) {
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: email.into(),
            first_name: "Sub".into(),
            last_name: "Fact".into(),
            ..Default::default()
        })
        .expect("create customer");
    let subscriptions = db.subscriptions();
    let plan = subscriptions
        .create_plan(CreateSubscriptionPlan {
            code: None,
            name: "Fact Monthly".into(),
            description: None,
            billing_interval: BillingInterval::Monthly,
            custom_interval_days: None,
            price: dec!(29.99),
            setup_fee: None,
            currency: Some(CurrencyCode::USD),
            trial_days: Some(0),
            trial_requires_payment_method: Some(true),
            min_cycles: None,
            max_cycles: None,
            items: None,
            discount_percent: None,
            discount_amount: None,
            metadata: None,
        })
        .expect("create plan");
    subscriptions.activate_plan(plan.id).expect("activate plan");
    let subscription = subscriptions
        .create_subscription(CreateSubscription {
            customer_id: customer.id,
            plan_id: plan.id,
            payment_method_id: Some("pm_fact".into()),
            skip_trial: Some(true),
            ..Default::default()
        })
        .expect("create subscription");
    let cycle = subscriptions
        .list_billing_cycles(BillingCycleFilter {
            subscription_id: Some(subscription.id),
            ..Default::default()
        })
        .expect("list billing cycles")
        .into_iter()
        .next()
        .expect("initial billing cycle");
    (subscription.id.into(), cycle.id)
}

#[test]
fn subscription_lifecycle_events_emit_recorded_facts() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let (subscription_id, _) = seeded_subscription(&db, "subscription-fact@example.com");

    let (aggregate_type, payload, tier) =
        fact(&db, "subscription.created", &subscription_id.to_string())
            .expect("creating a subscription must emit a lifecycle fact");
    assert_eq!(aggregate_type, "subscription");
    assert_eq!(tier, "recorded");
    assert_eq!(payload["event_type"].as_str(), Some("created"));

    assert!(
        fact(&db, "subscription.activated", &subscription_id.to_string()).is_some(),
        "a subscription created without a trial is activated, and that is a fact too"
    );
}

#[test]
fn seeding_a_billing_cycle_emits_a_fact_naming_the_cycle() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let (subscription_id, cycle_id) = seeded_subscription(&db, "cycle-fact@example.com");

    let (aggregate_type, payload, _) = fact(&db, "billing_cycle.scheduled", &cycle_id.to_string())
        .expect("seeding cycle 1 must emit a fact");
    assert_eq!(aggregate_type, "billing_cycle");
    assert_eq!(payload["subscription_id"].as_str(), Some(subscription_id.to_string().as_str()));
    assert_eq!(payload["cycle_number"].as_i64(), Some(1));
}

#[test]
fn a_billing_cycle_status_change_emits_a_fact() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let (_, cycle_id) = seeded_subscription(&db, "cycle-status-fact@example.com");

    db.subscriptions()
        .update_billing_cycle_status(cycle_id, BillingCycleStatus::Processing, None, None)
        .expect("move the cycle to processing");

    let (aggregate_type, payload, _) =
        fact(&db, "billing_cycle.status_changed", &cycle_id.to_string())
            .expect("a cycle status change must emit a fact");
    assert_eq!(aggregate_type, "billing_cycle");
    assert_eq!(payload["from"].as_str(), Some("scheduled"));
    assert_eq!(payload["to"].as_str(), Some("processing"));
}

// ============================================================================
// credit.rs — create_credit_account_with_conn, insert_transaction_with_conn,
//             release_reservation_with_conn
// ============================================================================

fn credit_account(db: &SqliteDatabase, limit: rust_decimal::Decimal) -> CustomerId {
    let customer_id = CustomerId::new();
    db.credit()
        .create_credit_account(CreateCreditAccount {
            customer_id,
            credit_limit: limit,
            ..Default::default()
        })
        .expect("create credit account");
    customer_id
}

#[test]
fn creating_a_credit_account_emits_a_fact_naming_the_customer() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let customer_id = credit_account(&db, dec!(1000));

    let (aggregate_type, payload, tier) =
        fact(&db, "credit_account.created", &customer_id.to_string())
            .expect("creating a credit account must emit a fact");
    assert_eq!(aggregate_type, "credit_account");
    assert_eq!(tier, "recorded");
    assert_eq!(payload["credit_limit"].as_str(), Some("1000"));
}

#[test]
fn a_credit_transaction_emits_a_fact_carrying_the_running_balance() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let customer_id = credit_account(&db, dec!(1000));

    db.credit()
        .record_transaction(RecordCreditTransaction {
            customer_id,
            transaction_type: CreditTransactionType::Payment,
            amount: dec!(25),
            reference_type: None,
            reference_id: None,
            notes: None,
        })
        .expect("record transaction");

    let (aggregate_type, payload, _) =
        fact(&db, "credit_account.transaction_recorded", &customer_id.to_string())
            .expect("a credit ledger append must emit a fact");
    assert_eq!(aggregate_type, "credit_account");
    assert_eq!(payload["transaction_type"].as_str(), Some("payment"));
    assert_eq!(payload["amount"].as_str(), Some("25"));
    assert_eq!(payload["running_balance"].as_str(), Some("-25"));
}

#[test]
fn releasing_a_credit_reservation_emits_a_fact_naming_the_reservation() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let customer_id = credit_account(&db, dec!(1000));
    let order_id = OrderId::new();
    db.credit().reserve_credit(customer_id, order_id, dec!(100)).expect("reserve");

    db.credit().release_credit_reservation(customer_id, order_id).expect("release");

    let conn = db.pool().get().expect("connection");
    let reservation_id: String = conn
        .query_row(
            "SELECT id FROM credit_reservations WHERE customer_id = ?1 AND order_id = ?2",
            rusqlite::params![customer_id.to_string(), order_id.to_string()],
            |row| row.get(0),
        )
        .expect("reservation row");
    drop(conn);

    let (aggregate_type, payload, _) = fact(&db, "credit_reservation.released", &reservation_id)
        .expect("releasing a reservation must emit a fact");
    assert_eq!(aggregate_type, "credit_reservation");
    assert_eq!(payload["order_id"].as_str(), Some(order_id.to_string().as_str()));
    assert_eq!(payload["amount"].as_str(), Some("100"));
}

#[test]
fn a_rejected_credit_charge_leaves_no_outbox_fact() {
    // `charge_credit` releases this order's reservation (emitting its fact)
    // BEFORE testing the credit line. A charge that busts the line must roll
    // the release — and its fact — back with everything else.
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let customer_id = credit_account(&db, dec!(100));
    let order_id = OrderId::new();
    db.credit().reserve_credit(customer_id, order_id, dec!(50)).expect("reserve");

    let before = outbox_count(&db);
    db.credit()
        .charge_credit(customer_id, order_id, dec!(200))
        .expect_err("a charge past the credit line must be refused");
    let after = outbox_count(&db);

    assert_eq!(
        before, after,
        "the fact is written in the mutation's transaction, so a rollback must take it too"
    );
    let conn = db.pool().get().expect("connection");
    let status: String = conn
        .query_row(
            "SELECT status FROM credit_reservations WHERE customer_id = ?1 AND order_id = ?2",
            rusqlite::params![customer_id.to_string(), order_id.to_string()],
            |row| row.get(0),
        )
        .expect("reservation row");
    assert_eq!(status, "active", "the rejected charge must not have consumed the reservation");
}

// ============================================================================
// accounts_payable.rs — insert_bill_item_with_conn
// ============================================================================

#[test]
fn adding_a_bill_item_emits_a_fact_naming_the_bill() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let ap = db.accounts_payable();
    let bill = ap
        .create_bill(CreateBill {
            supplier_id: uuid::Uuid::new_v4(),
            due_date: "2026-10-10T00:00:00Z".parse().expect("due date"),
            items: vec![],
            ..Default::default()
        })
        .expect("create bill");

    let item = ap
        .add_bill_item(
            bill.id,
            CreateBillItem {
                description: "Consulting".into(),
                account_code: None,
                quantity: dec!(2),
                unit_price: dec!(150),
                tax_rate: None,
                ..Default::default()
            },
        )
        .expect("add bill item");

    let (aggregate_type, payload, tier) = fact(&db, "bill.item_added", &bill.id.to_string())
        .expect("adding a bill line must emit a fact");
    assert_eq!(aggregate_type, "bill", "the fact must locate the bill, not the line");
    assert_eq!(tier, "recorded");
    assert_eq!(payload["item_id"].as_str(), Some(item.id.to_string().as_str()));
    assert_eq!(payload["amount"].as_str(), Some("300"), "2 x 150, as stored");
}
