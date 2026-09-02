#![cfg(all(feature = "postgres", feature = "sqlite"))]
//! Kernel executor round 5 (PostgreSQL): mirrors of `sqlite_kernel_round5.rs`
//! plus the proofs SQLite already had and Postgres lacked (idempotency
//! conflict, policy denial, version conflict, receipt-append rollback), the
//! shipment expiry race, preview envelopes for every cheap op kind, and the
//! cross-backend receipt-shape proof.
//!
//! Requires a live Postgres (`POSTGRES_URL` / `DATABASE_URL`); skipped otherwise.

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::Serialize;
use serde_json::{Value, json};
use stateset_core::{
    A2ADisputeResolutionType, ApprovalEvidence, ChargeSubscription, CommandEnvelope, CommerceError,
    CommitCheckout, ConfirmInventoryReservation, CreateA2AEscrow, CreateCustomer,
    CreateInventoryItem, CreateOrder, CreateOrderItem, CreatePayment, CreateProduct, CreateRefund,
    CustomerRepository, DisputeA2AEscrow, ExecutionMode, ExecutionReceipt, ExecutionStatus,
    FileA2ADispute, FundA2AEscrow, InventoryRepository, KernelCommandPolicy, KernelPolicy,
    KernelPrincipal, OrderRepository, OrderStatus, Payment, PaymentMethodType,
    PaymentTransactionStatus, PostJournalEntry, PrincipalKind, ProductId, RefundA2AEscrow,
    ReleaseA2AEscrow, ReleaseInventoryReservation, ReserveInventory, ResolveA2ADispute,
    RetryDisposition, ReturnStatus, SettleX402Intent, ShipOrderCommand, SubmitA2ADisputeEvidence,
    TransitionOrder, TransitionReturn,
};
use stateset_db::{PostgresDatabase, SqliteDatabase};
use std::env;
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

const ALL_COMMANDS: &[&str] = &[
    "inventory.item.create",
    "products.create",
    "payments.create",
    "payments.create_refund",
    "inventory.reserve",
    "inventory.reservation.confirm",
    "inventory.reservation.release",
    "orders.transition",
    "orders.ship",
    "returns.transition",
    "ledger.post",
    "x402.settle",
    "checkout.commit",
    "subscriptions.charge",
    "a2a.escrow.create",
    "a2a.escrow.dispute",
    "a2a.escrow.fund",
    "a2a.escrow.release",
    "a2a.escrow.refund",
    "a2a.dispute.file",
    "a2a.dispute.evidence.submit",
    "a2a.dispute.resolve",
];

fn policy() -> KernelPolicy {
    ALL_COMMANDS.iter().fold(KernelPolicy::new("commerce-policy-1"), |policy, command| {
        policy.allow(*command, KernelCommandPolicy::requiring([*command]))
    })
}

fn principal(capability: &str) -> KernelPrincipal {
    KernelPrincipal {
        id: "agent:round5".into(),
        kind: PrincipalKind::Agent,
        tenant_id: Some("tenant-1".into()),
        delegated_by: Some("user-1".into()),
        capabilities: vec![capability.into()],
    }
}

fn command<C>(command_type: &str, key: String, payload: C) -> CommandEnvelope<C> {
    let mut command = CommandEnvelope::preview(command_type, key, principal(command_type), payload);
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

fn key(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4())
}

fn payment_command(key: String, amount: Decimal) -> CommandEnvelope<CreatePayment> {
    let mut command = command(
        "payments.create",
        key,
        CreatePayment {
            amount,
            payment_method: PaymentMethodType::CreditCard,
            ..Default::default()
        },
    );
    command.mode = ExecutionMode::Apply;
    command
}

fn transition_command(
    key: String,
    order_id: stateset_core::OrderId,
    status: OrderStatus,
) -> CommandEnvelope<TransitionOrder> {
    let mut command = command(
        "orders.transition",
        key,
        TransitionOrder { order_id, status, payment_status: None, void_payments: false },
    );
    command.mode = ExecutionMode::Apply;
    command
}

async fn order_with_stock(db: &PostgresDatabase, unit_price: Decimal) -> stateset_core::Order {
    let sku = format!("R5-{}", Uuid::new_v4());
    db.inventory()
        .create_item_async(CreateInventoryItem {
            sku: sku.clone(),
            name: sku.clone(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .await
        .expect("create inventory item");
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("round5-{}@example.com", Uuid::new_v4()),
            first_name: "Round".into(),
            last_name: "Five".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    db.orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: sku.clone(),
                name: sku,
                quantity: 1,
                unit_price,
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create order")
}

async fn processing_order(db: &PostgresDatabase, order_id: Uuid) -> stateset_core::Order {
    for status in [OrderStatus::Confirmed, OrderStatus::Processing] {
        db.orders()
            .update_async(
                order_id,
                stateset_core::UpdateOrder { status: Some(status), ..Default::default() },
            )
            .await
            .expect("advance order");
    }
    db.orders().get_async(order_id).await.expect("get").expect("order")
}

async fn payment(db: &PostgresDatabase, order_id: Uuid, amount: Decimal) -> Payment {
    db.payments()
        .create_async(CreatePayment {
            order_id: Some(order_id.into()),
            payment_method: PaymentMethodType::CreditCard,
            amount,
            ..Default::default()
        })
        .await
        .expect("create payment")
}

async fn status(db: &PostgresDatabase, id: Uuid) -> PaymentTransactionStatus {
    db.payments().get_async(id).await.expect("get payment").expect("payment exists").status
}

async fn count(db: &PostgresDatabase, sql: &str, bind: &str) -> i64 {
    sqlx::query_scalar(sql).bind(bind).fetch_one(db.pool()).await.expect("count")
}

// ---------------------------------------------------------------------------
// Finding 3 — the kernel cancel path honours the order money rule
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_kernel_cancel_is_refused_while_captured_money_is_outstanding() {
    let db = require_db!();
    let order = order_with_stock(&db, dec!(100.00)).await;
    let captured = payment(&db, order.id.into_uuid(), dec!(60.00)).await;
    db.payments().mark_completed_async(captured.id.into_uuid()).await.expect("complete");

    let mut preview =
        transition_command(key("r5-cancel-preview"), order.id, OrderStatus::Cancelled);
    preview.mode = ExecutionMode::Preview;
    let previewed = db
        .kernel_executor(policy())
        .execute_transition_order_async(&preview)
        .await
        .expect("preview");
    assert_eq!(previewed.status, ExecutionStatus::Rejected);
    assert_eq!(previewed.error_code.as_deref(), Some("commerce.order.captured_money_outstanding"));

    let cancel = transition_command(key("r5-cancel-apply"), order.id, OrderStatus::Cancelled);
    let receipt = db
        .kernel_executor(policy())
        .execute_transition_order_async(&cancel)
        .await
        .expect("receipt");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("commerce.order.captured_money_outstanding"));
    assert_eq!(receipt.retry, RetryDisposition::Never);
    assert_eq!(receipt.version_before, Some(order.version));
    assert!(receipt.error_message.as_deref().unwrap_or_default().contains("60.00 USD"));
    assert!(receipt.audit_hash.is_some());

    let stored = db.orders().get_async(order.id.into_uuid()).await.expect("get").expect("order");
    assert_eq!(stored.status, OrderStatus::Pending);
    assert_eq!(status(&db, captured.id.into_uuid()).await, PaymentTransactionStatus::Completed);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM inventory_reservations WHERE reference_id = $1 AND status = 'released'",
            &order.id.to_string()
        )
        .await,
        0
    );

    let mut retry = cancel.clone();
    retry.command_id = Uuid::new_v4();
    let replay =
        db.kernel_executor(policy()).execute_transition_order_async(&retry).await.expect("replay");
    assert_eq!(replay.receipt_id, receipt.receipt_id);
}

#[tokio::test]
async fn postgres_kernel_forced_cancel_voids_in_flight_payments_and_reports_settled_money() {
    let db = require_db!();
    let order = order_with_stock(&db, dec!(100.00)).await;
    let settled = payment(&db, order.id.into_uuid(), dec!(60.00)).await;
    db.payments().mark_completed_async(settled.id.into_uuid()).await.expect("complete");
    let in_flight = payment(&db, order.id.into_uuid(), dec!(30.00)).await;

    let mut cancel = transition_command(key("r5-forced-cancel"), order.id, OrderStatus::Cancelled);
    cancel.payload.void_payments = true;
    let receipt = db
        .kernel_executor(policy())
        .execute_transition_order_async(&cancel)
        .await
        .expect("receipt");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{receipt:?}");
    assert_eq!(status(&db, in_flight.id.into_uuid()).await, PaymentTransactionStatus::Cancelled);
    assert_eq!(status(&db, settled.id.into_uuid()).await, PaymentTransactionStatus::Completed);

    let payload: Value = sqlx::query_scalar(
        "SELECT payload FROM kernel_outbox WHERE event_type = 'orders.updated.v1' AND command_id = $1",
    )
    .bind(cancel.command_id)
    .fetch_one(db.pool())
    .await
    .expect("orders.updated.v1");
    assert_eq!(payload["void_payments"], true);
    assert_eq!(payload["voided_payment_ids"], json!([in_flight.id.to_string()]));
    assert_eq!(payload["outstanding_payment_ids"], json!([settled.id.to_string()]));
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM inventory_reservations WHERE reference_id = $1 AND status = 'released'",
            &order.id.to_string()
        )
        .await,
        1
    );
}

// ---------------------------------------------------------------------------
// Finding 1 — a reservation that expires mid-shipment is a sealed rejection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_shipment_whose_reservation_expired_underneath_it_seals_a_rejection() {
    let db = require_db!();
    let order = order_with_stock(&db, dec!(10.00)).await;
    let processing = processing_order(&db, order.id.into_uuid()).await;
    // An expiry sweep marks the hold expired between the kernel's pre-check
    // (which only inspects live holds) and the confirmation loop.
    sqlx::query(
        "UPDATE inventory_reservations SET status = 'expired'
         WHERE reference_type = 'order' AND reference_id = $1",
    )
    .bind(order.id.to_string())
    .execute(db.pool())
    .await
    .expect("expire hold");

    let mut ship = command(
        "orders.ship",
        key("r5-ship-expired"),
        ShipOrderCommand { order_id: order.id, tracking_number: Some("R5".into()), lines: None },
    );
    ship.mode = ExecutionMode::Apply;
    ship.expected_version = Some(processing.version);
    let receipt =
        db.kernel_executor(policy()).execute_ship_order_async(&ship).await.expect("sealed receipt");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("commerce.reservation_expired"));
    assert_eq!(
        receipt.error_message.as_deref(),
        Some("an inventory reservation expired during shipment")
    );
    assert!(receipt.audit_hash.is_some(), "committed, not rolled back");

    let stored = db.orders().get_async(order.id.into_uuid()).await.expect("get").expect("order");
    assert_eq!(stored.status, OrderStatus::Processing, "no shipment applied");
    assert_eq!(stored.items[0].shipped_quantity, 0);
    assert_eq!(stored.version, processing.version);

    let mut retry = ship.clone();
    retry.command_id = Uuid::new_v4();
    let replay =
        db.kernel_executor(policy()).execute_ship_order_async(&retry).await.expect("replay");
    assert_eq!(replay.receipt_id, receipt.receipt_id, "retry key is bound");
}

// ---------------------------------------------------------------------------
// Finding 2 — replay verifies the sealed receipt before trusting it
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_replay_refuses_tampered_or_unsealed_receipts() {
    let db = require_db!();
    let command = payment_command(key("r5-tamper-row"), dec!(12.34));
    let receipt =
        db.kernel_executor(policy()).execute_create_payment_async(&command).await.expect("apply");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded);

    let original: Value =
        sqlx::query_scalar("SELECT receipt FROM kernel_receipts WHERE idempotency_key = $1")
            .bind(&command.idempotency_key)
            .fetch_one(db.pool())
            .await
            .expect("stored receipt");
    sqlx::query(
        "UPDATE kernel_receipts SET receipt = jsonb_set(receipt, '{status}', '\"rejected\"')
         WHERE idempotency_key = $1",
    )
    .bind(&command.idempotency_key)
    .execute(db.pool())
    .await
    .expect("tamper");
    let mut retry = command.clone();
    retry.command_id = Uuid::new_v4();
    let error = db
        .kernel_executor(policy())
        .execute_create_payment_async(&retry)
        .await
        .expect_err("tampered receipt must not replay");
    assert!(matches!(error, CommerceError::KernelReceiptTampered { .. }), "got {error:?}");
    assert!(error.to_string().contains("kernel.receipt_tampered"));
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM payments WHERE idempotency_key = $1",
            &command.idempotency_key
        )
        .await,
        1
    );
    sqlx::query("UPDATE kernel_receipts SET receipt = $1 WHERE idempotency_key = $2")
        .bind(&original)
        .bind(&command.idempotency_key)
        .execute(db.pool())
        .await
        .expect("restore receipt");

    let audit = payment_command(key("r5-tamper-audit"), dec!(5.00));
    db.kernel_executor(policy()).execute_create_payment_async(&audit).await.expect("apply");
    sqlx::query(
        "UPDATE kernel_receipt_audit_log SET request_hash = 'rebound' WHERE idempotency_key = $1",
    )
    .bind(&audit.idempotency_key)
    .execute(db.pool())
    .await
    .expect("tamper audit");
    let mut retry = audit.clone();
    retry.command_id = Uuid::new_v4();
    let error = db
        .kernel_executor(policy())
        .execute_create_payment_async(&retry)
        .await
        .expect_err("rebound audit entry must not replay");
    assert!(matches!(error, CommerceError::KernelReceiptTampered { .. }), "got {error:?}");
    sqlx::query(
        "UPDATE kernel_receipt_audit_log a SET request_hash = r.request_hash FROM kernel_receipts r
         WHERE a.idempotency_key = r.idempotency_key AND a.idempotency_key = $1",
    )
    .bind(&audit.idempotency_key)
    .execute(db.pool())
    .await
    .expect("restore audit entry");

    // An audit entry the receipt cannot find (here: hidden under another hash)
    // is indistinguishable from a receipt that was never sealed.
    let unsealed = payment_command(key("r5-tamper-unsealed"), dec!(5.00));
    let sealed =
        db.kernel_executor(policy()).execute_create_payment_async(&unsealed).await.expect("apply");
    let sealed_hash = sealed.audit_hash.clone().expect("sealed");
    sqlx::query("UPDATE kernel_receipt_audit_log SET audit_hash = $1 WHERE audit_hash = $2")
        .bind(format!("hidden-{sealed_hash}"))
        .bind(&sealed_hash)
        .execute(db.pool())
        .await
        .expect("hide audit entry");
    let mut retry = unsealed.clone();
    retry.command_id = Uuid::new_v4();
    let error = db
        .kernel_executor(policy())
        .execute_create_payment_async(&retry)
        .await
        .expect_err("unsealed receipt must not replay");
    assert!(matches!(error, CommerceError::KernelReceiptTampered { .. }), "got {error:?}");
    sqlx::query("UPDATE kernel_receipt_audit_log SET audit_hash = $1 WHERE audit_hash = $2")
        .bind(&sealed_hash)
        .bind(format!("hidden-{sealed_hash}"))
        .execute(db.pool())
        .await
        .expect("restore audit entry");

    let intact = payment_command(key("r5-intact"), dec!(7.00));
    let first =
        db.kernel_executor(policy()).execute_create_payment_async(&intact).await.expect("apply");
    let mut retry = intact.clone();
    retry.command_id = Uuid::new_v4();
    let replay =
        db.kernel_executor(policy()).execute_create_payment_async(&retry).await.expect("replay");
    assert_eq!(replay.receipt_id, first.receipt_id);
}

// ---------------------------------------------------------------------------
// Ported from SQLite: idempotency conflict, policy denial, version conflict,
// receipt-append failure rollback, key-mismatch guard, envelope guards
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_kernel_rejects_idempotency_key_reuse_for_different_work() {
    let db = require_db!();
    let first = payment_command(key("r5-conflict"), dec!(10.00));
    let first_receipt =
        db.kernel_executor(policy()).execute_create_payment_async(&first).await.expect("first");
    assert_eq!(first_receipt.status, ExecutionStatus::Succeeded);

    let mut conflicting = payment_command(first.idempotency_key.clone(), dec!(11.00));
    conflicting.command_id = Uuid::new_v4();
    let conflict = db
        .kernel_executor(policy())
        .execute_create_payment_async(&conflicting)
        .await
        .expect("conflict receipt");
    assert_eq!(conflict.status, ExecutionStatus::Rejected);
    assert_eq!(conflict.error_code.as_deref(), Some("kernel.idempotency_conflict"));
    assert_eq!(conflict.retry, RetryDisposition::Never);
    assert!(conflict.policy.is_none());
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM payments WHERE idempotency_key = $1",
            &first.idempotency_key
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM kernel_receipts WHERE idempotency_key = $1",
            &first.idempotency_key
        )
        .await,
        1
    );
}

#[tokio::test]
async fn postgres_kernel_policy_denial_is_a_durable_non_mutating_receipt() {
    let db = require_db!();
    let command = payment_command(key("r5-denied"), dec!(10.00));
    let receipt = db
        .kernel_executor(KernelPolicy::new("commerce-policy-1"))
        .execute_create_payment_async(&command)
        .await
        .expect("denial receipt");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.policy_denied"));
    let decision = receipt.policy.expect("policy evidence");
    assert!(!decision.allowed);
    assert!(decision.reason_codes.contains(&"policy.command_not_allowed".to_string()));
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM payments WHERE idempotency_key = $1",
            &command.idempotency_key
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM kernel_outbox WHERE idempotency_key = $1",
            &command.idempotency_key
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM kernel_receipts WHERE idempotency_key = $1",
            &command.idempotency_key
        )
        .await,
        1
    );
}

#[tokio::test]
async fn postgres_kernel_version_conflict_is_durable_and_non_mutating() {
    let db = require_db!();
    let order = order_with_stock(&db, dec!(10.00)).await;
    let mut stale =
        transition_command(key("r5-version-conflict"), order.id, OrderStatus::Confirmed);
    stale.expected_version = Some(order.version + 7);
    let receipt = db
        .kernel_executor(policy())
        .execute_transition_order_async(&stale)
        .await
        .expect("conflict receipt");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.version_conflict"));
    assert_eq!(receipt.retry, RetryDisposition::AfterConflict);
    assert_eq!(receipt.version_before, Some(order.version));
    assert_eq!(receipt.aggregate_id.as_deref(), Some(order.id.to_string().as_str()));
    let stored = db.orders().get_async(order.id.into_uuid()).await.expect("get").expect("order");
    assert_eq!(stored.status, OrderStatus::Pending);
    assert_eq!(stored.version, order.version);
}

#[tokio::test]
async fn postgres_receipt_insert_failure_rolls_back_payment_and_event() {
    let db = require_db!();
    let command = payment_command(key("r5-rollback"), dec!(8.00));
    let seeded_key = key("r5-rollback-other");
    // `kernel_receipts.command_id` is the primary key: a pre-existing row for
    // this command under a different key makes the receipt append fail after
    // the payment and its event have been written.
    sqlx::query(
        "INSERT INTO kernel_receipts (command_id, idempotency_key, command_type, contract_version,
             request_hash, status, receipt, created_at, completed_at)
         VALUES ($1, $2, 'payments.create', '1.0', 'preexisting', 'rejected', '{}', NOW(), NOW())",
    )
    .bind(command.command_id)
    .bind(&seeded_key)
    .execute(db.pool())
    .await
    .expect("seed conflicting command identity");

    let error = db
        .kernel_executor(policy())
        .execute_create_payment_async(&command)
        .await
        .expect_err("receipt append must fail");
    assert!(matches!(error, CommerceError::DatabaseError(_)), "got {error:?}");
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM payments WHERE idempotency_key = $1",
            &command.idempotency_key
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM kernel_outbox WHERE idempotency_key = $1",
            &command.idempotency_key
        )
        .await,
        0
    );
    // The seeded row has no audit entry; remove it so the shared chain stays verifiable.
    sqlx::query("DELETE FROM kernel_receipts WHERE idempotency_key = $1")
        .bind(&seeded_key)
        .execute(db.pool())
        .await
        .expect("cleanup seeded receipt");
}

#[tokio::test]
async fn postgres_payment_key_mismatch_guard_is_a_sealed_rejection_with_policy_evidence() {
    let db = require_db!();
    let mut command = payment_command(key("r5-key-mismatch"), dec!(1.00));
    command.payload.idempotency_key = Some("someone-elses-key".into());
    let receipt =
        db.kernel_executor(policy()).execute_create_payment_async(&command).await.expect("receipt");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.idempotency_key_mismatch"));
    assert_eq!(receipt.retry, RetryDisposition::Never);
    assert_eq!(receipt.aggregate_type.as_deref(), Some("payment"));
    assert!(receipt.policy.expect("policy evidence").allowed);
    assert!(receipt.audit_hash.is_some());

    let mut corrected = command.clone();
    corrected.command_id = Uuid::new_v4();
    corrected.payload.idempotency_key = None;
    let replay = db
        .kernel_executor(policy())
        .execute_create_payment_async(&corrected)
        .await
        .expect("replay");
    assert_eq!(replay.error_code.as_deref(), Some("kernel.idempotency_conflict"));
}

#[tokio::test]
async fn postgres_envelope_guards_reject_type_actor_and_version_misuse() {
    let db = require_db!();
    let mut mismatched = payment_command(key("r5-type-mismatch"), dec!(1.00));
    mismatched.command_type = "orders.transition".into();
    mismatched.principal.capabilities = vec!["orders.transition".into()];
    let receipt = db
        .kernel_executor(policy())
        .execute_create_payment_async(&mismatched)
        .await
        .expect("receipt");
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.command_type_mismatch"));
    assert_eq!(receipt.error_message.as_deref(), Some("expected payments.create command type"));
    assert_eq!(receipt.retry, RetryDisposition::Never);

    let mut self_delegated = payment_command(key("r5-self-delegated"), dec!(1.00));
    self_delegated.principal.delegated_by = Some(self_delegated.principal.id.clone());
    let receipt = db
        .kernel_executor(policy())
        .execute_create_payment_async(&self_delegated)
        .await
        .expect("receipt");
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.actor_mismatch"));

    let mut self_approved = payment_command(key("r5-self-approved"), dec!(1.00));
    self_approved.approval = Some(ApprovalEvidence {
        approval_id: "approval-1".into(),
        approved_by: self_approved.principal.id.clone(),
        scope: "payments.create".into(),
        tenant_id: self_approved.principal.tenant_id.clone(),
        store_id: self_approved.store_id.clone(),
        idempotency_key: Some(self_approved.idempotency_key.clone()),
        approved_at: Utc::now(),
        expires_at: None,
    });
    let receipt = db
        .kernel_executor(policy())
        .execute_create_payment_async(&self_approved)
        .await
        .expect("receipt");
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.actor_mismatch"));

    let mut versioned = payment_command(key("r5-expected-version"), dec!(1.00));
    versioned.expected_version = Some(1);
    let receipt = db
        .kernel_executor(policy())
        .execute_create_payment_async(&versioned)
        .await
        .expect("receipt");
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.expected_version_not_applicable"));

    let order = order_with_stock(&db, dec!(1.00)).await;
    let mut transition =
        transition_command(key("r5-actor-order"), order.id, OrderStatus::Confirmed);
    transition.principal.delegated_by = Some(transition.principal.id.clone());
    let receipt = db
        .kernel_executor(policy())
        .execute_transition_order_async(&transition)
        .await
        .expect("receipt");
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.actor_mismatch"));
    let mut honoured = transition_command(key("r5-version-ok"), order.id, OrderStatus::Confirmed);
    honoured.expected_version = Some(order.version);
    let receipt = db
        .kernel_executor(policy())
        .execute_transition_order_async(&honoured)
        .await
        .expect("receipt");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded);
}

// ---------------------------------------------------------------------------
// Preview envelopes across op kinds
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_preview_envelopes_are_durable_non_mutating_and_replayable_across_ops() {
    let db = require_db!();
    let executor = db.kernel_executor(policy());
    let suffix = Uuid::new_v4();

    macro_rules! preview {
        ($method:ident, $command:expr) => {{
            let command = $command;
            let first = executor.$method(&command).await.expect("preview");
            assert_eq!(first.status, ExecutionStatus::Previewed, "{first:?}");
            assert!(first.audit_hash.is_some());
            assert_eq!(first.retry, RetryDisposition::Never);
            assert!(first.event_ids.is_empty());
            let mut retry = command.clone();
            retry.command_id = Uuid::new_v4();
            let replay = executor.$method(&retry).await.expect("replay preview");
            assert_eq!(replay.receipt_id, first.receipt_id);
            first
        }};
    }

    let sku = format!("R5-PREVIEW-{suffix}");
    preview!(
        execute_create_inventory_item_async,
        command(
            "inventory.item.create",
            key("r5-preview-item"),
            CreateInventoryItem {
                sku: sku.clone(),
                name: "preview".into(),
                initial_quantity: Some(dec!(3)),
                ..Default::default()
            },
        )
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM inventory_items WHERE sku = $1", &sku).await, 0);

    let slug = format!("r5-preview-{suffix}");
    preview!(
        execute_create_product_async,
        command(
            "products.create",
            key("r5-preview-product"),
            CreateProduct {
                name: "Preview".into(),
                slug: Some(slug.clone()),
                ..Default::default()
            },
        )
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM products WHERE slug = $1", &slug).await, 0);

    let mut pay = payment_command(key("r5-preview-payment"), dec!(9.99));
    pay.mode = ExecutionMode::Preview;
    preview!(execute_create_payment_async, pay);

    let order = order_with_stock(&db, dec!(20.00)).await;
    let captured = payment(&db, order.id.into_uuid(), dec!(20.00)).await;
    db.payments().mark_completed_async(captured.id.into_uuid()).await.expect("complete");
    preview!(
        execute_create_refund_async,
        command(
            "payments.create_refund",
            key("r5-preview-refund"),
            CreateRefund {
                payment_id: captured.id,
                amount: Some(dec!(5.00)),
                ..Default::default()
            },
        )
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM refunds WHERE payment_id::text = $1",
            &captured.id.to_string()
        )
        .await,
        0
    );

    let stock_sku = format!("R5-STOCK-{suffix}");
    db.inventory()
        .create_item_async(CreateInventoryItem {
            sku: stock_sku.clone(),
            name: stock_sku.clone(),
            initial_quantity: Some(dec!(5)),
            ..Default::default()
        })
        .await
        .expect("stock");
    preview!(
        execute_reserve_inventory_async,
        command(
            "inventory.reserve",
            key("r5-preview-reserve"),
            ReserveInventory {
                sku: stock_sku.clone(),
                location_id: None,
                quantity: dec!(2),
                reference_type: "preview".into(),
                reference_id: suffix.to_string(),
                expires_in_seconds: None,
            },
        )
    );
    let reservation = db
        .inventory()
        .reserve_async(ReserveInventory {
            sku: stock_sku.clone(),
            location_id: None,
            quantity: dec!(1),
            reference_type: "preview".into(),
            reference_id: format!("{suffix}-direct"),
            expires_in_seconds: None,
        })
        .await
        .expect("direct reservation");
    preview!(
        execute_confirm_inventory_reservation_async,
        command(
            "inventory.reservation.confirm",
            key("r5-preview-confirm"),
            ConfirmInventoryReservation { reservation_id: reservation.id, quantity: None },
        )
    );
    preview!(
        execute_release_inventory_reservation_async,
        command(
            "inventory.reservation.release",
            key("r5-preview-release"),
            ReleaseInventoryReservation { reservation_id: reservation.id },
        )
    );
    let held = db
        .inventory()
        .get_reservation_async(reservation.id)
        .await
        .expect("reservation")
        .expect("exists");
    assert_eq!(held.status, stateset_core::ReservationStatus::Pending);

    let mut transition =
        transition_command(key("r5-preview-transition"), order.id, OrderStatus::Confirmed);
    transition.mode = ExecutionMode::Preview;
    let previewed = preview!(execute_transition_order_async, transition);
    assert_eq!(previewed.version_after, Some(order.version + 1));
    let stored = db.orders().get_async(order.id.into_uuid()).await.expect("get").expect("order");
    assert_eq!(stored.status, OrderStatus::Pending);
    assert_eq!(stored.version, order.version);
    let shippable = order_with_stock(&db, dec!(20.00)).await;
    let processing = processing_order(&db, shippable.id.into_uuid()).await;
    preview!(
        execute_ship_order_async,
        command(
            "orders.ship",
            key("r5-preview-ship"),
            ShipOrderCommand { order_id: shippable.id, tracking_number: None, lines: None },
        )
    );
    let stored =
        db.orders().get_async(shippable.id.into_uuid()).await.expect("get").expect("order");
    assert_eq!(stored.status, OrderStatus::Processing);
    assert_eq!(stored.version, processing.version);
    assert_eq!(stored.items[0].shipped_quantity, 0);

    let expires_at = Utc::now() + Duration::hours(1);
    let escrow_input = CreateA2AEscrow {
        quote_id: None,
        payment_id: None,
        buyer_address: "0xbuyer".into(),
        seller_address: "0xseller".into(),
        amount: dec!(25),
        asset: "usdc".into(),
        network: "base".into(),
        release_conditions: vec![],
        expires_at,
        auto_release_after: None,
        metadata: None,
    };
    preview!(
        execute_create_a2a_escrow_async,
        command("a2a.escrow.create", key("r5-preview-escrow"), escrow_input.clone())
    );
    let mut create_escrow = command("a2a.escrow.create", key("r5-escrow-live"), escrow_input);
    create_escrow.mode = ExecutionMode::Apply;
    let escrow = executor
        .execute_create_a2a_escrow_async(&create_escrow)
        .await
        .expect("create escrow")
        .result
        .expect("escrow");
    preview!(
        execute_fund_a2a_escrow_async,
        command(
            "a2a.escrow.fund",
            key("r5-preview-fund"),
            FundA2AEscrow { escrow_id: escrow.id.clone() }
        )
    );
    let escrow_status: String = sqlx::query_scalar("SELECT status FROM a2a_escrows WHERE id = $1")
        .bind(&escrow.id)
        .fetch_one(db.pool())
        .await
        .expect("escrow status");
    assert_eq!(escrow_status, "created", "preview never funded the escrow");
}

// ---------------------------------------------------------------------------
// Cross-backend receipt shape: same command → structurally identical receipt
// ---------------------------------------------------------------------------

fn normalize<T: Serialize>(receipt: &ExecutionReceipt<T>) -> Value {
    let mut value = serde_json::to_value(receipt).expect("serialize receipt");
    let object = value.as_object_mut().expect("object");
    for volatile in ["receipt_id", "command_id", "started_at", "completed_at", "audit_hash"] {
        object.remove(volatile);
    }
    if let Some(policy) = object.get_mut("policy").and_then(Value::as_object_mut) {
        policy.remove("decision_id");
    }
    value
}

#[tokio::test]
async fn postgres_and_sqlite_seal_structurally_identical_receipts_for_every_op_kind() {
    let pg = require_db!();
    let lite = SqliteDatabase::in_memory().expect("sqlite");
    let pg_exec = pg.kernel_executor(policy());
    let lite_exec = lite.kernel_executor(policy());
    let past = Utc::now() - Duration::minutes(1);
    let future = Utc::now() + Duration::hours(1);

    // Every op kind is driven through the shared envelope guard with an
    // elapsed deadline, so the receipt is sealed before any backend SQL runs;
    // the two backends must then agree on every non-volatile field.
    macro_rules! shape {
        ($lite:ident, $pg:ident, $type:literal, $payload:expr) => {{
            let mut command =
                command($type, format!("r5-shape-{}-{}", $type, Uuid::new_v4()), $payload);
            command.mode = ExecutionMode::Apply;
            command.deadline = Some(past);
            let sqlite_receipt = lite_exec.$lite(&command).expect("sqlite receipt");
            let pg_receipt = pg_exec.$pg(&command).await.expect("pg receipt");
            assert_eq!(sqlite_receipt.error_code.as_deref(), Some("kernel.deadline_exceeded"));
            assert_eq!(normalize(&sqlite_receipt), normalize(&pg_receipt), "{}", $type);
        }};
    }

    shape!(
        execute_create_inventory_item,
        execute_create_inventory_item_async,
        "inventory.item.create",
        CreateInventoryItem { sku: "S".into(), name: "n".into(), ..Default::default() }
    );
    shape!(
        execute_create_product,
        execute_create_product_async,
        "products.create",
        CreateProduct { name: "P".into(), ..Default::default() }
    );
    shape!(
        execute_create_payment,
        execute_create_payment_async,
        "payments.create",
        CreatePayment {
            amount: dec!(1),
            payment_method: PaymentMethodType::CreditCard,
            ..Default::default()
        }
    );
    shape!(
        execute_create_refund,
        execute_create_refund_async,
        "payments.create_refund",
        CreateRefund { payment_id: stateset_core::PaymentId::new(), ..Default::default() }
    );
    shape!(
        execute_reserve_inventory,
        execute_reserve_inventory_async,
        "inventory.reserve",
        ReserveInventory {
            sku: "S".into(),
            location_id: None,
            quantity: dec!(1),
            reference_type: "t".into(),
            reference_id: "r".into(),
            expires_in_seconds: None
        }
    );
    shape!(
        execute_confirm_inventory_reservation,
        execute_confirm_inventory_reservation_async,
        "inventory.reservation.confirm",
        ConfirmInventoryReservation { reservation_id: Uuid::new_v4(), quantity: None }
    );
    shape!(
        execute_release_inventory_reservation,
        execute_release_inventory_reservation_async,
        "inventory.reservation.release",
        ReleaseInventoryReservation { reservation_id: Uuid::new_v4() }
    );
    shape!(
        execute_transition_order,
        execute_transition_order_async,
        "orders.transition",
        TransitionOrder {
            order_id: stateset_core::OrderId::new(),
            status: OrderStatus::Confirmed,
            payment_status: None,
            void_payments: false
        }
    );
    shape!(
        execute_ship_order,
        execute_ship_order_async,
        "orders.ship",
        ShipOrderCommand {
            order_id: stateset_core::OrderId::new(),
            tracking_number: None,
            lines: None
        }
    );
    shape!(
        execute_transition_return,
        execute_transition_return_async,
        "returns.transition",
        TransitionReturn {
            return_id: stateset_core::ReturnId::new(),
            status: ReturnStatus::Approved
        }
    );
    shape!(
        execute_post_journal_entry,
        execute_post_journal_entry_async,
        "ledger.post",
        PostJournalEntry { journal_entry_id: Uuid::new_v4(), posted_by: "u".into() }
    );
    shape!(
        execute_settle_x402_intent,
        execute_settle_x402_intent_async,
        "x402.settle",
        SettleX402Intent { intent_id: Uuid::new_v4(), tx_hash: "0xabc".into(), block_number: 1 }
    );
    shape!(
        execute_commit_checkout,
        execute_commit_checkout_async,
        "checkout.commit",
        CommitCheckout { cart_id: stateset_core::CartId::new() }
    );
    shape!(
        execute_charge_subscription,
        execute_charge_subscription_async,
        "subscriptions.charge",
        ChargeSubscription {
            billing_cycle_id: Uuid::new_v4(),
            payment_method: PaymentMethodType::CreditCard,
            processor: None
        }
    );
    shape!(
        execute_create_a2a_escrow,
        execute_create_a2a_escrow_async,
        "a2a.escrow.create",
        CreateA2AEscrow {
            quote_id: None,
            payment_id: None,
            buyer_address: "b".into(),
            seller_address: "s".into(),
            amount: dec!(1),
            asset: "USDC".into(),
            network: "base".into(),
            release_conditions: vec![],
            expires_at: future,
            auto_release_after: None,
            metadata: None
        }
    );
    shape!(
        execute_dispute_a2a_escrow,
        execute_dispute_a2a_escrow_async,
        "a2a.escrow.dispute",
        DisputeA2AEscrow { escrow_id: "e".into(), reason: "r".into(), category: None }
    );
    shape!(
        execute_fund_a2a_escrow,
        execute_fund_a2a_escrow_async,
        "a2a.escrow.fund",
        FundA2AEscrow { escrow_id: "e".into() }
    );
    shape!(
        execute_release_a2a_escrow,
        execute_release_a2a_escrow_async,
        "a2a.escrow.release",
        ReleaseA2AEscrow { escrow_id: "e".into() }
    );
    shape!(
        execute_refund_a2a_escrow,
        execute_refund_a2a_escrow_async,
        "a2a.escrow.refund",
        RefundA2AEscrow { escrow_id: "e".into(), reason: None }
    );
    shape!(
        execute_file_a2a_dispute,
        execute_file_a2a_dispute_async,
        "a2a.dispute.file",
        FileA2ADispute {
            escrow_id: "e".into(),
            claimant_address: "b".into(),
            reason: "r".into(),
            category: "c".into(),
            evidence_deadline: future,
            review_deadline: future,
            metadata: None
        }
    );
    shape!(
        execute_submit_a2a_dispute_evidence,
        execute_submit_a2a_dispute_evidence_async,
        "a2a.dispute.evidence.submit",
        SubmitA2ADisputeEvidence {
            dispute_id: "d".into(),
            submitted_by: "b".into(),
            evidence_type: "t".into(),
            title: "t".into(),
            description: None,
            content: "c".into()
        }
    );
    shape!(
        execute_resolve_a2a_dispute,
        execute_resolve_a2a_dispute_async,
        "a2a.dispute.resolve",
        ResolveA2ADispute {
            dispute_id: "d".into(),
            resolution_type: A2ADisputeResolutionType::FullRefund,
            buyer_amount: None,
            seller_amount: None,
            note: None
        }
    );

    // A committed success shape must agree too.
    let pg_order = order_with_stock(&pg, dec!(10.00)).await;
    let mut pg_confirm =
        transition_command(key("r5-shape-success"), pg_order.id, OrderStatus::Confirmed);
    pg_confirm.expected_version = Some(pg_order.version);
    let pg_receipt = pg_exec.execute_transition_order_async(&pg_confirm).await.expect("pg apply");
    let lite_sku = "R5-SHAPE";
    lite.inventory()
        .create_item(CreateInventoryItem {
            sku: lite_sku.into(),
            name: lite_sku.into(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .expect("stock");
    let lite_customer = lite
        .customers()
        .create(CreateCustomer {
            email: "shape@example.com".into(),
            first_name: "Shape".into(),
            last_name: "Test".into(),
            ..Default::default()
        })
        .expect("customer");
    let lite_order = lite
        .orders()
        .create(CreateOrder {
            customer_id: lite_customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: lite_sku.into(),
                name: lite_sku.into(),
                quantity: 1,
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("order");
    let mut lite_confirm = pg_confirm.clone();
    lite_confirm.payload.order_id = lite_order.id;
    lite_confirm.expected_version = Some(lite_order.version);
    let lite_receipt = lite_exec.execute_transition_order(&lite_confirm).expect("sqlite apply");
    let strip = |receipt: &ExecutionReceipt<stateset_core::Order>| {
        let mut value = normalize(receipt);
        let object = value.as_object_mut().expect("object");
        object.remove("result");
        object.remove("aggregate_id");
        object.remove("event_ids");
        value
    };
    assert_eq!(strip(&lite_receipt), strip(&pg_receipt));
    assert_eq!(lite_receipt.event_ids.len(), pg_receipt.event_ids.len());
}
