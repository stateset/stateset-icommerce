#![cfg(feature = "postgres")]
//! Live PostgreSQL proofs for the v1.30–v1.33 kernel: durable economic
//! budgets, the governed checkout, and the stock policy it enforces.
//!
//! These features shipped with SQLite coverage only (`sqlite_kernel_outbox.rs`
//! and the `stateset-embedded` kernel unit tests), so the async executor —
//! `enforce_budget_pg`, `provision_economic_budget`,
//! `checkout_money_with_policy_in_tx` and the checkout savepoint — ran
//! untested. Each test below mirrors one of those SQLite proofs.
//!
//! Requires a live Postgres (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

use chrono::{Duration, SubsecRound, Timelike, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AddCartItem, CartAddress, CartStatus, CommandEnvelope, CommerceError, CommitCheckout,
    CreateCart, CreateCustomer, CreateInventoryItem, CreatePayment, CreateRefund, CurrencyCode,
    EconomicBudget, EconomicCommitment, ExecutionMode, ExecutionStatus, KernelCommandPolicy,
    KernelPolicy, KernelPrincipal, Money, PaymentMethodType, PrincipalKind, SetCartPayment,
    StockPolicy,
};
use stateset_db::PostgresDatabase;
use std::{env, sync::Arc};
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

async fn connect() -> Option<PostgresDatabase> {
    let url = postgres_url()?;
    Some(PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"))
}

macro_rules! require_db {
    () => {
        match connect().await {
            Some(db) => db,
            None => {
                eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
                return;
            }
        }
    };
}

const TENANT: &str = "tenant-r8-budget";
const STORE: &str = "store-r8-budget";
const AGENT: &str = "agent:r8-budget";

fn policy() -> KernelPolicy {
    KernelPolicy::new("commerce-policy-1")
        .allow("payments.create", KernelCommandPolicy::requiring(["payments.create"]).with_budget())
        .allow(
            "payments.create_refund",
            KernelCommandPolicy::requiring(["payments.create_refund"]).with_budget(),
        )
        .allow("checkout.commit", KernelCommandPolicy::requiring(["checkout.commit"]).with_budget())
}

/// A checkout policy without the budget requirement, for the stock and
/// fingerprint proofs that do not exercise money authority.
fn unbudgeted_policy() -> KernelPolicy {
    KernelPolicy::new("commerce-policy-1")
        .allow("checkout.commit", KernelCommandPolicy::requiring(["checkout.commit"]))
}

fn principal(capability: &str) -> KernelPrincipal {
    KernelPrincipal {
        id: AGENT.into(),
        kind: PrincipalKind::Agent,
        tenant_id: Some(TENANT.into()),
        delegated_by: Some("user-r8".into()),
        capabilities: vec![capability.into()],
    }
}

fn command<C>(command_type: &str, key: String, payload: C) -> CommandEnvelope<C> {
    let mut command = CommandEnvelope::preview(command_type, key, principal(command_type), payload);
    command.store_id = Some(STORE.into());
    command.policy_version = Some("commerce-policy-1".into());
    command.mode = ExecutionMode::Apply;
    command
}

fn budget(id: &str, limit: Decimal) -> EconomicBudget {
    let now = Utc::now();
    EconomicBudget::new(
        id,
        AGENT,
        Money::new(limit, CurrencyCode::USD),
        now - Duration::minutes(1),
        now + Duration::days(1),
    )
    .for_scope(TENANT, STORE)
}

fn spending(budget_id: &str, amount: Decimal) -> Option<EconomicCommitment> {
    Some(EconomicCommitment::for_money(budget_id, Money::new(amount, CurrencyCode::USD)))
}

fn payment(key: String, budget_id: &str, amount: Decimal) -> CommandEnvelope<CreatePayment> {
    let mut command = command(
        "payments.create",
        key,
        CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount,
            currency: Some(CurrencyCode::USD),
            ..Default::default()
        },
    );
    command.commitment = spending(budget_id, amount);
    command
}

// ---------------------------------------------------------------------------
// provision_economic_budget
// ---------------------------------------------------------------------------

/// `TIMESTAMPTZ` keeps microseconds, so a definition carrying the nanosecond
/// precision `Utc::now()` hands out came back different from what was stored
/// and a retried provisioning call conflicted with itself.
#[tokio::test]
async fn postgres_budget_provisioning_is_idempotent_at_nanosecond_precision() {
    let db = require_db!();
    let executor = db.kernel_executor(policy());
    let id = format!("budget:r8-nanos-{}", Uuid::new_v4());
    let mut definition = budget(&id, dec!(20.00));
    definition.valid_from = definition.valid_from.with_nanosecond(123_456_789).expect("nanos");
    definition.expires_at = definition.expires_at.with_nanosecond(987_654_321).expect("nanos");
    assert_ne!(definition.valid_from, definition.valid_from.trunc_subsecs(6));

    let first = executor.provision_economic_budget(&definition).await.expect("provision");
    let second = executor
        .provision_economic_budget(&definition)
        .await
        .expect("re-provisioning the identical definition is idempotent");
    assert_eq!(first.budget, second.budget);
    assert_eq!(first.available.amount, "20.00");
    assert_eq!(
        first.budget.valid_from,
        definition.valid_from.trunc_subsecs(6),
        "the stored definition is the caller's, truncated to storage precision"
    );
    assert_eq!(first.budget.expires_at, definition.expires_at.trunc_subsecs(6));
}

#[tokio::test]
async fn postgres_budget_definitions_are_immutable_after_creation() {
    let db = require_db!();
    let executor = db.kernel_executor(policy());
    let id = format!("budget:r8-immutable-{}", Uuid::new_v4());
    let definition = budget(&id, dec!(20.00));
    executor.provision_economic_budget(&definition).await.expect("provision");

    let mut changed = definition.clone();
    changed.limit = Money::new(dec!(21.00), CurrencyCode::USD).to_wire();
    let conflict = executor
        .provision_economic_budget(&changed)
        .await
        .expect_err("budget definitions are immutable");
    assert!(matches!(conflict, CommerceError::Conflict(_)), "{conflict:?}");

    let mut shifted = definition;
    shifted.expires_at += Duration::seconds(1);
    let conflict = executor
        .provision_economic_budget(&shifted)
        .await
        .expect_err("a changed window is a different definition");
    assert!(matches!(conflict, CommerceError::Conflict(_)), "{conflict:?}");
}

// ---------------------------------------------------------------------------
// enforce_budget_pg
// ---------------------------------------------------------------------------

/// The `FOR UPDATE` path: concurrent debits against one budget serialize, and
/// the committed total never exceeds the limit.
#[tokio::test]
async fn postgres_concurrent_debits_cannot_overspend_one_budget() {
    let db = Arc::new(require_db!());
    let id = format!("budget:r8-concurrent-{}", Uuid::new_v4());
    db.kernel_executor(policy())
        .provision_economic_budget(&budget(&id, dec!(20.00)))
        .await
        .expect("provision");

    let barrier = Arc::new(tokio::sync::Barrier::new(4));
    let mut tasks = Vec::new();
    for contender in 0..4 {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let command = payment(format!("r8-budget-race-{id}-{contender}"), &id, dec!(12.00));
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            db.kernel_executor(policy()).execute_create_payment_async(&command).await
        }));
    }
    let mut receipts = Vec::new();
    for task in tasks {
        receipts.push(task.await.expect("join").expect("durable outcome"));
    }
    assert_eq!(
        receipts.iter().filter(|r| r.status == ExecutionStatus::Succeeded).count(),
        1,
        "only one 12.00 debit fits a 20.00 budget: {receipts:?}"
    );
    assert_eq!(
        receipts
            .iter()
            .filter(|r| r.error_code.as_deref() == Some("kernel.budget_exceeded"))
            .count(),
        3
    );
    let status = db
        .kernel_executor(policy())
        .economic_budget_status(&id)
        .await
        .expect("budget status")
        .expect("budget exists");
    assert_eq!(status.committed.amount, "12.00");
    assert_eq!(status.available.amount, "8.00");
}

/// A preview commits no money, so it must not take the row lock an apply
/// needs. It used to take `FOR UPDATE` unconditionally and block behind any
/// in-flight debit of the same budget.
#[tokio::test]
async fn postgres_budget_preview_does_not_wait_on_a_locked_budget_row() {
    let db = require_db!();
    let id = format!("budget:r8-preview-lock-{}", Uuid::new_v4());
    db.kernel_executor(policy())
        .provision_economic_budget(&budget(&id, dec!(50.00)))
        .await
        .expect("provision");

    // Hold the budget row exactly as an in-flight apply would.
    let mut holder = db.pool().begin().await.expect("begin holder transaction");
    let held: String = sqlx::query_scalar(
        "SELECT budget_id FROM kernel_economic_budgets WHERE budget_id = $1 FOR UPDATE",
    )
    .bind(&id)
    .fetch_one(holder.as_mut())
    .await
    .expect("lock the budget row");
    assert_eq!(held, id);

    let mut preview = payment(format!("r8-budget-preview-{id}"), &id, dec!(10.00));
    preview.mode = ExecutionMode::Preview;
    let receipt = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        db.kernel_executor(policy()).execute_create_payment_async(&preview),
    )
    .await
    .expect("a preview must not block on a locked budget row")
    .expect("preview receipt");
    assert_eq!(receipt.status, ExecutionStatus::Previewed);

    holder.rollback().await.expect("release the budget row");
    let status = db
        .kernel_executor(policy())
        .economic_budget_status(&id)
        .await
        .expect("budget status")
        .expect("budget exists");
    assert_eq!(status.committed.amount, "0", "a preview commits nothing");
}

#[tokio::test]
async fn postgres_budget_window_and_scope_are_enforced_by_the_executor() {
    let db = require_db!();
    let executor = db.kernel_executor(policy());
    let now = Utc::now();
    let suffix = Uuid::new_v4();

    let expired_id = format!("budget:r8-expired-{suffix}");
    let mut expired = budget(&expired_id, dec!(50.00));
    expired.valid_from = now - Duration::days(2);
    expired.expires_at = now - Duration::minutes(1);
    executor.provision_economic_budget(&expired).await.expect("provision expired");

    let future_id = format!("budget:r8-future-{suffix}");
    let mut not_yet = budget(&future_id, dec!(50.00));
    not_yet.valid_from = now + Duration::hours(1);
    not_yet.expires_at = now + Duration::days(2);
    executor.provision_economic_budget(&not_yet).await.expect("provision future");

    let other_tenant_id = format!("budget:r8-other-tenant-{suffix}");
    let other_tenant = EconomicBudget::new(
        &other_tenant_id,
        AGENT,
        Money::new(dec!(50.00), CurrencyCode::USD),
        now - Duration::minutes(1),
        now + Duration::days(1),
    )
    .for_scope("tenant-somebody-else", STORE);
    executor.provision_economic_budget(&other_tenant).await.expect("provision other tenant");

    let missing_id = format!("budget:r8-missing-{suffix}");

    for (budget_id, expected) in [
        (&expired_id, "kernel.budget_expired"),
        (&future_id, "kernel.budget_not_yet_valid"),
        (&other_tenant_id, "kernel.budget_tenant_mismatch"),
        (&missing_id, "kernel.budget_not_found"),
    ] {
        let command = payment(format!("r8-budget-window-{budget_id}"), budget_id, dec!(10.00));
        let receipt =
            executor.execute_create_payment_async(&command).await.expect("sealed rejection");
        assert_eq!(receipt.status, ExecutionStatus::Rejected, "{budget_id}");
        assert_eq!(receipt.error_code.as_deref(), Some(expected), "{budget_id}");
    }
}

/// Current semantics, deliberately unchanged in this round: a refund *spends*
/// budget rather than returning it, exactly as the SQLite executor does.
#[tokio::test]
async fn postgres_refunds_consume_budget_rather_than_restoring_it() {
    let db = require_db!();
    let executor = db.kernel_executor(policy());
    let id = format!("budget:r8-refund-{}", Uuid::new_v4());
    executor.provision_economic_budget(&budget(&id, dec!(200.00))).await.expect("provision");

    let paid = db
        .payments()
        .create_async(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(100.00),
            currency: Some(CurrencyCode::USD),
            ..Default::default()
        })
        .await
        .expect("create payment");
    db.payments().mark_completed_async(paid.id.into_uuid()).await.expect("complete payment");

    let mut refund = command(
        "payments.create_refund",
        format!("r8-refund-{id}"),
        CreateRefund {
            payment_id: paid.id,
            amount: Some(dec!(20.00)),
            reason: Some("round 8 budget semantics".into()),
            ..Default::default()
        },
    );
    refund.commitment = spending(&id, dec!(20.00));
    let receipt = executor.execute_create_refund_async(&refund).await.expect("refund");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{receipt:?}");

    let status =
        executor.economic_budget_status(&id).await.expect("budget status").expect("budget exists");
    assert_eq!(status.committed.amount, "20.00", "refunds debit the budget in this round");
    assert_eq!(status.available.amount, "180.00");
}

// ---------------------------------------------------------------------------
// checkout.commit
// ---------------------------------------------------------------------------

async fn stock(db: &PostgresDatabase, sku: &str, quantity: Decimal) {
    db.inventory()
        .create_item_async(CreateInventoryItem {
            sku: sku.into(),
            name: format!("Stock {sku}"),
            initial_quantity: Some(quantity),
            ..Default::default()
        })
        .await
        .expect("create stock");
}

async fn ready_cart(db: &PostgresDatabase, sku: &str, quantity: i32) -> stateset_core::CartId {
    let email = format!("r8-budget-{}@example.com", Uuid::new_v4());
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: email.clone(),
            first_name: "Round".into(),
            last_name: "Eight".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    let carts = db.carts();
    let cart = carts
        .create_async(CreateCart {
            customer_id: Some(customer.id),
            customer_email: Some(email.clone()),
            customer_name: Some("Round Eight".into()),
            ..Default::default()
        })
        .await
        .expect("create cart");
    carts
        .add_item_async(
            cart.id.into_uuid(),
            AddCartItem {
                product_id: None,
                sku: sku.into(),
                name: "Round Eight Item".into(),
                quantity,
                unit_price: dec!(10.00),
                ..Default::default()
            },
        )
        .await
        .expect("add item");
    carts
        .set_shipping_address_async(
            cart.id.into_uuid(),
            CartAddress {
                first_name: "Round".into(),
                last_name: "Eight".into(),
                line1: "1 Atomic Way".into(),
                city: "Vancouver".into(),
                state: Some("BC".into()),
                postal_code: "V6B 1A1".into(),
                country: "CA".into(),
                email: Some(email),
                ..Default::default()
            },
        )
        .await
        .expect("set shipping");
    carts
        .set_payment_async(
            cart.id.into_uuid(),
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_r8_budget".into()),
                ..Default::default()
            },
        )
        .await
        .expect("set payment");
    cart.id
}

async fn cart_total(db: &PostgresDatabase, cart: stateset_core::CartId) -> Decimal {
    db.carts().get_async(cart.into_uuid()).await.expect("load cart").expect("cart").grand_total
}

/// A cart is one economic event: a second, differently-keyed checkout command
/// against a completed cart is a durable conflict, not a second order.
#[tokio::test]
async fn postgres_checkout_conflicts_on_an_already_completed_cart() {
    let db = require_db!();
    let executor = db.kernel_executor(policy());
    let suffix = Uuid::new_v4();
    let id = format!("budget:r8-checkout-{suffix}");
    executor.provision_economic_budget(&budget(&id, dec!(500.00))).await.expect("provision");
    let cart = ready_cart(&db, &format!("R8-BUDGET-CHECKOUT-{suffix}"), 2).await;
    let total = cart_total(&db, cart).await;

    let mut first = command(
        "checkout.commit",
        format!("r8-checkout-first-{suffix}"),
        CommitCheckout::new(cart),
    );
    first.commitment = spending(&id, total);
    let committed = executor.execute_commit_checkout_async(&first).await.expect("commit checkout");
    assert_eq!(committed.status, ExecutionStatus::Succeeded, "{committed:?}");

    let mut second = command(
        "checkout.commit",
        format!("r8-checkout-second-{suffix}"),
        CommitCheckout::new(cart),
    );
    second.commitment = spending(&id, total);
    let conflict = executor.execute_commit_checkout_async(&second).await.expect("sealed rejection");
    assert_eq!(conflict.status, ExecutionStatus::Rejected);
    assert_eq!(conflict.error_code.as_deref(), Some("commerce.checkout.conflict"));
    let status =
        executor.economic_budget_status(&id).await.expect("budget status").expect("budget exists");
    assert_eq!(status.committed.amount, total.to_string());
}

/// The checkout savepoint: a budget refusal *after* the checkout applied must
/// leave no order and an untouched cart.
#[tokio::test]
async fn postgres_checkout_budget_refusal_rolls_the_applied_checkout_back() {
    let db = require_db!();
    let executor = db.kernel_executor(policy());
    let suffix = Uuid::new_v4();
    let id = format!("budget:r8-checkout-poor-{suffix}");
    executor.provision_economic_budget(&budget(&id, dec!(1.00))).await.expect("provision");
    let cart = ready_cart(&db, &format!("R8-BUDGET-POOR-{suffix}"), 2).await;
    let total = cart_total(&db, cart).await;
    assert!(total > dec!(1.00));

    let mut checkout =
        command("checkout.commit", format!("r8-checkout-poor-{suffix}"), CommitCheckout::new(cart));
    checkout.commitment = spending(&id, total);
    let receipt =
        executor.execute_commit_checkout_async(&checkout).await.expect("sealed rejection");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.budget_exceeded"));

    let untouched = db.carts().get_async(cart.into_uuid()).await.expect("load cart").expect("cart");
    assert_eq!(untouched.status, CartStatus::Active);
    assert_eq!(untouched.order_id, None);
    let orders: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orders WHERE cart_id = $1")
        .bind(cart.into_uuid())
        .fetch_one(db.pool())
        .await
        .expect("count orders");
    assert_eq!(orders, 0);
    let status =
        executor.economic_budget_status(&id).await.expect("budget status").expect("budget exists");
    assert_eq!(status.committed.amount, "0");
}

/// A quote is a commitment to the whole cart snapshot: changing the terms
/// after the quote is a conflict in preview and apply alike, with no effects.
#[tokio::test]
async fn postgres_checkout_fingerprint_mismatch_is_a_durable_conflict() {
    let db = require_db!();
    let executor = db.kernel_executor(unbudgeted_policy());
    let suffix = Uuid::new_v4();
    let cart = ready_cart(&db, &format!("R8-BUDGET-FINGERPRINT-{suffix}"), 2).await;
    let quoted = db
        .carts()
        .get_async(cart.into_uuid())
        .await
        .expect("load cart")
        .expect("cart")
        .checkout_fingerprint()
        .expect("fingerprint");
    sqlx::query("UPDATE cart_items SET sku = $1 WHERE cart_id = $2")
        .bind(format!("R8-SUBSTITUTED-{suffix}"))
        .bind(cart.into_uuid())
        .execute(db.pool())
        .await
        .expect("substitute the quoted line");

    for mode in [ExecutionMode::Preview, ExecutionMode::Apply] {
        let mut checkout = command(
            "checkout.commit",
            format!("r8-fingerprint-{mode:?}-{suffix}"),
            CommitCheckout::new(cart),
        );
        checkout.mode = mode;
        checkout.payload.expected_cart_fingerprint = Some(quoted.clone());
        let receipt =
            executor.execute_commit_checkout_async(&checkout).await.expect("sealed rejection");
        assert_eq!(receipt.status, ExecutionStatus::Rejected, "{mode:?}: {receipt:?}");
        assert_eq!(receipt.error_code.as_deref(), Some("commerce.checkout.conflict"));
        assert!(receipt.error_message.as_deref().expect("message").contains("fingerprint"));
    }
    let orders: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orders WHERE cart_id = $1")
        .bind(cart.into_uuid())
        .fetch_one(db.pool())
        .await
        .expect("count orders");
    assert_eq!(orders, 0);
}

/// `StockPolicy::RejectIfInsufficient` inside the checkout transaction: two
/// buyers, one unit, exactly one order.
#[tokio::test]
async fn postgres_checkout_strict_stock_lets_only_one_of_two_buyers_win() {
    let db = Arc::new(require_db!());
    let suffix = Uuid::new_v4();
    let sku = format!("R8-BUDGET-STRICT-{suffix}");
    stock(&db, &sku, dec!(1)).await;
    let carts = [ready_cart(&db, &sku, 1).await, ready_cart(&db, &sku, 1).await];

    let barrier = Arc::new(tokio::sync::Barrier::new(carts.len()));
    let mut tasks = Vec::new();
    for (index, cart) in carts.into_iter().enumerate() {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let mut checkout = command(
            "checkout.commit",
            format!("r8-strict-{suffix}-{index}"),
            CommitCheckout::new(cart),
        );
        checkout.payload.stock_policy = Some(StockPolicy::RejectIfInsufficient);
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            let executor = db.kernel_executor(unbudgeted_policy());
            let receipt = executor.execute_commit_checkout_async(&checkout).await.expect("outcome");
            let replay = executor.execute_commit_checkout_async(&checkout).await.expect("replay");
            assert_eq!(receipt.receipt_id, replay.receipt_id);
            receipt
        }));
    }
    let mut receipts = Vec::new();
    for task in tasks {
        receipts.push(task.await.expect("join"));
    }
    assert_eq!(
        receipts.iter().filter(|r| r.status == ExecutionStatus::Succeeded).count(),
        1,
        "one unit cannot satisfy two strict carts: {receipts:?}"
    );
    assert_eq!(
        receipts
            .iter()
            .filter(|r| {
                r.error_code.as_deref() == Some("commerce.inventory.insufficient_available")
            })
            .count(),
        1
    );
    let allocated = db
        .inventory()
        .get_stock_async(&sku)
        .await
        .expect("stock query")
        .expect("stock")
        .total_allocated;
    assert_eq!(allocated, dec!(1));
    let backorders: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM backorders WHERE sku = $1")
        .bind(&sku)
        .fetch_one(db.pool())
        .await
        .expect("count backorders");
    assert_eq!(backorders, 0, "strict stock never backorders");
}
