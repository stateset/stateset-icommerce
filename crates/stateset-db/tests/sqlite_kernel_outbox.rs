#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use sha2::{Digest, Sha256};
use stateset_core::{
    A2ADisputeResolutionType, AccountType, AddCartItem, BillingCycleFilter, BillingCycleStatus,
    BillingInterval, CartAddress, CartRepository, ChargeSubscription, CommandEnvelope,
    CommerceError, CommitCheckout, ConfirmInventoryReservation, CreateA2AEscrow, CreateCart,
    CreateCustomer, CreateGlAccount, CreateGlPeriod, CreateInventoryItem, CreateJournalEntry,
    CreateJournalEntryLine, CreateOrder, CreateOrderItem, CreatePayment, CreateProduct,
    CreateProductVariant, CreateRefund, CreateReturn, CreateReturnItem, CreateSubscription,
    CreateSubscriptionPlan, CreateX402PaymentIntent, CurrencyCode, CustomerRepository,
    DisputeA2AEscrow, EconomicCommitment, ExecutionMode, ExecutionStatus, FileA2ADispute,
    FundA2AEscrow, GeneralLedgerRepository, InventoryRepository, JournalEntryStatus,
    KernelCommandPolicy, KernelPolicy, KernelPrincipal, OrderRepository, OrderStatus,
    PaymentMethodType, PaymentRepository, PostJournalEntry, PrincipalKind, ProductId,
    ProductRepository, RefundA2AEscrow, ReleaseA2AEscrow, ReleaseInventoryReservation,
    ReservationStatus, ReserveInventory, ResolveA2ADispute, ReturnRepository, ReturnStatus,
    SetCartPayment, SettleX402Intent, ShipOrderCommand, ShipmentLineInput,
    SubmitA2ADisputeEvidence, TransitionOrder, TransitionReturn, UpdateOrder, X402Asset,
    X402IntentStatus, X402Network, X402PaymentIntentRepository,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

fn reseal_checkpoint(checkpoint: &mut stateset_db::kernel_outbox::KernelAuditCheckpoint) {
    let preimage = serde_json::json!({
        "contract_version": checkpoint.contract_version,
        "algorithm": checkpoint.algorithm,
        "entries": checkpoint.entries,
        "head_hash": checkpoint.head_hash,
        "generated_at": checkpoint.generated_at,
    });
    let canonical = serde_jcs::to_vec(&preimage).expect("canonical checkpoint");
    checkpoint.checkpoint_hash = format!("{:x}", Sha256::digest(canonical));
}

fn payment_policy() -> KernelPolicy {
    KernelPolicy::new("commerce-policy-1")
        .allow("products.create", KernelCommandPolicy::requiring(["products.create"]))
        .allow("inventory.item.create", KernelCommandPolicy::requiring(["inventory.item.create"]))
        .allow("payments.create", KernelCommandPolicy::requiring(["payments.create"]))
        .allow("payments.create_refund", KernelCommandPolicy::requiring(["payments.create_refund"]))
        .allow("inventory.reserve", KernelCommandPolicy::requiring(["inventory.reserve"]))
        .allow(
            "inventory.reservation.confirm",
            KernelCommandPolicy::requiring(["inventory.reservation.confirm"]),
        )
        .allow(
            "inventory.reservation.release",
            KernelCommandPolicy::requiring(["inventory.reservation.release"]),
        )
        .allow("orders.transition", KernelCommandPolicy::requiring(["orders.transition"]))
        .allow("orders.ship", KernelCommandPolicy::requiring(["orders.ship"]))
        .allow("returns.transition", KernelCommandPolicy::requiring(["returns.transition"]))
        .allow("ledger.post", KernelCommandPolicy::requiring(["ledger.post"]))
        .allow("x402.settle", KernelCommandPolicy::requiring(["x402.settle"]))
        .allow("checkout.commit", KernelCommandPolicy::requiring(["checkout.commit"]))
        .allow("subscriptions.charge", KernelCommandPolicy::requiring(["subscriptions.charge"]))
        .allow("a2a.escrow.create", KernelCommandPolicy::requiring(["a2a.escrow.create"]))
        .allow("a2a.escrow.dispute", KernelCommandPolicy::requiring(["a2a.escrow.dispute"]))
        .allow("a2a.escrow.fund", KernelCommandPolicy::requiring(["a2a.escrow.fund"]))
        .allow("a2a.escrow.release", KernelCommandPolicy::requiring(["a2a.escrow.release"]))
        .allow("a2a.escrow.refund", KernelCommandPolicy::requiring(["a2a.escrow.refund"]))
        .allow("a2a.dispute.file", KernelCommandPolicy::requiring(["a2a.dispute.file"]))
        .allow(
            "a2a.dispute.evidence.submit",
            KernelCommandPolicy::requiring(["a2a.dispute.evidence.submit"]),
        )
        .allow("a2a.dispute.resolve", KernelCommandPolicy::requiring(["a2a.dispute.resolve"]))
}

fn inventory_item_command(key: &str, sku: &str) -> CommandEnvelope<CreateInventoryItem> {
    let mut command = CommandEnvelope::preview(
        "inventory.item.create",
        key,
        KernelPrincipal {
            id: "agent:inventory-1".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-1".into()),
            capabilities: vec!["inventory.item.create".into()],
        },
        CreateInventoryItem {
            sku: sku.into(),
            name: "Fractional autonomous inventory".into(),
            initial_quantity: Some(dec!(9007199254740993.125)),
            reorder_point: Some(dec!(0.125)),
            safety_stock: Some(dec!(0.025)),
            ..Default::default()
        },
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

#[test]
fn kernel_inventory_item_create_is_exact_atomic_previewable_and_replayable() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let sku = format!("INV-{}", Uuid::new_v4());
    let preview = inventory_item_command("kernel-inventory-create-1", &sku);
    let previewed = db
        .kernel_executor(payment_policy())
        .execute_create_inventory_item(&preview)
        .expect("preview inventory item");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    assert!(db.inventory().get_item_by_sku(&sku).expect("query preview").is_none());

    let mut apply = preview;
    apply.command_id = Uuid::new_v4();
    apply.mode = ExecutionMode::Apply;
    let applied = db
        .kernel_executor(payment_policy())
        .execute_create_inventory_item(&apply)
        .expect("apply inventory item");
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    assert_eq!(applied.event_ids.len(), 1);
    let stock = db.inventory().get_stock(&sku).expect("load exact stock").expect("stock exists");
    assert_eq!(stock.total_on_hand, dec!(9007199254740993.125));
    assert_eq!(stock.total_available, dec!(9007199254740993.125));
    let transaction_quantity: String = db
        .pool()
        .get()
        .expect("connection")
        .query_row(
            "SELECT quantity FROM inventory_transactions WHERE item_id = ?",
            [applied.result.as_ref().expect("item").id],
            |row| row.get(0),
        )
        .expect("initial stock transaction");
    assert_eq!(transaction_quantity, "9007199254740993.125");

    let mut replay_command = apply;
    replay_command.command_id = Uuid::new_v4();
    let replay = db
        .kernel_executor(payment_policy())
        .execute_create_inventory_item(&replay_command)
        .expect("replay inventory item");
    assert_eq!(replay.receipt_id, applied.receipt_id);

    let mut conflict = inventory_item_command("kernel-inventory-create-conflict", &sku);
    conflict.mode = ExecutionMode::Apply;
    let rejected = db
        .kernel_executor(payment_policy())
        .execute_create_inventory_item(&conflict)
        .expect("durable SKU rejection");
    assert_eq!(rejected.status, ExecutionStatus::Rejected);
    assert_eq!(rejected.error_code.as_deref(), Some("commerce.inventory.sku_conflict"));
    assert!(rejected.audit_hash.is_some());
}

fn product_command(key: &str, sku: &str, slug: &str) -> CommandEnvelope<CreateProduct> {
    let mut command = CommandEnvelope::preview(
        "products.create",
        key,
        KernelPrincipal {
            id: "agent:catalog-1".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-1".into()),
            capabilities: vec!["products.create".into()],
        },
        CreateProduct {
            name: "Autonomous Offer".into(),
            slug: Some(slug.into()),
            description: Some("An exact-money offer launched by a delegated agent".into()),
            variants: Some(vec![CreateProductVariant {
                sku: sku.into(),
                name: Some("Default".into()),
                price: dec!(9007199254740993.25),
                compare_at_price: Some(dec!(9007199254740994.25)),
                is_default: Some(true),
                ..Default::default()
            }]),
            ..Default::default()
        },
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

#[test]
fn kernel_product_create_is_exact_atomic_previewable_and_replayable() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let suffix = Uuid::new_v4();
    let sku = format!("AGENT-{suffix}");
    let slug = format!("autonomous-offer-{suffix}");
    let preview = product_command("kernel-product-create-1", &sku, &slug);
    let previewed = db
        .kernel_executor(payment_policy())
        .execute_create_product(&preview)
        .expect("preview product");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    assert_eq!(db.products().count(Default::default()).expect("count products"), 0);

    let mut apply = preview;
    apply.command_id = Uuid::new_v4();
    apply.mode = ExecutionMode::Apply;
    let applied =
        db.kernel_executor(payment_policy()).execute_create_product(&apply).expect("apply product");
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    assert_eq!(applied.event_ids.len(), 1);
    assert!(applied.audit_hash.is_some());
    let product = applied.result.as_ref().expect("product result");
    assert_eq!(product.slug, slug);
    let variant =
        db.products().get_variant_by_sku(&sku).expect("load variant").expect("variant exists");
    assert_eq!(variant.price, dec!(9007199254740993.25));

    let mut replay_command = apply;
    replay_command.command_id = Uuid::new_v4();
    let replay = db
        .kernel_executor(payment_policy())
        .execute_create_product(&replay_command)
        .expect("replay product");
    assert_eq!(replay.receipt_id, applied.receipt_id);

    let mut conflict =
        product_command("kernel-product-create-conflict", &format!("OTHER-{suffix}"), &slug);
    conflict.mode = ExecutionMode::Apply;
    let rejected = db
        .kernel_executor(payment_policy())
        .execute_create_product(&conflict)
        .expect("durable slug rejection");
    assert_eq!(rejected.status, ExecutionStatus::Rejected);
    assert_eq!(rejected.error_code.as_deref(), Some("commerce.product.slug_conflict"));
    assert!(rejected.audit_hash.is_some());
}

fn payment_command(key: &str, amount: rust_decimal::Decimal) -> CommandEnvelope<CreatePayment> {
    let mut command = CommandEnvelope::preview(
        "payments.create",
        key,
        KernelPrincipal {
            id: "agent:checkout-1".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-1".into()),
            capabilities: vec!["payments.create".into()],
        },
        CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount,
            currency: Some(CurrencyCode::USD),
            ..Default::default()
        },
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

fn refund_command(
    key: &str,
    payment_id: stateset_core::PaymentId,
    amount: rust_decimal::Decimal,
) -> CommandEnvelope<CreateRefund> {
    let mut command = CommandEnvelope::preview(
        "payments.create_refund",
        key,
        KernelPrincipal {
            id: "agent:refunds-1".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-1".into()),
            capabilities: vec!["payments.create_refund".into()],
        },
        CreateRefund {
            payment_id,
            amount: Some(amount),
            reason: Some("requested by customer".into()),
            ..Default::default()
        },
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

fn completed_payment(db: &SqliteDatabase, amount: rust_decimal::Decimal) -> stateset_core::Payment {
    let payment = db
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount,
            currency: Some(CurrencyCode::USD),
            ..Default::default()
        })
        .expect("create payment");
    db.payments().mark_completed(payment.id).expect("complete payment")
}

#[test]
fn kernel_refund_preview_promotes_applies_and_replays_exactly_once() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let payment = completed_payment(&db, dec!(9007199254740993.25));
    let preview = refund_command("kernel-refund-1", payment.id, dec!(0.25));
    let preview_receipt = db
        .kernel_executor(payment_policy())
        .execute_create_refund(&preview)
        .expect("preview refund");
    assert_eq!(preview_receipt.status, ExecutionStatus::Previewed);

    let mut apply = preview;
    apply.command_id = Uuid::new_v4();
    apply.mode = ExecutionMode::Apply;
    let applied =
        db.kernel_executor(payment_policy()).execute_create_refund(&apply).expect("apply refund");
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    let refund = applied.result.as_ref().expect("refund result");
    assert_eq!(refund.amount, dec!(0.25));
    assert_eq!(applied.event_ids.len(), 1);

    let mut retry = apply.clone();
    retry.command_id = Uuid::new_v4();
    let replay =
        db.kernel_executor(payment_policy()).execute_create_refund(&retry).expect("replay refund");
    assert_eq!(replay.receipt_id, applied.receipt_id);
    assert_eq!(replay.result.expect("replayed refund").id, refund.id);

    let refund_events: Vec<_> = db
        .kernel_outbox()
        .pending(20)
        .expect("events")
        .into_iter()
        .filter(|event| event.event_type == "payments.refund_created.v1")
        .collect();
    assert_eq!(refund_events.len(), 1);
    assert_eq!(refund_events[0].command_id, Some(apply.command_id));
    assert_eq!(refund_events[0].payload["amount"], "0.25");
}

#[test]
fn kernel_refund_overage_and_changed_intent_are_durable_rejections() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let payment = completed_payment(&db, dec!(10.00));
    let mut valid = refund_command("kernel-refund-conflict", payment.id, dec!(4.00));
    valid.mode = ExecutionMode::Apply;
    let applied =
        db.kernel_executor(payment_policy()).execute_create_refund(&valid).expect("valid refund");
    assert_eq!(applied.status, ExecutionStatus::Succeeded);

    let mut changed = refund_command("kernel-refund-conflict", payment.id, dec!(5.00));
    changed.mode = ExecutionMode::Apply;
    let conflict =
        db.kernel_executor(payment_policy()).execute_create_refund(&changed).expect("conflict");
    assert_eq!(conflict.error_code.as_deref(), Some("kernel.idempotency_conflict"));

    let mut excessive = refund_command("kernel-refund-overage", payment.id, dec!(7.00));
    excessive.mode = ExecutionMode::Apply;
    let rejected = db
        .kernel_executor(payment_policy())
        .execute_create_refund(&excessive)
        .expect("overage receipt");
    assert_eq!(rejected.status, ExecutionStatus::Rejected);
    assert_eq!(rejected.error_code.as_deref(), Some("commerce.refund.exceeds_captured"));
    assert!(
        db.kernel_outbox()
            .receipt_by_idempotency_key("kernel-refund-overage")
            .expect("receipt lookup")
            .is_some()
    );

    let conn = db.pool().get().expect("connection");
    let refunds: i64 =
        conn.query_row("SELECT COUNT(*) FROM refunds", [], |row| row.get(0)).unwrap();
    let refund_events: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM kernel_outbox WHERE event_type = 'payments.refund_created.v1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((refunds, refund_events), (1, 1));
}

#[test]
fn concurrent_kernel_refunds_cannot_reserve_beyond_captured_amount() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let payment = completed_payment(&db, dec!(10.00));
    let executor = db.kernel_executor(payment_policy());
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let handles: Vec<_> = ["kernel-concurrent-refund-1", "kernel-concurrent-refund-2"]
        .into_iter()
        .map(|key| {
            let executor = executor.clone();
            let barrier = barrier.clone();
            let mut command = refund_command(key, payment.id, dec!(6.00));
            command.mode = ExecutionMode::Apply;
            std::thread::spawn(move || {
                barrier.wait();
                executor.execute_create_refund(&command).expect("kernel receipt")
            })
        })
        .collect();
    barrier.wait();
    let receipts: Vec<_> =
        handles.into_iter().map(|handle| handle.join().expect("thread")).collect();
    assert_eq!(
        receipts.iter().filter(|receipt| receipt.status == ExecutionStatus::Succeeded).count(),
        1
    );
    assert_eq!(
        receipts.iter().filter(|receipt| receipt.status == ExecutionStatus::Rejected).count(),
        1
    );
    assert!(receipts.iter().any(|receipt| {
        receipt.error_code.as_deref() == Some("commerce.refund.exceeds_captured")
    }));

    let conn = db.pool().get().expect("connection");
    let refunds: i64 =
        conn.query_row("SELECT COUNT(*) FROM refunds", [], |row| row.get(0)).unwrap();
    let receipts: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM kernel_receipts WHERE command_type = 'payments.create_refund'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((refunds, receipts), (1, 2));
}

#[test]
fn refund_receipt_failure_rolls_back_refund_and_event() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let payment = completed_payment(&db, dec!(10.00));
    let mut command = refund_command("kernel-refund-rollback", payment.id, dec!(2.00));
    command.mode = ExecutionMode::Apply;
    let conn = db.pool().get().expect("connection");
    conn.execute(
        "INSERT INTO kernel_receipts (
            command_id, idempotency_key, command_type, contract_version,
            request_hash, status, receipt, created_at, completed_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        rusqlite::params![
            command.command_id.to_string(),
            "different-refund-key",
            "payments.create_refund",
            "1.0",
            "preexisting",
            "rejected",
            "{}",
            chrono::Utc::now().to_rfc3339(),
            chrono::Utc::now().to_rfc3339(),
        ],
    )
    .expect("seed conflicting command identity");
    drop(conn);

    assert!(db.kernel_executor(payment_policy()).execute_create_refund(&command).is_err());
    let conn = db.pool().get().expect("connection");
    let refunds: i64 =
        conn.query_row("SELECT COUNT(*) FROM refunds", [], |row| row.get(0)).unwrap();
    let refund_events: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM kernel_outbox WHERE event_type = 'payments.refund_created.v1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((refunds, refund_events), (0, 0));
}

#[test]
fn kernel_preview_is_durable_idempotent_and_non_mutating() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let command = payment_command("kernel-preview-1", dec!(12.34));
    let first =
        db.kernel_executor(payment_policy()).execute_create_payment(&command).expect("preview");
    assert_eq!(first.status, ExecutionStatus::Previewed);
    assert!(first.result.is_none());

    let mut retry = command;
    retry.command_id = Uuid::new_v4();
    let replay =
        db.kernel_executor(payment_policy()).execute_create_payment(&retry).expect("replay");
    assert_eq!(replay.receipt_id, first.receipt_id);
    assert_eq!(replay.command_id, first.command_id);

    let conn = db.pool().get().expect("connection");
    let payments: i64 =
        conn.query_row("SELECT COUNT(*) FROM payments", [], |row| row.get(0)).unwrap();
    let events: i64 =
        conn.query_row("SELECT COUNT(*) FROM kernel_outbox", [], |row| row.get(0)).unwrap();
    let receipts: i64 =
        conn.query_row("SELECT COUNT(*) FROM kernel_receipts", [], |row| row.get(0)).unwrap();
    assert_eq!((payments, events, receipts), (0, 0, 1));
}

#[test]
fn kernel_apply_promotes_the_same_intent_after_preview() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let preview = payment_command("kernel-promote-1", dec!(14.25));
    let preview_receipt =
        db.kernel_executor(payment_policy()).execute_create_payment(&preview).expect("preview");
    assert_eq!(preview_receipt.status, ExecutionStatus::Previewed);

    let mut apply = preview;
    apply.command_id = Uuid::new_v4();
    apply.mode = ExecutionMode::Apply;
    let applied = db
        .kernel_executor(payment_policy())
        .execute_create_payment(&apply)
        .expect("apply promotion");
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    assert_eq!(applied.command_id, apply.command_id);
    assert_ne!(applied.receipt_id, preview_receipt.receipt_id);

    let stored = db
        .kernel_outbox()
        .receipt_by_idempotency_key("kernel-promote-1")
        .expect("query")
        .expect("stored receipt");
    assert_eq!(stored.command_id, apply.command_id);
    assert_eq!(stored.status, "succeeded");
    let conn = db.pool().get().expect("connection");
    let counts: (i64, i64, i64) = (
        conn.query_row("SELECT COUNT(*) FROM payments", [], |row| row.get(0)).unwrap(),
        conn.query_row("SELECT COUNT(*) FROM kernel_outbox", [], |row| row.get(0)).unwrap(),
        conn.query_row("SELECT COUNT(*) FROM kernel_receipts", [], |row| row.get(0)).unwrap(),
    );
    assert_eq!(counts, (1, 1, 1));
}

#[test]
fn kernel_apply_atomically_commits_payment_event_and_receipt() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let mut command = payment_command("kernel-apply-1", dec!(9007199254740993.25));
    command.mode = ExecutionMode::Apply;
    command.correlation_id = Some(Uuid::new_v4());
    let receipt =
        db.kernel_executor(payment_policy()).execute_create_payment(&command).expect("apply");

    assert_eq!(receipt.status, ExecutionStatus::Succeeded);
    assert_eq!(receipt.result.as_ref().expect("payment").amount, dec!(9007199254740993.25));
    assert_eq!(receipt.event_ids.len(), 1);
    let event = db.kernel_outbox().pending(10).expect("events").remove(0);
    assert_eq!(event.id, receipt.event_ids[0]);
    assert_eq!(event.command_id, Some(command.command_id));
    assert_eq!(event.principal_id.as_deref(), Some("agent:checkout-1"));
    assert_eq!(event.correlation_id, command.correlation_id);
    assert_eq!(event.payload["amount"], "9007199254740993.25");

    let stored = db
        .kernel_outbox()
        .receipt_by_idempotency_key("kernel-apply-1")
        .expect("receipt query")
        .expect("stored receipt");
    assert_eq!(stored.command_id, command.command_id);
    assert_eq!(stored.request_hash.len(), 64);
}

#[test]
fn kernel_rejects_idempotency_key_reuse_for_different_work() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let mut first = payment_command("kernel-conflict-1", dec!(10.00));
    first.mode = ExecutionMode::Apply;
    let first_receipt =
        db.kernel_executor(payment_policy()).execute_create_payment(&first).expect("first apply");
    assert_eq!(first_receipt.status, ExecutionStatus::Succeeded);

    let mut conflicting = payment_command("kernel-conflict-1", dec!(11.00));
    conflicting.mode = ExecutionMode::Apply;
    let conflict = db
        .kernel_executor(payment_policy())
        .execute_create_payment(&conflicting)
        .expect("conflict receipt");
    assert_eq!(conflict.status, ExecutionStatus::Rejected);
    assert_eq!(conflict.error_code.as_deref(), Some("kernel.idempotency_conflict"));

    let conn = db.pool().get().expect("connection");
    let payments: i64 =
        conn.query_row("SELECT COUNT(*) FROM payments", [], |row| row.get(0)).unwrap();
    let receipts: i64 =
        conn.query_row("SELECT COUNT(*) FROM kernel_receipts", [], |row| row.get(0)).unwrap();
    assert_eq!((payments, receipts), (1, 1));
}

#[test]
fn kernel_policy_denial_is_a_durable_non_mutating_receipt() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let command = payment_command("kernel-denied-1", dec!(10.00));
    let receipt = db
        .kernel_executor(KernelPolicy::new("commerce-policy-1"))
        .execute_create_payment(&command)
        .expect("denial receipt");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.policy_denied"));
    let decision = receipt.policy.expect("policy evidence");
    assert!(!decision.allowed);
    assert!(decision.reason_codes.contains(&"policy.command_not_allowed".to_string()));

    let conn = db.pool().get().expect("connection");
    let payments: i64 =
        conn.query_row("SELECT COUNT(*) FROM payments", [], |row| row.get(0)).unwrap();
    let events: i64 =
        conn.query_row("SELECT COUNT(*) FROM kernel_outbox", [], |row| row.get(0)).unwrap();
    let receipts: i64 =
        conn.query_row("SELECT COUNT(*) FROM kernel_receipts", [], |row| row.get(0)).unwrap();
    assert_eq!((payments, events, receipts), (0, 0, 1));
}

#[test]
fn receipt_insert_failure_rolls_back_payment_and_event() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let mut command = payment_command("kernel-rollback-new", dec!(8.00));
    command.mode = ExecutionMode::Apply;
    let conn = db.pool().get().expect("connection");
    conn.execute(
        "INSERT INTO kernel_receipts (
            command_id, idempotency_key, command_type, contract_version,
            request_hash, status, receipt, created_at, completed_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        rusqlite::params![
            command.command_id.to_string(),
            "different-key",
            "payments.create",
            "1.0",
            "preexisting",
            "rejected",
            "{}",
            chrono::Utc::now().to_rfc3339(),
            chrono::Utc::now().to_rfc3339(),
        ],
    )
    .expect("seed conflicting receipt identity");
    drop(conn);

    assert!(db.kernel_executor(payment_policy()).execute_create_payment(&command).is_err());
    let conn = db.pool().get().expect("connection");
    let payments: i64 =
        conn.query_row("SELECT COUNT(*) FROM payments", [], |row| row.get(0)).unwrap();
    let events: i64 =
        conn.query_row("SELECT COUNT(*) FROM kernel_outbox", [], |row| row.get(0)).unwrap();
    assert_eq!((payments, events), (0, 0));
}

#[test]
fn reservation_lifecycle_emits_exact_transactional_events() {
    let db = SqliteDatabase::in_memory().expect("create database");
    db.inventory()
        .create_item(CreateInventoryItem {
            sku: "KERNEL-STOCK-1".into(),
            name: "Kernel stock".into(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .expect("create item");
    let reservation = db
        .inventory()
        .reserve(ReserveInventory {
            sku: "KERNEL-STOCK-1".into(),
            location_id: Some(1),
            quantity: dec!(2.500),
            reference_type: "order".into(),
            reference_id: "order-kernel-1".into(),
            expires_in_seconds: None,
        })
        .expect("reserve");
    db.inventory().release_reservation(reservation.id).expect("release");

    let events = db.kernel_outbox().pending(100).expect("events");
    let lifecycle: Vec<_> =
        events.iter().filter(|event| event.aggregate_id == reservation.id.to_string()).collect();
    assert_eq!(lifecycle.len(), 2);
    let created = lifecycle
        .iter()
        .find(|event| event.event_type == "inventory.reservation_created.v1")
        .expect("reservation created event");
    assert_eq!(created.payload["quantity"], "2.500");
    assert!(lifecycle.iter().any(|event| event.event_type == "inventory.reservation_released.v1"));
}

#[test]
fn payment_and_outbox_event_commit_together_with_exact_money() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let payment = db
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(9007199254740993.25),
            currency: Some(CurrencyCode::USD),
            idempotency_key: Some("pay-exact-1".into()),
            ..Default::default()
        })
        .expect("create payment");

    let (event_count, payload): (i64, String) = db
        .pool()
        .get()
        .expect("connection")
        .query_row(
            "SELECT COUNT(*), payload FROM kernel_outbox WHERE aggregate_id = ?",
            [payment.id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("outbox event");

    assert_eq!(event_count, 1);
    let payload: serde_json::Value = serde_json::from_str(&payload).expect("valid payload");
    assert_eq!(payload["amount"], "9007199254740993.25");
    assert_eq!(payload["currency"], "USD");

    let pending = db.kernel_outbox().pending(10).expect("pending events");
    assert_eq!(pending.len(), 1);
    db.kernel_outbox().record_failure(pending[0].id, "temporary").expect("record failure");
    let pending = db.kernel_outbox().pending(10).expect("pending after failure");
    assert_eq!(pending[0].attempts, 1);
    assert_eq!(pending[0].last_error.as_deref(), Some("temporary"));
    db.kernel_outbox().mark_published(pending[0].id).expect("ack event");
    assert!(db.kernel_outbox().pending(10).expect("pending after ack").is_empty());
}

#[test]
fn outbox_leases_prevent_double_delivery_and_dead_letter_exhausted_events() {
    let db = SqliteDatabase::in_memory().expect("create database");
    db.payments()
        .create(CreatePayment {
            amount: dec!(1.00),
            currency: Some(CurrencyCode::USD),
            ..Default::default()
        })
        .expect("create event");
    let claimed = db.kernel_outbox().claim_pending("worker-a", 10, 30).expect("claim event");
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].lease_owner.as_deref(), Some("worker-a"));
    assert!(
        db.kernel_outbox().claim_pending("worker-b", 10, 30).expect("competing claim").is_empty()
    );
    assert!(!db.kernel_outbox().mark_published_by(claimed[0].id, "worker-b").expect("wrong ack"));
    assert!(
        db.kernel_outbox()
            .record_failure_by(claimed[0].id, "worker-a", "temporary", 60, 2)
            .expect("schedule retry")
    );
    assert!(db.kernel_outbox().pending(10).expect("delayed pending").is_empty());

    db.pool()
        .get()
        .expect("connection")
        .execute(
            "UPDATE kernel_outbox SET next_attempt_at = ? WHERE id = ?",
            rusqlite::params![
                (chrono::Utc::now() - chrono::Duration::seconds(1)).to_rfc3339(),
                claimed[0].id.to_string(),
            ],
        )
        .expect("make retry due");
    let retried = db.kernel_outbox().claim_pending("worker-b", 10, 30).expect("reclaim event");
    assert_eq!(retried.len(), 1);
    assert_eq!(retried[0].attempts, 1);
    assert!(
        db.kernel_outbox()
            .record_failure_by(retried[0].id, "worker-b", "permanent", 60, 2)
            .expect("dead letter")
    );
    assert!(db.kernel_outbox().pending(10).expect("pending after dead letter").is_empty());
    let (attempts, dead_lettered): (i64, Option<String>) = db
        .pool()
        .get()
        .expect("connection")
        .query_row(
            "SELECT attempts, dead_lettered_at FROM kernel_outbox WHERE id = ?",
            [claimed[0].id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("dead letter state");
    assert_eq!(attempts, 2);
    assert!(dead_lettered.is_some());
    let health = db.kernel_outbox().delivery_health().expect("delivery health");
    assert_eq!(health.dead_lettered, 1);
    assert_eq!(db.kernel_outbox().dead_letters(10).expect("dead letters").len(), 1);
    assert!(db.kernel_outbox().redrive_dead_letter(claimed[0].id, true).expect("redrive"));
    let health = db.kernel_outbox().delivery_health().expect("health after redrive");
    assert_eq!(health.dead_lettered, 0);
    assert_eq!(health.ready, 1);
    let redriven = db.kernel_outbox().claim_pending("worker-c", 1, 30).expect("claim redrive");
    assert_eq!(redriven[0].attempts, 0);
    assert!(db.kernel_outbox().mark_published_by(redriven[0].id, "worker-c").expect("ack"));
    assert_eq!(db.kernel_outbox().delivery_health().expect("final health").published, 1);
}

#[test]
fn payment_and_refund_reject_excess_currency_scale_without_writes() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let error = db
        .payments()
        .create(CreatePayment {
            amount: dec!(10.001),
            currency: Some(CurrencyCode::USD),
            ..Default::default()
        })
        .expect_err("over-scale payment must fail");
    assert!(matches!(error, CommerceError::MoneyScaleExceedsCurrency { .. }));

    let payment = db
        .payments()
        .create(CreatePayment {
            amount: dec!(10.00),
            currency: Some(CurrencyCode::USD),
            ..Default::default()
        })
        .expect("create valid payment");
    let payment = db.payments().mark_completed(payment.id).expect("complete payment");
    let error = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(1.001)),
            ..Default::default()
        })
        .expect_err("over-scale refund must fail");
    assert!(matches!(error, CommerceError::MoneyScaleExceedsCurrency { .. }));

    let conn = db.pool().get().expect("connection");
    let payments: i64 =
        conn.query_row("SELECT COUNT(*) FROM payments", [], |row| row.get(0)).unwrap();
    let refunds: i64 =
        conn.query_row("SELECT COUNT(*) FROM refunds", [], |row| row.get(0)).unwrap();
    let events: i64 =
        conn.query_row("SELECT COUNT(*) FROM kernel_outbox", [], |row| row.get(0)).unwrap();
    assert_eq!((payments, refunds, events), (1, 0, 1));
}

fn inventory_command(
    key: &str,
    sku: &str,
    quantity: rust_decimal::Decimal,
) -> CommandEnvelope<ReserveInventory> {
    let mut command = CommandEnvelope::preview(
        "inventory.reserve",
        key,
        KernelPrincipal {
            id: "agent:allocator-1".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-1".into()),
            capabilities: vec!["inventory.reserve".into()],
        },
        ReserveInventory {
            sku: sku.into(),
            location_id: Some(1),
            quantity,
            reference_type: "order".into(),
            reference_id: format!("order-{key}"),
            expires_in_seconds: Some(900),
        },
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

fn create_stock(db: &SqliteDatabase, sku: &str, quantity: rust_decimal::Decimal) {
    db.inventory()
        .create_item(CreateInventoryItem {
            sku: sku.into(),
            name: format!("Stock {sku}"),
            initial_quantity: Some(quantity),
            ..Default::default()
        })
        .expect("create stock");
}

fn confirm_reservation_command(
    key: &str,
    reservation_id: Uuid,
    quantity: Option<rust_decimal::Decimal>,
) -> CommandEnvelope<ConfirmInventoryReservation> {
    let mut command = CommandEnvelope::preview(
        "inventory.reservation.confirm",
        key,
        KernelPrincipal {
            id: "agent:fulfillment-1".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-1".into()),
            capabilities: vec!["inventory.reservation.confirm".into()],
        },
        ConfirmInventoryReservation { reservation_id, quantity },
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

fn release_reservation_command(
    key: &str,
    reservation_id: Uuid,
) -> CommandEnvelope<ReleaseInventoryReservation> {
    let mut command = CommandEnvelope::preview(
        "inventory.reservation.release",
        key,
        KernelPrincipal {
            id: "agent:allocator-1".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-1".into()),
            capabilities: vec!["inventory.reservation.release".into()],
        },
        ReleaseInventoryReservation { reservation_id },
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

fn direct_reservation(
    db: &SqliteDatabase,
    sku: &str,
    quantity: rust_decimal::Decimal,
) -> stateset_core::InventoryReservation {
    db.inventory()
        .reserve(ReserveInventory {
            sku: sku.into(),
            location_id: Some(1),
            quantity,
            reference_type: "order".into(),
            reference_id: format!("order-{sku}"),
            expires_in_seconds: Some(900),
        })
        .expect("reserve inventory")
}

#[test]
fn kernel_inventory_preview_promotes_applies_and_replays_exactly_once() {
    let db = SqliteDatabase::in_memory().expect("create database");
    create_stock(&db, "KERNEL-INV-1", dec!(10.000));
    let preview = inventory_command("kernel-inventory-1", "KERNEL-INV-1", dec!(2.500));
    let preview_receipt = db
        .kernel_executor(payment_policy())
        .execute_reserve_inventory(&preview)
        .expect("preview inventory");
    assert_eq!(preview_receipt.status, ExecutionStatus::Previewed);
    assert_eq!(preview_receipt.version_before, Some(1));
    assert_eq!(preview_receipt.version_after, Some(2));

    let mut apply = preview;
    apply.command_id = Uuid::new_v4();
    apply.mode = ExecutionMode::Apply;
    let applied = db
        .kernel_executor(payment_policy())
        .execute_reserve_inventory(&apply)
        .expect("reserve inventory");
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    assert_eq!(applied.result.as_ref().expect("reservation").quantity, dec!(2.500));
    assert_eq!(applied.version_after, Some(2));

    let mut retry = apply.clone();
    retry.command_id = Uuid::new_v4();
    let replay = db
        .kernel_executor(payment_policy())
        .execute_reserve_inventory(&retry)
        .expect("replay reservation");
    assert_eq!(replay.receipt_id, applied.receipt_id);
    assert_eq!(replay.result.expect("reservation").id, applied.result.expect("reservation").id);
    let mut changed = inventory_command("kernel-inventory-1", "KERNEL-INV-1", dec!(3.000));
    changed.mode = ExecutionMode::Apply;
    let conflict = db
        .kernel_executor(payment_policy())
        .execute_reserve_inventory(&changed)
        .expect("intent conflict");
    assert_eq!(conflict.error_code.as_deref(), Some("kernel.idempotency_conflict"));

    let events: Vec<_> = db
        .kernel_outbox()
        .pending(20)
        .expect("events")
        .into_iter()
        .filter(|event| event.event_type == "inventory.reservation_created.v1")
        .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].command_id, Some(apply.command_id));
    assert_eq!(events[0].principal_id.as_deref(), Some("agent:allocator-1"));
    assert_eq!(events[0].payload["quantity"], "2.500");
}

#[test]
fn kernel_inventory_rejects_award_quantity_substitution() {
    let db = SqliteDatabase::in_memory().expect("create database");
    create_stock(&db, "KERNEL-AWARD-1", dec!(100));
    let mut command = inventory_command("marketplace-award-1", "KERNEL-AWARD-1", dec!(75));
    command.mode = ExecutionMode::Apply;
    command.commitment = Some(EconomicCommitment {
        budget_id: None,
        amount: None,
        asset_amount: None,
        counterparty_id: Some("agent:buyer".into()),
        quantity: Some("50".into()),
        evidence: vec!["award:1".into()],
    });

    let receipt = db
        .kernel_executor(payment_policy())
        .execute_reserve_inventory(&command)
        .expect("sealed rejection");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.commitment_quantity_mismatch"));
    assert_eq!(
        db.inventory()
            .get_stock("KERNEL-AWARD-1")
            .expect("stock query")
            .expect("stock")
            .total_allocated,
        dec!(0)
    );
}

#[test]
fn concurrent_kernel_reservations_cannot_oversell_and_both_get_receipts() {
    let db = SqliteDatabase::in_memory().expect("create database");
    create_stock(&db, "KERNEL-INV-RACE", dec!(10));
    let executor = db.kernel_executor(payment_policy());
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let handles: Vec<_> = ["inventory-race-1", "inventory-race-2"]
        .into_iter()
        .map(|key| {
            let executor = executor.clone();
            let barrier = barrier.clone();
            let mut command = inventory_command(key, "KERNEL-INV-RACE", dec!(6));
            command.mode = ExecutionMode::Apply;
            std::thread::spawn(move || {
                barrier.wait();
                executor.execute_reserve_inventory(&command).expect("kernel receipt")
            })
        })
        .collect();
    barrier.wait();
    let receipts: Vec<_> =
        handles.into_iter().map(|handle| handle.join().expect("thread")).collect();
    assert_eq!(
        receipts.iter().filter(|receipt| receipt.status == ExecutionStatus::Succeeded).count(),
        1
    );
    assert_eq!(
        receipts
            .iter()
            .filter(|receipt| {
                receipt.error_code.as_deref() == Some("commerce.insufficient_stock")
            })
            .count(),
        1
    );
    let conn = db.pool().get().expect("connection");
    let reservations: i64 = conn
        .query_row("SELECT COUNT(*) FROM inventory_reservations", [], |row| row.get(0))
        .unwrap();
    let receipts: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM kernel_receipts WHERE command_type = 'inventory.reserve'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((reservations, receipts), (1, 2));
}

#[test]
fn kernel_inventory_version_conflict_is_durable_and_non_mutating() {
    let db = SqliteDatabase::in_memory().expect("create database");
    create_stock(&db, "KERNEL-INV-VERSION", dec!(10));
    let mut command = inventory_command("inventory-version-1", "KERNEL-INV-VERSION", dec!(1));
    command.mode = ExecutionMode::Apply;
    command.expected_version = Some(99);
    let receipt = db
        .kernel_executor(payment_policy())
        .execute_reserve_inventory(&command)
        .expect("version receipt");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.version_conflict"));
    assert_eq!(receipt.retry, stateset_core::RetryDisposition::AfterConflict);
    assert_eq!(receipt.version_before, Some(1));
    let conn = db.pool().get().expect("connection");
    let reservations: i64 = conn
        .query_row("SELECT COUNT(*) FROM inventory_reservations", [], |row| row.get(0))
        .unwrap();
    assert_eq!(reservations, 0);
}

#[test]
fn inventory_receipt_failure_rolls_back_reservation_balance_and_event() {
    let db = SqliteDatabase::in_memory().expect("create database");
    create_stock(&db, "KERNEL-INV-ROLLBACK", dec!(10));
    let mut command = inventory_command("inventory-rollback-1", "KERNEL-INV-ROLLBACK", dec!(2));
    command.mode = ExecutionMode::Apply;
    let conn = db.pool().get().expect("connection");
    conn.execute(
        "INSERT INTO kernel_receipts (
            command_id, idempotency_key, command_type, contract_version,
            request_hash, status, receipt, created_at, completed_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        rusqlite::params![
            command.command_id.to_string(),
            "different-inventory-key",
            "inventory.reserve",
            "1.0",
            "preexisting",
            "rejected",
            "{}",
            chrono::Utc::now().to_rfc3339(),
            chrono::Utc::now().to_rfc3339(),
        ],
    )
    .expect("seed conflicting command identity");
    drop(conn);

    assert!(db.kernel_executor(payment_policy()).execute_reserve_inventory(&command).is_err());
    let conn = db.pool().get().expect("connection");
    let reservations: i64 = conn
        .query_row("SELECT COUNT(*) FROM inventory_reservations", [], |row| row.get(0))
        .unwrap();
    let reservation_events: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM kernel_outbox
             WHERE event_type = 'inventory.reservation_created.v1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let (allocated, available): (String, String) = conn
        .query_row(
            "SELECT quantity_allocated, quantity_available FROM inventory_balances
             WHERE location_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((reservations, reservation_events), (0, 0));
    assert_eq!((allocated.as_str(), available.as_str()), ("0", "10"));
}

#[test]
fn kernel_confirmation_preview_promotes_and_replays_exactly_once() {
    let db = SqliteDatabase::in_memory().expect("create database");
    create_stock(&db, "KERNEL-CONFIRM", dec!(10));
    let reservation = direct_reservation(&db, "KERNEL-CONFIRM", dec!(4));
    let preview = confirm_reservation_command("confirm-reservation-1", reservation.id, None);
    let previewed = db
        .kernel_executor(payment_policy())
        .execute_confirm_inventory_reservation(&preview)
        .expect("preview confirmation");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    assert_eq!(previewed.version_before, Some(2));
    assert_eq!(previewed.version_after, Some(2));
    assert_eq!(previewed.result.expect("reservation").status, ReservationStatus::Pending);

    let mut apply = preview;
    apply.command_id = Uuid::new_v4();
    apply.mode = ExecutionMode::Apply;
    let applied = db
        .kernel_executor(payment_policy())
        .execute_confirm_inventory_reservation(&apply)
        .expect("apply confirmation");
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    assert_eq!(applied.result.as_ref().expect("reservation").status, ReservationStatus::Confirmed);
    assert_eq!(applied.version_after, Some(2));
    assert_eq!(applied.event_ids.len(), 1);

    let mut retry = apply.clone();
    retry.command_id = Uuid::new_v4();
    let replay = db
        .kernel_executor(payment_policy())
        .execute_confirm_inventory_reservation(&retry)
        .expect("replay confirmation");
    assert_eq!(replay.receipt_id, applied.receipt_id);

    let events: Vec<_> = db
        .kernel_outbox()
        .pending(20)
        .expect("events")
        .into_iter()
        .filter(|event| event.event_type == "inventory.reservation_confirmed.v1")
        .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].command_id, Some(apply.command_id));
    assert_eq!(events[0].principal_id.as_deref(), Some("agent:fulfillment-1"));
}

#[test]
fn kernel_partial_confirmation_returns_the_confirmed_split() {
    let db = SqliteDatabase::in_memory().expect("create database");
    create_stock(&db, "KERNEL-PARTIAL-CONFIRM", dec!(10));
    let reservation = direct_reservation(&db, "KERNEL-PARTIAL-CONFIRM", dec!(5));
    let mut command =
        confirm_reservation_command("partial-confirm-1", reservation.id, Some(dec!(2)));
    command.mode = ExecutionMode::Apply;
    let receipt = db
        .kernel_executor(payment_policy())
        .execute_confirm_inventory_reservation(&command)
        .expect("partial confirmation");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded);
    let confirmed = receipt.result.expect("confirmed split");
    assert_ne!(confirmed.id, reservation.id);
    assert_eq!(confirmed.quantity, dec!(2));
    assert_eq!(confirmed.status, ReservationStatus::Confirmed);
    assert_eq!(receipt.version_before, Some(2));
    assert_eq!(receipt.version_after, Some(2));

    let conn = db.pool().get().expect("connection");
    let (remaining, status): (String, String) = conn
        .query_row(
            "SELECT quantity, status FROM inventory_reservations WHERE id = ?",
            [reservation.id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("source reservation");
    let (allocated, available): (String, String) = conn
        .query_row(
            "SELECT quantity_allocated, quantity_available FROM inventory_balances WHERE location_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("inventory balance");
    assert_eq!((remaining.as_str(), status.as_str()), ("3", "pending"));
    assert_eq!((allocated.as_str(), available.as_str()), ("5", "5"));
}

#[test]
fn kernel_release_preview_applies_and_rejects_later_confirmation() {
    let db = SqliteDatabase::in_memory().expect("create database");
    create_stock(&db, "KERNEL-RELEASE", dec!(10));
    let reservation = direct_reservation(&db, "KERNEL-RELEASE", dec!(3));
    let preview = release_reservation_command("release-reservation-1", reservation.id);
    let previewed = db
        .kernel_executor(payment_policy())
        .execute_release_inventory_reservation(&preview)
        .expect("preview release");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    assert_eq!(previewed.version_before, Some(2));
    assert_eq!(previewed.version_after, Some(3));

    let mut apply = preview;
    apply.command_id = Uuid::new_v4();
    apply.mode = ExecutionMode::Apply;
    let released = db
        .kernel_executor(payment_policy())
        .execute_release_inventory_reservation(&apply)
        .expect("apply release");
    assert_eq!(released.status, ExecutionStatus::Succeeded);
    assert_eq!(released.result.expect("reservation").status, ReservationStatus::Released);
    assert_eq!(released.version_after, Some(3));

    let mut confirm = confirm_reservation_command("confirm-released-1", reservation.id, None);
    confirm.mode = ExecutionMode::Apply;
    let rejected = db
        .kernel_executor(payment_policy())
        .execute_confirm_inventory_reservation(&confirm)
        .expect("durable rejection");
    assert_eq!(rejected.status, ExecutionStatus::Rejected);
    assert_eq!(rejected.error_code.as_deref(), Some("commerce.reservation_not_confirmable"));

    let conn = db.pool().get().expect("connection");
    let (allocated, available): (String, String) = conn
        .query_row(
            "SELECT quantity_allocated, quantity_available FROM inventory_balances WHERE location_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("inventory balance");
    assert_eq!((allocated.as_str(), available.as_str()), ("0", "10"));
}

#[test]
fn lifecycle_receipt_failure_rolls_back_release_balance_and_event() {
    let db = SqliteDatabase::in_memory().expect("create database");
    create_stock(&db, "KERNEL-RELEASE-ROLLBACK", dec!(10));
    let reservation = direct_reservation(&db, "KERNEL-RELEASE-ROLLBACK", dec!(2));
    let mut command = release_reservation_command("release-rollback-1", reservation.id);
    command.mode = ExecutionMode::Apply;
    let conn = db.pool().get().expect("connection");
    conn.execute(
        "INSERT INTO kernel_receipts (
            command_id, idempotency_key, command_type, contract_version,
            request_hash, status, receipt, created_at, completed_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        rusqlite::params![
            command.command_id.to_string(),
            "different-release-key",
            "inventory.reservation.release",
            "1.0",
            "preexisting",
            "rejected",
            "{}",
            chrono::Utc::now().to_rfc3339(),
            chrono::Utc::now().to_rfc3339(),
        ],
    )
    .expect("seed conflicting command identity");
    drop(conn);

    assert!(
        db.kernel_executor(payment_policy())
            .execute_release_inventory_reservation(&command)
            .is_err()
    );
    let conn = db.pool().get().expect("connection");
    let status: String = conn
        .query_row(
            "SELECT status FROM inventory_reservations WHERE id = ?",
            [reservation.id.to_string()],
            |row| row.get(0),
        )
        .expect("reservation status");
    let release_events: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM kernel_outbox WHERE event_type = 'inventory.reservation_released.v1'",
            [],
            |row| row.get(0),
        )
        .expect("release events");
    let (allocated, available): (String, String) = conn
        .query_row(
            "SELECT quantity_allocated, quantity_available FROM inventory_balances WHERE location_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("inventory balance");
    assert_eq!(status, "pending");
    assert_eq!(release_events, 0);
    assert_eq!((allocated.as_str(), available.as_str()), ("2", "8"));
}

fn create_kernel_order(db: &SqliteDatabase, sku: &str) -> stateset_core::Order {
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: format!("{sku}@example.com"),
            first_name: "Kernel".into(),
            last_name: "Order".into(),
            ..Default::default()
        })
        .expect("create customer");
    db.orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: sku.into(),
                name: "Kernel item".into(),
                quantity: 2,
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create order")
}

fn transition_order_command(
    key: &str,
    order_id: stateset_core::OrderId,
    status: OrderStatus,
) -> CommandEnvelope<TransitionOrder> {
    let mut command = CommandEnvelope::preview(
        "orders.transition",
        key,
        KernelPrincipal {
            id: "agent:order-ops-1".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-1".into()),
            capabilities: vec!["orders.transition".into()],
        },
        TransitionOrder { order_id, status, payment_status: None, void_payments: false },
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

#[test]
fn kernel_order_transition_previews_applies_and_replays() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let order = create_kernel_order(&db, "KERNEL-ORDER-TRANSITION");
    let preview = transition_order_command("order-confirm-1", order.id, OrderStatus::Confirmed);
    let previewed = db
        .kernel_executor(payment_policy())
        .execute_transition_order(&preview)
        .expect("preview transition");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    assert_eq!(previewed.version_before, Some(order.version));
    assert_eq!(previewed.version_after, Some(order.version + 1));
    assert_eq!(previewed.result.expect("order").status, OrderStatus::Pending);

    let mut apply = preview;
    apply.command_id = Uuid::new_v4();
    apply.mode = ExecutionMode::Apply;
    let applied = db
        .kernel_executor(payment_policy())
        .execute_transition_order(&apply)
        .expect("apply transition");
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    assert_eq!(applied.result.as_ref().expect("order").status, OrderStatus::Confirmed);
    assert_eq!(applied.event_ids.len(), 1);

    let mut retry = apply.clone();
    retry.command_id = Uuid::new_v4();
    let replay = db
        .kernel_executor(payment_policy())
        .execute_transition_order(&retry)
        .expect("replay transition");
    assert_eq!(replay.receipt_id, applied.receipt_id);
    let events: Vec<_> = db
        .kernel_outbox()
        .pending(50)
        .expect("events")
        .into_iter()
        .filter(|event| {
            event.event_type == "orders.updated.v1" && event.command_id == Some(apply.command_id)
        })
        .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload["status_before"], "pending");
    assert_eq!(events[0].payload["status_after"], "confirmed");
}

#[test]
fn kernel_order_transition_rejects_invalid_and_shipment_targets_durably() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let order = create_kernel_order(&db, "KERNEL-ORDER-REJECT");
    let mut invalid = transition_order_command("order-invalid-1", order.id, OrderStatus::Delivered);
    invalid.mode = ExecutionMode::Apply;
    let rejected = db
        .kernel_executor(payment_policy())
        .execute_transition_order(&invalid)
        .expect("invalid transition receipt");
    assert_eq!(rejected.status, ExecutionStatus::Rejected);
    assert_eq!(rejected.error_code.as_deref(), Some("commerce.invalid_order_status_transition"));

    let mut shipment = transition_order_command("order-shipment-1", order.id, OrderStatus::Shipped);
    shipment.mode = ExecutionMode::Apply;
    let rejected = db
        .kernel_executor(payment_policy())
        .execute_transition_order(&shipment)
        .expect("shipment command receipt");
    assert_eq!(rejected.error_code.as_deref(), Some("commerce.shipment_command_required"));
    assert_eq!(
        db.orders().get(order.id).expect("get order").expect("order").status,
        OrderStatus::Pending
    );
}

#[test]
fn kernel_order_cancellation_releases_inventory_atomically() {
    let db = SqliteDatabase::in_memory().expect("create database");
    create_stock(&db, "KERNEL-ORDER-CANCEL", dec!(10));
    let order = create_kernel_order(&db, "KERNEL-ORDER-CANCEL");
    let mut command = transition_order_command("order-cancel-1", order.id, OrderStatus::Cancelled);
    command.mode = ExecutionMode::Apply;
    command.expected_version = Some(order.version);
    let receipt = db
        .kernel_executor(payment_policy())
        .execute_transition_order(&command)
        .expect("cancel order");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded);
    assert_eq!(receipt.event_ids.len(), 2);
    assert_eq!(receipt.result.expect("order").status, OrderStatus::Cancelled);

    let conn = db.pool().get().expect("connection");
    let reservation_status: String = conn
        .query_row(
            "SELECT status FROM inventory_reservations WHERE reference_type = 'order' AND reference_id = ?",
            [order.id.to_string()],
            |row| row.get(0),
        )
        .expect("reservation");
    let (allocated, available): (String, String) = conn
        .query_row(
            "SELECT quantity_allocated, quantity_available FROM inventory_balances WHERE location_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("balance");
    assert_eq!(reservation_status, "released");
    assert_eq!((allocated.as_str(), available.as_str()), ("0", "10"));
    let release_command_id: String = conn
        .query_row(
            "SELECT command_id FROM kernel_outbox
             WHERE event_type = 'inventory.reservation_released.v1'",
            [],
            |row| row.get(0),
        )
        .expect("release command context");
    assert_eq!(release_command_id, command.command_id.to_string());
}

fn ship_order_command(
    key: &str,
    order_id: stateset_core::OrderId,
    lines: Option<Vec<ShipmentLineInput>>,
) -> CommandEnvelope<ShipOrderCommand> {
    let mut command = CommandEnvelope::preview(
        "orders.ship",
        key,
        KernelPrincipal {
            id: "agent:fulfillment-1".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-1".into()),
            capabilities: vec!["orders.ship".into()],
        },
        ShipOrderCommand { order_id, tracking_number: Some("TRACK-KERNEL-1".into()), lines },
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

#[test]
fn kernel_partial_shipment_promotes_replays_and_links_all_facts() {
    let db = SqliteDatabase::in_memory().expect("create database");
    create_stock(&db, "KERNEL-ORDER-SHIP", dec!(10));
    let order = create_kernel_order(&db, "KERNEL-ORDER-SHIP");
    db.orders()
        .update(
            order.id,
            UpdateOrder { status: Some(OrderStatus::Confirmed), ..Default::default() },
        )
        .expect("confirm order");
    let order = db
        .orders()
        .update(
            order.id,
            UpdateOrder { status: Some(OrderStatus::Processing), ..Default::default() },
        )
        .expect("process order");
    let line_id = order.items[0].id;
    let mut preview = ship_order_command(
        "order-ship-partial-1",
        order.id,
        Some(vec![ShipmentLineInput { order_item_id: line_id, quantity: 1 }]),
    );
    preview.expected_version = Some(order.version);
    let previewed = db
        .kernel_executor(payment_policy())
        .execute_ship_order(&preview)
        .expect("preview shipment");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    assert_eq!(previewed.version_before, Some(order.version));

    let mut apply = preview;
    apply.command_id = Uuid::new_v4();
    apply.mode = ExecutionMode::Apply;
    let applied =
        db.kernel_executor(payment_policy()).execute_ship_order(&apply).expect("apply shipment");
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    let shipped = applied.result.as_ref().expect("order");
    assert_eq!(shipped.status, OrderStatus::PartiallyShipped);
    assert_eq!(shipped.items[0].shipped_quantity, 1);
    assert_eq!(shipped.tracking_number.as_deref(), Some("TRACK-KERNEL-1"));
    assert_eq!(applied.event_ids.len(), 2);

    let mut retry = apply.clone();
    retry.command_id = Uuid::new_v4();
    let replay =
        db.kernel_executor(payment_policy()).execute_ship_order(&retry).expect("replay shipment");
    assert_eq!(replay.receipt_id, applied.receipt_id);
    let linked = db
        .kernel_outbox()
        .pending(100)
        .expect("events")
        .into_iter()
        .filter(|event| event.command_id == Some(apply.command_id))
        .count();
    assert_eq!(linked, 2);
}

#[test]
fn kernel_shipment_rejects_overship_without_mutation() {
    let db = SqliteDatabase::in_memory().expect("create database");
    create_stock(&db, "KERNEL-ORDER-OVERSHIP", dec!(10));
    let order = create_kernel_order(&db, "KERNEL-ORDER-OVERSHIP");
    db.orders()
        .update(
            order.id,
            UpdateOrder { status: Some(OrderStatus::Confirmed), ..Default::default() },
        )
        .expect("confirm order");
    let order = db
        .orders()
        .update(
            order.id,
            UpdateOrder { status: Some(OrderStatus::Processing), ..Default::default() },
        )
        .expect("process order");
    let mut command = ship_order_command(
        "order-overship-1",
        order.id,
        Some(vec![ShipmentLineInput { order_item_id: order.items[0].id, quantity: 3 }]),
    );
    command.mode = ExecutionMode::Apply;
    let receipt = db
        .kernel_executor(payment_policy())
        .execute_ship_order(&command)
        .expect("rejection receipt");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("commerce.shipment_invalid"));
    let unchanged = db.orders().get(order.id).expect("get order").expect("order");
    assert_eq!(unchanged.status, OrderStatus::Processing);
    assert_eq!(unchanged.items[0].shipped_quantity, 0);
}

fn transition_return_command(
    key: &str,
    return_id: stateset_core::ReturnId,
    status: ReturnStatus,
) -> CommandEnvelope<TransitionReturn> {
    let mut command = CommandEnvelope::preview(
        "returns.transition",
        key,
        KernelPrincipal {
            id: "agent:returns-1".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-1".into()),
            capabilities: vec!["returns.transition".into()],
        },
        TransitionReturn { return_id, status },
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

fn post_journal_command(key: &str, journal_entry_id: Uuid) -> CommandEnvelope<PostJournalEntry> {
    let mut command = CommandEnvelope::preview(
        "ledger.post",
        key,
        KernelPrincipal {
            id: "agent:finance-1".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-1".into()),
            capabilities: vec!["ledger.post".into()],
        },
        PostJournalEntry { journal_entry_id, posted_by: "agent:finance-1".into() },
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

fn create_kernel_journal(
    db: &SqliteDatabase,
    suffix: &str,
) -> (stateset_core::JournalEntry, Uuid, Uuid) {
    let gl = db.general_ledger();
    let period = gl
        .create_period(CreateGlPeriod {
            period_name: format!("FY2026-{suffix}"),
            fiscal_year: 2026,
            period_number: 1,
            start_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date"),
            end_date: chrono::NaiveDate::from_ymd_opt(2026, 12, 31).expect("date"),
        })
        .expect("create period");
    gl.open_period(period.id).expect("open period");
    let make_account = |number: String, name: &str, account_type| {
        gl.create_account(CreateGlAccount {
            account_number: number,
            name: name.into(),
            description: None,
            account_type,
            account_sub_type: None,
            parent_account_id: None,
            is_header: Some(false),
            is_posting: Some(true),
            currency: None,
        })
        .expect("create account")
    };
    let cash = make_account(format!("1000-{suffix}"), "Cash", AccountType::Asset);
    let revenue = make_account(format!("4000-{suffix}"), "Revenue", AccountType::Revenue);
    let entry = gl
        .create_journal_entry(CreateJournalEntry {
            entry_date: chrono::NaiveDate::from_ymd_opt(2026, 8, 1).expect("date"),
            entry_type: None,
            description: "Kernel-governed sale".into(),
            lines: vec![
                CreateJournalEntryLine::debit(cash.id, dec!(25), None),
                CreateJournalEntryLine::credit(revenue.id, dec!(25), None),
            ],
            source_document_type: Some("kernel_command".into()),
            source_document_id: None,
            auto_post: Some(false),
        })
        .expect("create journal entry");
    (entry, cash.id, revenue.id)
}

#[test]
fn kernel_return_transition_promotes_replays_and_rejects_stale_or_invalid_work() {
    let db = SqliteDatabase::in_memory().expect("create database");
    create_stock(&db, "KERNEL-RETURN", dec!(10));
    let order = create_kernel_order(&db, "KERNEL-RETURN");
    for status in [OrderStatus::Confirmed, OrderStatus::Processing, OrderStatus::Shipped] {
        db.orders()
            .update(order.id, UpdateOrder { status: Some(status), ..Default::default() })
            .expect("advance order");
    }
    let order = db.orders().get(order.id).expect("get order").expect("order");
    let returned = db
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            items: vec![CreateReturnItem {
                order_item_id: order.items[0].id,
                quantity: 1,
                condition: None,
            }],
            ..Default::default()
        })
        .expect("create return");
    let mut preview =
        transition_return_command("return-approve-1", returned.id, ReturnStatus::Approved);
    preview.expected_version = Some(returned.version);
    let previewed = db
        .kernel_executor(payment_policy())
        .execute_transition_return(&preview)
        .expect("preview return");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    assert_eq!(previewed.version_after, Some(returned.version + 1));

    let mut apply = preview;
    apply.command_id = Uuid::new_v4();
    apply.mode = ExecutionMode::Apply;
    let applied = db
        .kernel_executor(payment_policy())
        .execute_transition_return(&apply)
        .expect("approve return");
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    assert_eq!(applied.result.as_ref().expect("return").status, ReturnStatus::Approved);
    assert_eq!(applied.event_ids.len(), 1);

    let mut retry = apply.clone();
    retry.command_id = Uuid::new_v4();
    let replay = db
        .kernel_executor(payment_policy())
        .execute_transition_return(&retry)
        .expect("replay return");
    assert_eq!(replay.receipt_id, applied.receipt_id);

    let mut stale =
        transition_return_command("return-stale-1", returned.id, ReturnStatus::InTransit);
    stale.mode = ExecutionMode::Apply;
    stale.expected_version = Some(returned.version);
    let rejected = db
        .kernel_executor(payment_policy())
        .execute_transition_return(&stale)
        .expect("stale receipt");
    assert_eq!(rejected.error_code.as_deref(), Some("kernel.version_conflict"));

    let mut invalid =
        transition_return_command("return-invalid-1", returned.id, ReturnStatus::Completed);
    invalid.mode = ExecutionMode::Apply;
    let rejected = db
        .kernel_executor(payment_policy())
        .execute_transition_return(&invalid)
        .expect("invalid receipt");
    assert_eq!(rejected.error_code.as_deref(), Some("commerce.invalid_return_status_transition"));
    let event = db
        .kernel_outbox()
        .pending(100)
        .expect("events")
        .into_iter()
        .find(|event| event.command_id == Some(apply.command_id))
        .expect("causal return event");
    assert_eq!(event.event_type, "returns.updated.v1");
}

#[test]
fn kernel_journal_post_previews_applies_and_replays_exactly_once() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let (entry, cash_id, revenue_id) = create_kernel_journal(&db, "kernel-post");
    let preview = post_journal_command("ledger-post-1", entry.id);
    let previewed = db
        .kernel_executor(payment_policy())
        .execute_post_journal_entry(&preview)
        .expect("preview journal post");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    assert_eq!(previewed.result.as_ref().expect("journal").status, JournalEntryStatus::Posted);

    let mut apply = preview;
    apply.command_id = Uuid::new_v4();
    apply.mode = ExecutionMode::Apply;
    let applied = db
        .kernel_executor(payment_policy())
        .execute_post_journal_entry(&apply)
        .expect("post journal");
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    assert_eq!(applied.result.as_ref().expect("journal").status, JournalEntryStatus::Posted);
    assert_eq!(applied.event_ids.len(), 1);

    let mut retry = apply.clone();
    retry.command_id = Uuid::new_v4();
    let replay = db
        .kernel_executor(payment_policy())
        .execute_post_journal_entry(&retry)
        .expect("replay journal post");
    assert_eq!(replay.receipt_id, applied.receipt_id);

    let gl = db.general_ledger();
    assert_eq!(gl.get_account(cash_id).expect("cash").expect("cash").current_balance, dec!(25));
    assert_eq!(
        gl.get_account(revenue_id).expect("revenue").expect("revenue").current_balance,
        dec!(25)
    );
    let event = db
        .kernel_outbox()
        .pending(100)
        .expect("events")
        .into_iter()
        .find(|event| event.command_id == Some(apply.command_id))
        .expect("causal ledger event");
    assert_eq!(event.event_type, "ledger.journal_entry_posted.v1");
    assert_eq!(event.payload["total_debits"], "25");
    assert_eq!(event.payload["total_credits"], "25");
}

#[test]
fn kernel_journal_post_rejects_closed_period_durably() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let (entry, cash_id, revenue_id) = create_kernel_journal(&db, "kernel-closed");
    db.general_ledger().close_period(entry.period_id, "tester").expect("close period");

    let mut command = post_journal_command("ledger-post-closed", entry.id);
    command.mode = ExecutionMode::Apply;
    let receipt = db
        .kernel_executor(payment_policy())
        .execute_post_journal_entry(&command)
        .expect("execute against closed period");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("commerce.ledger.period_not_open"));

    let gl = db.general_ledger();
    assert_eq!(
        gl.get_journal_entry(entry.id).expect("entry").expect("entry").status,
        JournalEntryStatus::Draft,
        "rejected post must leave the entry a draft"
    );
    assert_eq!(gl.get_account(cash_id).expect("cash").expect("cash").current_balance, dec!(0));
    assert_eq!(
        gl.get_account(revenue_id).expect("revenue").expect("revenue").current_balance,
        dec!(0)
    );
}

#[test]
fn kernel_journal_post_rolls_back_balances_and_event_when_receipt_fails() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let (entry, cash_id, revenue_id) = create_kernel_journal(&db, "kernel-rollback");
    let mut command = post_journal_command("ledger-post-rollback", entry.id);
    command.mode = ExecutionMode::Apply;
    let conn = db.pool().get().expect("connection");
    conn.execute(
        "INSERT INTO kernel_receipts (
            command_id, idempotency_key, command_type, contract_version,
            request_hash, status, receipt, created_at, completed_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        rusqlite::params![
            command.command_id.to_string(),
            "different-ledger-key",
            "ledger.post",
            "1.0",
            "preexisting",
            "rejected",
            "{}",
            chrono::Utc::now().to_rfc3339(),
            chrono::Utc::now().to_rfc3339(),
        ],
    )
    .expect("seed conflicting command identity");
    drop(conn);

    assert!(db.kernel_executor(payment_policy()).execute_post_journal_entry(&command).is_err());
    let gl = db.general_ledger();
    assert_eq!(
        gl.get_journal_entry(entry.id).expect("journal").expect("journal").status,
        JournalEntryStatus::Draft
    );
    assert_eq!(gl.get_account(cash_id).expect("cash").expect("cash").current_balance, dec!(0));
    assert_eq!(
        gl.get_account(revenue_id).expect("revenue").expect("revenue").current_balance,
        dec!(0)
    );
    let conn = db.pool().get().expect("connection");
    let events: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM kernel_outbox WHERE command_id = ?",
            [command.command_id.to_string()],
            |row| row.get(0),
        )
        .expect("count events");
    assert_eq!(events, 0);
}

#[test]
fn kernel_receipt_audit_chain_seals_receipts_and_detects_tampering() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let mut command = payment_command("audit-chain-payment-1", dec!(12.34));
    command.mode = ExecutionMode::Apply;
    let returned = db
        .kernel_executor(payment_policy())
        .execute_create_payment(&command)
        .expect("execute payment");

    let stored = db
        .kernel_outbox()
        .receipt_by_idempotency_key(&command.idempotency_key)
        .expect("load receipt")
        .expect("stored receipt");
    let audit_hash = stored.receipt["audit_hash"].as_str().expect("sealed audit hash");
    assert_eq!(audit_hash.len(), 64);
    assert_eq!(returned.audit_hash.as_deref(), Some(audit_hash));
    let verified = db.kernel_outbox().verify_audit_chain().expect("verify chain");
    assert!(verified.valid);
    assert_eq!(verified.entries, 1);
    assert_eq!(verified.head_hash.as_deref(), Some(audit_hash));

    let conn = db.pool().get().expect("connection");
    conn.execute(
        "UPDATE kernel_receipts
         SET receipt = json_set(receipt, '$.status', 'rejected') WHERE idempotency_key = ?",
        [&command.idempotency_key],
    )
    .expect("tamper materialized receipt");
    drop(conn);
    let verification = db.kernel_outbox().verify_audit_chain().expect("verify receipt tampering");
    assert!(!verification.valid);
    assert_eq!(verification.first_invalid_sequence, Some(1));

    let conn = db.pool().get().expect("connection");
    conn.execute(
        "UPDATE kernel_receipts SET receipt = ? WHERE idempotency_key = ?",
        rusqlite::params![
            serde_json::to_string(&stored.receipt).expect("serialize receipt"),
            command.idempotency_key,
        ],
    )
    .expect("restore materialized receipt");
    conn.execute(
        "UPDATE kernel_receipt_audit_log
         SET receipt = json_set(receipt, '$.status', 'rejected') WHERE sequence = 1",
        [],
    )
    .expect("tamper audit record");
    drop(conn);
    let verification = db.kernel_outbox().verify_audit_chain().expect("verify tampering");
    assert!(!verification.valid);
    assert_eq!(verification.first_invalid_sequence, Some(1));
}

#[test]
fn audit_checkpoints_are_portable_append_stable_and_tamper_evident() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let mut first = payment_command("audit-checkpoint-payment-1", dec!(12.34));
    first.mode = ExecutionMode::Apply;
    db.kernel_executor(payment_policy())
        .execute_create_payment(&first)
        .expect("execute first payment");

    let checkpoint = db.kernel_outbox().audit_checkpoint().expect("create checkpoint");
    assert_eq!(checkpoint.entries, 1);
    assert_eq!(checkpoint.checkpoint_hash.len(), 64);
    assert!(db.kernel_outbox().verify_audit_checkpoint(&checkpoint).expect("verify checkpoint"));

    let mut second = payment_command("audit-checkpoint-payment-2", dec!(56.78));
    second.mode = ExecutionMode::Apply;
    db.kernel_executor(payment_policy())
        .execute_create_payment(&second)
        .expect("execute second payment");
    assert!(
        db.kernel_outbox()
            .verify_audit_checkpoint(&checkpoint)
            .expect("old checkpoint remains valid")
    );

    let mut wrong_sequence = db.kernel_outbox().audit_checkpoint().expect("later checkpoint");
    assert!(wrong_sequence.entries > checkpoint.entries);
    wrong_sequence.entries = checkpoint.entries;
    reseal_checkpoint(&mut wrong_sequence);
    assert!(
        !db.kernel_outbox()
            .verify_audit_checkpoint(&wrong_sequence)
            .expect("head hash must match the claimed sequence")
    );

    let mut forged = checkpoint;
    forged.head_hash = Some("00".repeat(32));
    assert!(!db.kernel_outbox().verify_audit_checkpoint(&forged).expect("reject forged anchor"));
}

fn settle_x402_command(key: &str, intent_id: Uuid) -> CommandEnvelope<SettleX402Intent> {
    let mut command = CommandEnvelope::preview(
        "x402.settle",
        key,
        KernelPrincipal {
            id: "agent:settlement-1".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-1".into()),
            capabilities: vec!["x402.settle".into()],
        },
        SettleX402Intent { intent_id, tx_hash: "0xconfirmed-settlement".into(), block_number: 42 },
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

#[test]
fn kernel_x402_settlement_previews_applies_and_replays_exactly_once() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let intent = db
        .x402_payment_intents()
        .create(CreateX402PaymentIntent {
            payer_address: "0xpayer-kernel".into(),
            payee_address: "0xpayee-kernel".into(),
            amount: 1_000_000,
            asset: X402Asset::Usdc,
            network: X402Network::SetChain,
            ..Default::default()
        })
        .expect("create intent");
    let conn = db.pool().get().expect("connection");
    conn.execute(
        "UPDATE x402_payment_intents SET status = 'sequenced' WHERE id = ?",
        [intent.id.to_string()],
    )
    .expect("sequence intent fixture");
    drop(conn);

    let preview = settle_x402_command("x402-settle-1", intent.id);
    let previewed = db
        .kernel_executor(payment_policy())
        .execute_settle_x402_intent(&preview)
        .expect("preview settlement");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    assert_eq!(previewed.result.as_ref().expect("intent").status, X402IntentStatus::Settled);

    let mut apply = preview;
    apply.command_id = Uuid::new_v4();
    apply.mode = ExecutionMode::Apply;
    let applied = db
        .kernel_executor(payment_policy())
        .execute_settle_x402_intent(&apply)
        .expect("apply settlement");
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    assert_eq!(
        applied.result.as_ref().expect("intent").tx_hash.as_deref(),
        Some("0xconfirmed-settlement")
    );
    assert!(applied.audit_hash.is_some());

    let mut retry = apply.clone();
    retry.command_id = Uuid::new_v4();
    let replay = db
        .kernel_executor(payment_policy())
        .execute_settle_x402_intent(&retry)
        .expect("replay settlement");
    assert_eq!(replay.receipt_id, applied.receipt_id);
    let persisted = db.x402_payment_intents().get(intent.id).expect("get intent").expect("intent");
    assert_eq!(persisted.status, X402IntentStatus::Settled);
    assert_eq!(persisted.block_number, Some(42));
    let events = db
        .kernel_outbox()
        .pending(100)
        .expect("events")
        .into_iter()
        .filter(|event| event.command_id == Some(apply.command_id))
        .count();
    assert_eq!(events, 1);
}

fn checkout_command(key: &str, cart_id: stateset_core::CartId) -> CommandEnvelope<CommitCheckout> {
    let mut command = CommandEnvelope::preview(
        "checkout.commit",
        key,
        KernelPrincipal {
            id: "agent:checkout-commit-1".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-1".into()),
            capabilities: vec!["checkout.commit".into()],
        },
        CommitCheckout::new(cart_id),
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

fn ready_checkout_cart(db: &SqliteDatabase, email: &str) -> stateset_core::CartId {
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: email.into(),
            first_name: "Kernel".into(),
            last_name: "Checkout".into(),
            ..Default::default()
        })
        .expect("create customer");
    let carts = db.carts();
    let cart = carts
        .create(CreateCart {
            customer_id: Some(customer.id),
            customer_email: Some(email.into()),
            customer_name: Some("Kernel Checkout".into()),
            ..Default::default()
        })
        .expect("create cart");
    carts
        .add_item(
            cart.id,
            AddCartItem {
                product_id: Some(ProductId::new()),
                sku: "KERNEL-CHECKOUT-SKU".into(),
                name: "Kernel Checkout Item".into(),
                quantity: 2,
                unit_price: dec!(19.99),
                ..Default::default()
            },
        )
        .expect("add item");
    carts
        .set_shipping_address(
            cart.id,
            CartAddress {
                first_name: "Kernel".into(),
                last_name: "Checkout".into(),
                company: None,
                line1: "1 Atomic Way".into(),
                line2: None,
                city: "Vancouver".into(),
                state: Some("BC".into()),
                postal_code: "V6B 1A1".into(),
                country: "CA".into(),
                phone: None,
                email: Some(email.into()),
            },
        )
        .expect("set shipping");
    carts
        .set_payment(
            cart.id,
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_kernel_preview_only".into()),
                ..Default::default()
            },
        )
        .expect("set payment");
    cart.id
}

#[test]
fn kernel_checkout_fingerprint_rejects_changed_terms_without_economic_effects() {
    for mutation in [
        "UPDATE cart_items SET sku = 'SUBSTITUTED-SAME-PRICE'",
        "UPDATE carts SET customer_email = 'other@example.com'",
        "UPDATE carts SET shipping_address = json_set(shipping_address, '$.line1', 'Different destination')",
        "UPDATE carts SET expires_at = '2099-01-01T00:00:00Z'",
    ] {
        let db = SqliteDatabase::in_memory().unwrap();
        create_stock(&db, "KERNEL-CHECKOUT-SKU", dec!(2));
        let id = ready_checkout_cart(&db, "fingerprint@example.com");
        let quoted = db.carts().get(id).unwrap().unwrap().checkout_fingerprint().unwrap();
        db.pool().get().unwrap().execute_batch(mutation).unwrap();
        for mode in [ExecutionMode::Preview, ExecutionMode::Apply] {
            let mut command = checkout_command(&format!("fingerprint-{mode:?}"), id);
            command.mode = mode;
            command.payload.expected_cart_fingerprint = Some(quoted.clone());
            let receipt =
                db.kernel_executor(payment_policy()).execute_commit_checkout(&command).unwrap();
            assert_eq!(receipt.status, ExecutionStatus::Rejected, "{mutation}: {receipt:?}");
            assert!(receipt.error_message.as_deref().unwrap().contains("fingerprint"));
        }
        let conn = db.pool().get().unwrap();
        for table in ["orders", "inventory_reservations", "backorders"] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
                .unwrap();
            assert_eq!(count, 0, "{mutation}: {table}");
        }
    }
}

#[test]
fn kernel_checkout_fingerprint_commits_exact_snapshot_and_replays_after_cart_changes() {
    let db = SqliteDatabase::in_memory().unwrap();
    let id = ready_checkout_cart(&db, "fingerprint-valid@example.com");
    let mut command = checkout_command("fingerprint-valid", id);
    command.mode = ExecutionMode::Apply;
    command.payload.expected_cart_fingerprint =
        Some(db.carts().get(id).unwrap().unwrap().checkout_fingerprint().unwrap());
    let receipt = db.kernel_executor(payment_policy()).execute_commit_checkout(&command).unwrap();
    assert_eq!(receipt.status, ExecutionStatus::Succeeded);
    let replay = db.kernel_executor(payment_policy()).execute_commit_checkout(&command).unwrap();
    assert_eq!(receipt.receipt_id, replay.receipt_id);
    command.payload.expected_cart_fingerprint = Some("sha256:invalid".into());
    assert_eq!(
        db.kernel_executor(payment_policy()).execute_commit_checkout(&command).unwrap().status,
        ExecutionStatus::Rejected
    );
}

#[test]
fn kernel_checkout_fingerprint_is_independent_of_line_query_order() {
    let db = SqliteDatabase::in_memory().unwrap();
    let id = ready_checkout_cart(&db, "fingerprint-order@example.com");
    let mut cart = db.carts().get(id).unwrap().unwrap();
    let mut another = cart.items[0].clone();
    another.id = Uuid::new_v4();
    cart.items.push(another);
    let fingerprint = cart.checkout_fingerprint().unwrap();
    cart.items.reverse();
    assert_eq!(cart.checkout_fingerprint().unwrap(), fingerprint);
    cart.items[0].name = "Substituted description".into();
    assert_ne!(cart.checkout_fingerprint().unwrap(), fingerprint);
}

#[test]
fn kernel_checkout_stock_policy_wire_compatibility() {
    let legacy = serde_json::json!({"cart_id": stateset_core::CartId::new()});
    let command: CommitCheckout = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(command.stock_policy, None);
    // Keep old request hashes stable when the optional field is omitted.
    assert_eq!(serde_json::to_value(command).unwrap(), legacy);
    let invalid = serde_json::json!({"cart_id": stateset_core::CartId::new(), "stock_policy": "ignore_stock"});
    assert!(serde_json::from_value::<CommitCheckout>(invalid).is_err());
}

#[test]
fn kernel_checkout_strict_stock_preview_aggregates_duplicate_skus() {
    let db = SqliteDatabase::in_memory().unwrap();
    create_stock(&db, "KERNEL-CHECKOUT-SKU", dec!(3));
    let cart_id = ready_checkout_cart(&db, "strict-duplicate@example.com");
    db.carts()
        .add_item(
            cart_id,
            AddCartItem {
                product_id: Some(ProductId::new()),
                sku: "KERNEL-CHECKOUT-SKU".into(),
                name: "Another line of the same SKU".into(),
                quantity: 2,
                unit_price: dec!(19.99),
                ..Default::default()
            },
        )
        .unwrap();
    for mode in [ExecutionMode::Preview, ExecutionMode::Apply] {
        let mut command = checkout_command(&format!("strict-duplicate-{mode:?}"), cart_id);
        command.mode = mode;
        command.payload.stock_policy = Some(stateset_core::StockPolicy::RejectIfInsufficient);
        let result =
            db.kernel_executor(payment_policy()).execute_commit_checkout(&command).unwrap();
        assert_eq!(result.status, ExecutionStatus::Rejected, "{result:?}");
        assert_eq!(result.error_code.as_deref(), Some("commerce.inventory.insufficient_available"));
    }
    let conn = db.pool().get().unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM inventory_reservations", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn kernel_checkout_strict_stock_rejects_preview_and_apply_without_effects() {
    let db = SqliteDatabase::in_memory().expect("database");
    create_stock(&db, "KERNEL-CHECKOUT-SKU", dec!(1));
    let cart_id = ready_checkout_cart(&db, "strict-short@example.com");
    for mode in [ExecutionMode::Preview, ExecutionMode::Apply] {
        let mut command = checkout_command(&format!("strict-short-{mode:?}"), cart_id);
        command.mode = mode;
        command.payload.stock_policy = Some(stateset_core::StockPolicy::RejectIfInsufficient);
        let result =
            db.kernel_executor(payment_policy()).execute_commit_checkout(&command).unwrap();
        assert_eq!(result.status, ExecutionStatus::Rejected, "{result:?}");
        let replay =
            db.kernel_executor(payment_policy()).execute_commit_checkout(&command).unwrap();
        assert_eq!(result.receipt_id, replay.receipt_id);
    }
    let conn = db.pool().get().unwrap();
    for table in ["orders", "inventory_reservations", "backorders"] {
        let count: i64 =
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0)).unwrap();
        assert_eq!(count, 0, "unexpected effect in {table}");
    }
}

#[test]
fn kernel_checkout_default_still_backorders_shortages() {
    let db = SqliteDatabase::in_memory().unwrap();
    create_stock(&db, "KERNEL-CHECKOUT-SKU", dec!(1));
    let cart_id = ready_checkout_cart(&db, "legacy-backorder@example.com");
    let mut command = checkout_command("legacy-backorder", cart_id);
    command.mode = ExecutionMode::Apply;
    let result = db.kernel_executor(payment_policy()).execute_commit_checkout(&command).unwrap();
    assert_eq!(result.status, ExecutionStatus::Succeeded);
    assert_eq!(
        db.inventory().get_stock("KERNEL-CHECKOUT-SKU").unwrap().unwrap().total_allocated,
        dec!(1)
    );
    let conn = db.pool().get().unwrap();
    let count: i64 =
        conn.query_row("SELECT COUNT(*) FROM backorders", [], |row| row.get(0)).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn kernel_checkout_strict_stock_concurrent_buyers_cannot_oversell() {
    let db = SqliteDatabase::in_memory().unwrap();
    create_stock(&db, "KERNEL-CHECKOUT-SKU", dec!(3));
    let carts = [
        ready_checkout_cart(&db, "strict-race-a@example.com"),
        ready_checkout_cart(&db, "strict-race-b@example.com"),
    ];
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let executor = db.kernel_executor(payment_policy());
    let handles: Vec<_> = carts
        .into_iter()
        .enumerate()
        .map(|(index, cart_id)| {
            let executor = executor.clone();
            let barrier = barrier.clone();
            let mut command = checkout_command(&format!("strict-race-{index}"), cart_id);
            command.mode = ExecutionMode::Apply;
            command.payload.stock_policy = Some(stateset_core::StockPolicy::RejectIfInsufficient);
            std::thread::spawn(move || {
                barrier.wait();
                let receipt = executor.execute_commit_checkout(&command).unwrap();
                let replay = executor.execute_commit_checkout(&command).unwrap();
                assert_eq!(receipt.receipt_id, replay.receipt_id);
                receipt
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = handles.into_iter().map(|handle| handle.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.status == ExecutionStatus::Succeeded).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| r.error_code.as_deref() == Some("commerce.inventory.insufficient_available"))
            .count(),
        1
    );
    assert_eq!(
        db.inventory().get_stock("KERNEL-CHECKOUT-SKU").unwrap().unwrap().total_allocated,
        dec!(2)
    );
    let conn = db.pool().get().unwrap();
    let orders: i64 = conn.query_row("SELECT COUNT(*) FROM orders", [], |row| row.get(0)).unwrap();
    let backorders: i64 =
        conn.query_row("SELECT COUNT(*) FROM backorders", [], |row| row.get(0)).unwrap();
    assert_eq!((orders, backorders), (1, 0));
}

#[test]
fn kernel_checkout_strict_stock_succeeds_and_policy_changes_conflict() {
    let db = SqliteDatabase::in_memory().unwrap();
    create_stock(&db, "KERNEL-CHECKOUT-SKU", dec!(2));
    let cart_id = ready_checkout_cart(&db, "strict-enough@example.com");
    let mut command = checkout_command("strict-enough", cart_id);
    command.payload.stock_policy = Some(stateset_core::StockPolicy::RejectIfInsufficient);
    assert_eq!(
        db.kernel_executor(payment_policy()).execute_commit_checkout(&command).unwrap().status,
        ExecutionStatus::Previewed
    );
    command.mode = ExecutionMode::Apply;
    command.command_id = Uuid::new_v4();
    let result = db.kernel_executor(payment_policy()).execute_commit_checkout(&command).unwrap();
    assert_eq!(result.status, ExecutionStatus::Succeeded);
    assert_eq!(
        db.kernel_executor(payment_policy()).execute_commit_checkout(&command).unwrap().receipt_id,
        result.receipt_id
    );
    command.payload.stock_policy = Some(stateset_core::StockPolicy::AllowBackorder);
    let changed = db.kernel_executor(payment_policy()).execute_commit_checkout(&command).unwrap();
    assert_eq!(changed.status, ExecutionStatus::Rejected);
    let conn = db.pool().get().unwrap();
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM orders", [], |row| row.get(0)).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn kernel_checkout_preview_applies_and_replays_one_atomic_commit() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let cart_id = ready_checkout_cart(&db, "kernel-checkout@example.com");
    let preview = checkout_command("checkout-commit-1", cart_id);
    let previewed = db
        .kernel_executor(payment_policy())
        .execute_commit_checkout(&preview)
        .expect("preview checkout");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    assert!(previewed.result.is_none());
    assert_eq!(previewed.aggregate_id.as_deref(), Some(cart_id.to_string().as_str()));
    assert_eq!(
        db.carts().get(cart_id).expect("get cart").expect("cart").status,
        stateset_core::CartStatus::Active
    );
    let conn = db.pool().get().expect("connection");
    let orders_after_preview: i64 =
        conn.query_row("SELECT COUNT(*) FROM orders", [], |row| row.get(0)).expect("count orders");
    assert_eq!(orders_after_preview, 0);
    drop(conn);

    let mut apply = preview;
    apply.command_id = Uuid::new_v4();
    apply.mode = ExecutionMode::Apply;
    let applied = db
        .kernel_executor(payment_policy())
        .execute_commit_checkout(&apply)
        .expect("apply checkout");
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    let checkout = applied.result.as_ref().expect("checkout result");
    let order = db.orders().get(checkout.order_id).expect("get order").expect("order");
    assert_eq!(order.status, OrderStatus::Confirmed);
    assert_eq!(order.payment_status, stateset_core::PaymentStatus::Pending);
    assert!(applied.audit_hash.is_some());

    let mut retry = apply.clone();
    retry.command_id = Uuid::new_v4();
    let replay = db
        .kernel_executor(payment_policy())
        .execute_commit_checkout(&retry)
        .expect("replay checkout");
    assert_eq!(replay.receipt_id, applied.receipt_id);
    let conn = db.pool().get().expect("connection");
    let orders: i64 =
        conn.query_row("SELECT COUNT(*) FROM orders", [], |row| row.get(0)).expect("count orders");
    let events: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM kernel_outbox WHERE command_id = ?",
            [apply.command_id.to_string()],
            |row| row.get(0),
        )
        .expect("count events");
    assert_eq!(orders, 1);
    assert_eq!(events, 1);
}

#[test]
fn kernel_checkout_receipt_failure_rolls_back_order_cart_and_event() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let cart_id = ready_checkout_cart(&db, "kernel-checkout-rollback@example.com");
    let conn = db.pool().get().expect("connection");
    conn.execute_batch(
        "CREATE TRIGGER fail_checkout_receipt BEFORE INSERT ON kernel_receipts
         WHEN NEW.command_type = 'checkout.commit'
         BEGIN SELECT RAISE(ABORT, 'forced checkout receipt failure'); END;",
    )
    .expect("create failure trigger");
    drop(conn);
    let mut command = checkout_command("checkout-commit-rollback-1", cart_id);
    command.mode = ExecutionMode::Apply;
    assert!(db.kernel_executor(payment_policy()).execute_commit_checkout(&command).is_err());
    let cart = db.carts().get(cart_id).expect("get cart").expect("cart");
    assert_eq!(cart.status, stateset_core::CartStatus::Active);
    assert!(cart.order_id.is_none());
    let conn = db.pool().get().expect("connection");
    let orders: i64 =
        conn.query_row("SELECT COUNT(*) FROM orders", [], |row| row.get(0)).expect("count orders");
    let events: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM kernel_outbox WHERE command_id = ?",
            [command.command_id.to_string()],
            |row| row.get(0),
        )
        .expect("count events");
    assert_eq!(orders, 0);
    assert_eq!(events, 0);
}

fn subscription_charge_command(
    key: &str,
    billing_cycle_id: Uuid,
) -> CommandEnvelope<ChargeSubscription> {
    let mut command = CommandEnvelope::preview(
        "subscriptions.charge",
        key,
        KernelPrincipal {
            id: "agent:subscription-billing-1".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-1".into()),
            capabilities: vec!["subscriptions.charge".into()],
        },
        ChargeSubscription {
            billing_cycle_id,
            payment_method: PaymentMethodType::CreditCard,
            processor: Some("test-processor".into()),
        },
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

fn ready_billing_cycle(db: &SqliteDatabase, email: &str) -> Uuid {
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: email.into(),
            first_name: "Subscription".into(),
            last_name: "Kernel".into(),
            ..Default::default()
        })
        .expect("create customer");
    let subscriptions = db.subscriptions();
    let plan = subscriptions
        .create_plan(CreateSubscriptionPlan {
            code: None,
            name: "Kernel Monthly".into(),
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
            payment_method_id: Some("pm_kernel".into()),
            skip_trial: Some(true),
            ..Default::default()
        })
        .expect("create subscription");
    subscriptions
        .list_billing_cycles(BillingCycleFilter {
            subscription_id: Some(subscription.id),
            ..Default::default()
        })
        .expect("list billing cycles")
        .into_iter()
        .next()
        .expect("initial billing cycle")
        .id
}

#[test]
fn kernel_subscription_charge_previews_applies_and_replays_pending_collection() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let cycle_id = ready_billing_cycle(&db, "kernel-subscription@example.com");
    let preview = subscription_charge_command("subscription-charge-1", cycle_id);
    let previewed = db
        .kernel_executor(payment_policy())
        .execute_charge_subscription(&preview)
        .expect("preview charge");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    assert_eq!(
        db.subscriptions().get_billing_cycle(cycle_id).expect("load cycle").expect("cycle").status,
        BillingCycleStatus::Scheduled
    );

    let mut apply = preview;
    apply.command_id = Uuid::new_v4();
    apply.mode = ExecutionMode::Apply;
    let applied = db
        .kernel_executor(payment_policy())
        .execute_charge_subscription(&apply)
        .expect("apply charge");
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    let charge = applied.result.as_ref().expect("charge result");
    assert_eq!(charge.billing_cycle.status, BillingCycleStatus::Processing);
    assert_eq!(charge.payment.status, stateset_core::PaymentTransactionStatus::Pending);
    assert_eq!(charge.payment.amount, dec!(29.99));
    assert_eq!(
        charge.billing_cycle.payment_id.as_deref(),
        Some(charge.payment.id.to_string().as_str())
    );

    let mut retry = apply.clone();
    retry.command_id = Uuid::new_v4();
    let replay = db
        .kernel_executor(payment_policy())
        .execute_charge_subscription(&retry)
        .expect("replay charge");
    assert_eq!(replay.receipt_id, applied.receipt_id);
    let conn = db.pool().get().expect("connection");
    let payments: i64 = conn
        .query_row("SELECT COUNT(*) FROM payments", [], |row| row.get(0))
        .expect("count payments");
    let events: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM kernel_outbox WHERE command_id = ?",
            [apply.command_id.to_string()],
            |row| row.get(0),
        )
        .expect("count events");
    assert_eq!(payments, 1);
    assert_eq!(events, 1);
}

#[test]
fn kernel_subscription_receipt_failure_rolls_back_payment_cycle_and_event() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let cycle_id = ready_billing_cycle(&db, "kernel-subscription-rollback@example.com");
    let conn = db.pool().get().expect("connection");
    conn.execute_batch(
        "CREATE TRIGGER fail_subscription_receipt BEFORE INSERT ON kernel_receipts
         WHEN NEW.command_type = 'subscriptions.charge'
         BEGIN SELECT RAISE(ABORT, 'forced subscription receipt failure'); END;",
    )
    .expect("create failure trigger");
    drop(conn);
    let mut command = subscription_charge_command("subscription-charge-rollback-1", cycle_id);
    command.mode = ExecutionMode::Apply;
    assert!(db.kernel_executor(payment_policy()).execute_charge_subscription(&command).is_err());
    let cycle = db.subscriptions().get_billing_cycle(cycle_id).expect("load cycle").expect("cycle");
    assert_eq!(cycle.status, BillingCycleStatus::Scheduled);
    assert!(cycle.payment_id.is_none());
    let conn = db.pool().get().expect("connection");
    let payments: i64 = conn
        .query_row("SELECT COUNT(*) FROM payments", [], |row| row.get(0))
        .expect("count payments");
    let events: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM kernel_outbox WHERE command_id = ?",
            [command.command_id.to_string()],
            |row| row.get(0),
        )
        .expect("count events");
    assert_eq!(payments, 0);
    assert_eq!(events, 0);
}

fn escrow_principal(capability: &str) -> KernelPrincipal {
    KernelPrincipal {
        id: "agent:escrow-operator-1".into(),
        kind: PrincipalKind::Agent,
        tenant_id: Some("tenant-1".into()),
        delegated_by: Some("user-1".into()),
        capabilities: vec![capability.into()],
    }
}

fn scope_escrow_command<T>(mut command: CommandEnvelope<T>) -> CommandEnvelope<T> {
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

#[test]
fn kernel_a2a_escrow_create_fund_and_refund_are_exact_atomic_and_replayable() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let create = scope_escrow_command(CommandEnvelope::preview(
        "a2a.escrow.create",
        "a2a-escrow-create-lifecycle-1",
        escrow_principal("a2a.escrow.create"),
        CreateA2AEscrow {
            quote_id: None,
            payment_id: None,
            buyer_address: "did:key:buyer".into(),
            seller_address: "did:key:seller".into(),
            amount: dec!(123.456789),
            asset: "USDC".into(),
            network: "set_chain".into(),
            release_conditions: vec![serde_json::json!({
                "type": "buyer_confirmed",
                "completed": false
            })],
            expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
            auto_release_after: None,
            metadata: Some(serde_json::json!({"order": "agent-order-1"})),
        },
    ));
    let previewed = db
        .kernel_executor(payment_policy())
        .execute_create_a2a_escrow(&create)
        .expect("preview create");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    let mut apply_create = create;
    apply_create.command_id = Uuid::new_v4();
    apply_create.mode = ExecutionMode::Apply;
    let created = db
        .kernel_executor(payment_policy())
        .execute_create_a2a_escrow(&apply_create)
        .expect("apply create");
    assert_eq!(created.status, ExecutionStatus::Succeeded);
    assert!(created.audit_hash.is_some());
    let escrow = created.result.expect("created escrow");
    assert_eq!(escrow.amount_decimal, dec!(123.456789));
    assert_eq!(escrow.status, stateset_core::A2AEscrowStatus::Created);

    let fund = scope_escrow_command(CommandEnvelope::preview(
        "a2a.escrow.fund",
        "a2a-escrow-fund-lifecycle-1",
        escrow_principal("a2a.escrow.fund"),
        FundA2AEscrow { escrow_id: escrow.id.clone() },
    ));
    let previewed =
        db.kernel_executor(payment_policy()).execute_fund_a2a_escrow(&fund).expect("preview fund");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    assert_eq!(
        previewed.result.expect("fund projection").status,
        stateset_core::A2AEscrowStatus::Active
    );
    let mut apply_fund = fund;
    apply_fund.command_id = Uuid::new_v4();
    apply_fund.mode = ExecutionMode::Apply;
    let funded = db
        .kernel_executor(payment_policy())
        .execute_fund_a2a_escrow(&apply_fund)
        .expect("apply fund");
    assert_eq!(funded.status, ExecutionStatus::Succeeded);

    let mut dispute = scope_escrow_command(CommandEnvelope::preview(
        "a2a.escrow.dispute",
        "a2a-escrow-dispute-lifecycle-1",
        escrow_principal("a2a.escrow.dispute"),
        DisputeA2AEscrow {
            escrow_id: escrow.id.clone(),
            reason: "seller did not provide delivery evidence".into(),
            category: Some("non_delivery".into()),
        },
    ));
    dispute.mode = ExecutionMode::Apply;
    let disputed = db
        .kernel_executor(payment_policy())
        .execute_dispute_a2a_escrow(&dispute)
        .expect("apply dispute");
    assert_eq!(disputed.status, ExecutionStatus::Succeeded);
    assert_eq!(
        disputed.result.expect("disputed escrow").status,
        stateset_core::A2AEscrowStatus::Disputed
    );

    let mut refund = scope_escrow_command(CommandEnvelope::preview(
        "a2a.escrow.refund",
        "a2a-escrow-refund-lifecycle-1",
        escrow_principal("a2a.escrow.refund"),
        RefundA2AEscrow {
            escrow_id: escrow.id.clone(),
            reason: Some("buyer cancelled before fulfillment".into()),
        },
    ));
    refund.mode = ExecutionMode::Apply;
    let refunded = db
        .kernel_executor(payment_policy())
        .execute_refund_a2a_escrow(&refund)
        .expect("apply refund");
    assert_eq!(refunded.status, ExecutionStatus::Succeeded);
    assert_eq!(
        refunded.result.as_ref().expect("refund result").status,
        stateset_core::A2AEscrowStatus::Refunded
    );
    let mut replay_command = refund;
    replay_command.command_id = Uuid::new_v4();
    let replay = db
        .kernel_executor(payment_policy())
        .execute_refund_a2a_escrow(&replay_command)
        .expect("replay refund");
    assert_eq!(replay.receipt_id, refunded.receipt_id);

    let conn = db.pool().get().expect("connection");
    let event_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM kernel_outbox WHERE aggregate_id = ?",
            [&escrow.id],
            |row| row.get(0),
        )
        .expect("count lifecycle events");
    assert_eq!(event_count, 4);
}

fn governed_dispute_command<T>(
    command_type: &str,
    key: &str,
    actor: &str,
    payload: T,
) -> CommandEnvelope<T> {
    let mut command = CommandEnvelope::preview(
        command_type,
        key,
        KernelPrincipal {
            id: actor.into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-1".into()),
            capabilities: vec![command_type.into()],
        },
        payload,
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

#[test]
fn kernel_a2a_formal_dispute_is_scoped_exact_atomic_and_replayable() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let mut create = scope_escrow_command(CommandEnvelope::preview(
        "a2a.escrow.create",
        "a2a-formal-create-1",
        escrow_principal("a2a.escrow.create"),
        CreateA2AEscrow {
            quote_id: None,
            payment_id: None,
            buyer_address: "did:key:buyer".into(),
            seller_address: "did:key:seller".into(),
            amount: dec!(123.456789),
            asset: "USDC".into(),
            network: "set_chain".into(),
            release_conditions: vec![],
            expires_at: chrono::Utc::now() + chrono::Duration::days(30),
            auto_release_after: None,
            metadata: None,
        },
    ));
    create.mode = ExecutionMode::Apply;
    let escrow = db
        .kernel_executor(payment_policy())
        .execute_create_a2a_escrow(&create)
        .expect("create escrow")
        .result
        .expect("escrow result");
    assert_eq!(escrow.tenant_id, "tenant-1");
    assert_eq!(escrow.store_id, "store-1");

    let mut fund = scope_escrow_command(CommandEnvelope::preview(
        "a2a.escrow.fund",
        "a2a-formal-fund-1",
        escrow_principal("a2a.escrow.fund"),
        FundA2AEscrow { escrow_id: escrow.id.clone() },
    ));
    fund.mode = ExecutionMode::Apply;
    db.kernel_executor(payment_policy()).execute_fund_a2a_escrow(&fund).expect("fund escrow");

    let now = chrono::Utc::now();
    let file = governed_dispute_command(
        "a2a.dispute.file",
        "a2a-formal-file-1",
        "did:key:buyer",
        FileA2ADispute {
            escrow_id: escrow.id,
            claimant_address: "did:key:buyer".into(),
            reason: "delivery evidence missing".into(),
            category: "non_delivery".into(),
            evidence_deadline: now + chrono::Duration::days(7),
            review_deadline: now + chrono::Duration::days(14),
            metadata: Some(serde_json::json!({"case": "agent-order-1"})),
        },
    );
    let preview = db
        .kernel_executor(payment_policy())
        .execute_file_a2a_dispute(&file)
        .expect("preview dispute");
    assert_eq!(preview.status, ExecutionStatus::Previewed);
    let mut apply_file = file;
    apply_file.command_id = Uuid::new_v4();
    apply_file.mode = ExecutionMode::Apply;
    let filed = db
        .kernel_executor(payment_policy())
        .execute_file_a2a_dispute(&apply_file)
        .expect("file dispute");
    let dispute = filed.result.expect("dispute result");
    assert_eq!(dispute.amount, dec!(123.456789));
    assert_eq!(dispute.respondent_address, "did:key:seller");

    let mut evidence = governed_dispute_command(
        "a2a.dispute.evidence.submit",
        "a2a-formal-evidence-1",
        "did:key:buyer",
        SubmitA2ADisputeEvidence {
            dispute_id: dispute.id.clone(),
            submitted_by: "did:key:buyer".into(),
            evidence_type: "communication".into(),
            title: "Seller conversation".into(),
            description: None,
            content: "seller acknowledged that delivery did not occur".into(),
        },
    );
    evidence.mode = ExecutionMode::Apply;
    let submitted = db
        .kernel_executor(payment_policy())
        .execute_submit_a2a_dispute_evidence(&evidence)
        .expect("submit evidence");
    assert!(submitted.result.expect("evidence result").content_hash.starts_with("sha256:"));

    let mut cross_tenant = governed_dispute_command(
        "a2a.dispute.resolve",
        "a2a-formal-cross-tenant-1",
        "agent:resolver",
        ResolveA2ADispute {
            dispute_id: dispute.id.clone(),
            resolution_type: A2ADisputeResolutionType::FullRefund,
            buyer_amount: None,
            seller_amount: None,
            note: None,
        },
    );
    cross_tenant.principal.tenant_id = Some("tenant-other".into());
    cross_tenant.mode = ExecutionMode::Apply;
    let denied = db
        .kernel_executor(payment_policy())
        .execute_resolve_a2a_dispute(&cross_tenant)
        .expect("durable cross-tenant denial");
    assert_eq!(denied.status, ExecutionStatus::Rejected);
    assert_eq!(denied.error_code.as_deref(), Some("commerce.a2a.dispute_not_found"));

    let mut resolve = governed_dispute_command(
        "a2a.dispute.resolve",
        "a2a-formal-resolve-1",
        "agent:resolver",
        ResolveA2ADispute {
            dispute_id: dispute.id,
            resolution_type: A2ADisputeResolutionType::Split,
            buyer_amount: Some(dec!(40.000001)),
            seller_amount: Some(dec!(83.456788)),
            note: Some("evidence supports an exact proportional split".into()),
        },
    );
    resolve.mode = ExecutionMode::Apply;
    let resolved = db
        .kernel_executor(payment_policy())
        .execute_resolve_a2a_dispute(&resolve)
        .expect("resolve dispute");
    assert_eq!(resolved.status, ExecutionStatus::Succeeded);
    assert_eq!(resolved.event_ids.len(), 2);
    assert!(resolved.audit_hash.is_some());
    let result = resolved.result.as_ref().expect("resolution result");
    assert_eq!(result.dispute.buyer_amount, Some(dec!(40.000001)));
    assert_eq!(result.dispute.seller_amount, Some(dec!(83.456788)));
    assert_eq!(result.escrow.status, stateset_core::A2AEscrowStatus::Resolved);
    let mut replay_command = resolve;
    replay_command.command_id = Uuid::new_v4();
    let replay = db
        .kernel_executor(payment_policy())
        .execute_resolve_a2a_dispute(&replay_command)
        .expect("replay resolution");
    assert_eq!(replay.receipt_id, resolved.receipt_id);
    assert!(db.kernel_outbox().verify_audit_chain().expect("verify chain").valid);
}

fn release_escrow_command(key: &str, escrow_id: &str) -> CommandEnvelope<ReleaseA2AEscrow> {
    let mut command = CommandEnvelope::preview(
        "a2a.escrow.release",
        key,
        KernelPrincipal {
            id: "agent:escrow-release-1".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-1".into()),
            capabilities: vec!["a2a.escrow.release".into()],
        },
        ReleaseA2AEscrow { escrow_id: escrow_id.into() },
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

fn seed_releasable_escrow(db: &SqliteDatabase, escrow_id: &str, conditions_met: bool) {
    let now = chrono::Utc::now();
    let quote_id = format!("quote-{escrow_id}");
    let conn = db.pool().get().expect("connection");
    conn.execute(
        "INSERT INTO a2a_quotes (
            id, quote_number, status, buyer_agent_id, seller_agent_id, items,
            subtotal, tax_amount, shipping_amount, discount_amount, total, currency,
            valid_until, created_at, updated_at
         ) VALUES (?, ?, 'fulfilled', '0xbuyer', '0xseller', '[]',
                   '123.45', '0', '0', '0', '123.45', 'USD', ?, ?, ?)",
        rusqlite::params![
            &quote_id,
            format!("QUOTE-{escrow_id}"),
            (now + chrono::Duration::hours(24)).to_rfc3339(),
            now.to_rfc3339(),
            now.to_rfc3339(),
        ],
    )
    .expect("seed quote");
    let conditions = serde_json::json!([
        {"type": "seller_fulfilled", "quoteId": quote_id},
        {"type": "buyer_confirmed", "completed": conditions_met},
        {"type": "time_lock", "releaseAfter": (now - chrono::Duration::minutes(1)).to_rfc3339()},
        {"type": "milestone", "description": "delivered", "completed": conditions_met}
    ]);
    conn.execute(
        "INSERT INTO a2a_escrows (
            id, status, quote_id, buyer_address, seller_address, amount, amount_decimal,
            asset, network, release_conditions, funded_at, expires_at, created_at, updated_at,
            tenant_id, store_id
         ) VALUES (?, 'active', ?, '0xbuyer', '0xseller', 123450000, '123.45',
                   'USDC', 'set_chain', ?, ?, ?, ?, ?, 'tenant-1', 'store-1')",
        rusqlite::params![
            escrow_id,
            format!("quote-{escrow_id}"),
            conditions.to_string(),
            now.to_rfc3339(),
            (now + chrono::Duration::hours(24)).to_rfc3339(),
            now.to_rfc3339(),
            now.to_rfc3339(),
        ],
    )
    .expect("seed escrow");
}

#[test]
fn kernel_a2a_escrow_release_validates_conditions_previews_applies_and_replays() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let escrow_id = format!("escrow-{}", Uuid::new_v4());
    seed_releasable_escrow(&db, &escrow_id, true);
    let preview = release_escrow_command("a2a-escrow-release-1", &escrow_id);
    let previewed = db
        .kernel_executor(payment_policy())
        .execute_release_a2a_escrow(&preview)
        .expect("preview release");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    assert_eq!(
        previewed.result.as_ref().expect("projection").status,
        stateset_core::A2AEscrowStatus::Released
    );
    let conn = db.pool().get().expect("connection");
    let status: String = conn
        .query_row("SELECT status FROM a2a_escrows WHERE id = ?", [&escrow_id], |row| row.get(0))
        .expect("load status");
    assert_eq!(status, "active");
    drop(conn);

    let mut apply = preview;
    apply.command_id = Uuid::new_v4();
    apply.mode = ExecutionMode::Apply;
    let applied = db
        .kernel_executor(payment_policy())
        .execute_release_a2a_escrow(&apply)
        .expect("apply release");
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    assert_eq!(applied.result.as_ref().expect("escrow").amount_decimal, dec!(123.45));
    assert!(applied.audit_hash.is_some());
    let mut retry = apply.clone();
    retry.command_id = Uuid::new_v4();
    let replay = db
        .kernel_executor(payment_policy())
        .execute_release_a2a_escrow(&retry)
        .expect("replay release");
    assert_eq!(replay.receipt_id, applied.receipt_id);
    let conn = db.pool().get().expect("connection");
    let released: (String, Option<String>) = conn
        .query_row(
            "SELECT status, released_at FROM a2a_escrows WHERE id = ?",
            [&escrow_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("load escrow");
    let events: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM kernel_outbox WHERE command_id = ?",
            [apply.command_id.to_string()],
            |row| row.get(0),
        )
        .expect("count events");
    assert_eq!(released.0, "released");
    assert!(released.1.is_some());
    assert_eq!(events, 1);
}

#[test]
fn kernel_a2a_escrow_unmet_conditions_are_durable_and_non_mutating() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let escrow_id = format!("escrow-{}", Uuid::new_v4());
    seed_releasable_escrow(&db, &escrow_id, false);
    let mut command = release_escrow_command("a2a-escrow-unmet-1", &escrow_id);
    command.mode = ExecutionMode::Apply;
    let rejected = db
        .kernel_executor(payment_policy())
        .execute_release_a2a_escrow(&command)
        .expect("durable rejection");
    assert_eq!(rejected.status, ExecutionStatus::Rejected);
    assert_eq!(rejected.error_code.as_deref(), Some("commerce.a2a.escrow_conditions_unmet"));
    assert!(rejected.audit_hash.is_some());
    let conn = db.pool().get().expect("connection");
    let status: String = conn
        .query_row("SELECT status FROM a2a_escrows WHERE id = ?", [&escrow_id], |row| row.get(0))
        .expect("load status");
    let events: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM kernel_outbox WHERE command_id = ?",
            [command.command_id.to_string()],
            |row| row.get(0),
        )
        .expect("count events");
    assert_eq!(status, "active");
    assert_eq!(events, 0);
}
