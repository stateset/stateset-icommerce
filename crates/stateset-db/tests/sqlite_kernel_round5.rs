#![cfg(feature = "sqlite")]
//! Kernel executor round 5 (SQLite): the cancel money rule inside the
//! kernel, verified replay of sealed receipts, the shared envelope guard
//! chain, and the aligned `payments.create` key-mismatch guard. Every
//! scenario has a Postgres mirror in `postgres_kernel_round5.rs`.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    ApprovalEvidence, CommandEnvelope, CommerceError, CreateCustomer, CreateInventoryItem,
    CreateOrder, CreateOrderItem, CreatePayment, CustomerRepository, ExecutionMode,
    ExecutionStatus, InventoryRepository, KernelCommandPolicy, KernelPolicy, KernelPrincipal,
    OrderId, OrderRepository, OrderStatus, Payment, PaymentMethodType, PaymentRepository,
    PaymentTransactionStatus, PrincipalKind, ProductId, RetryDisposition, TransitionOrder,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

fn db() -> SqliteDatabase {
    SqliteDatabase::in_memory().expect("create in-memory sqlite db")
}

fn policy() -> KernelPolicy {
    KernelPolicy::new("commerce-policy-1")
        .allow("payments.create", KernelCommandPolicy::requiring(["payments.create"]))
        .allow("orders.transition", KernelCommandPolicy::requiring(["orders.transition"]))
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

fn command<C>(command_type: &str, key: &str, payload: C) -> CommandEnvelope<C> {
    let mut command = CommandEnvelope::preview(command_type, key, principal(command_type), payload);
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

fn payment_command(key: &str, amount: Decimal) -> CommandEnvelope<CreatePayment> {
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
    key: &str,
    order_id: OrderId,
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

fn order_totalling(db: &SqliteDatabase, unit_price: Decimal) -> OrderId {
    let sku = format!("R5-{}", Uuid::new_v4());
    db.inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: sku.clone(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .expect("create inventory item");
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: format!("round5-{}@example.com", Uuid::new_v4()),
            first_name: "Round".into(),
            last_name: "Five".into(),
            ..Default::default()
        })
        .expect("create customer");
    db.orders()
        .create(CreateOrder {
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
        .expect("create order")
        .id
}

fn payment(db: &SqliteDatabase, order_id: OrderId, amount: Decimal) -> Payment {
    db.payments()
        .create(CreatePayment {
            order_id: Some(order_id),
            payment_method: PaymentMethodType::CreditCard,
            amount,
            ..Default::default()
        })
        .expect("create payment")
}

fn status(db: &SqliteDatabase, id: stateset_core::PaymentId) -> PaymentTransactionStatus {
    db.payments().get(id).expect("get payment").expect("payment exists").status
}

fn count(db: &SqliteDatabase, sql: &str) -> i64 {
    db.pool().get().expect("connection").query_row(sql, [], |row| row.get(0)).expect("count")
}

// ---------------------------------------------------------------------------
// Finding 3 — the kernel cancel path honours the order money rule
// ---------------------------------------------------------------------------

#[test]
fn kernel_cancel_is_refused_while_captured_money_is_outstanding() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00));
    let captured = payment(&db, order_id, dec!(60.00));
    db.payments().mark_completed(captured.id).expect("complete");

    // Preview refuses exactly what apply refuses.
    let mut preview = transition_command("r5-cancel-preview", order_id, OrderStatus::Cancelled);
    preview.mode = ExecutionMode::Preview;
    let previewed =
        db.kernel_executor(policy()).execute_transition_order(&preview).expect("preview");
    assert_eq!(previewed.status, ExecutionStatus::Rejected);
    assert_eq!(previewed.error_code.as_deref(), Some("commerce.order.captured_money_outstanding"));

    let cancel = transition_command("r5-cancel-apply", order_id, OrderStatus::Cancelled);
    let receipt = db.kernel_executor(policy()).execute_transition_order(&cancel).expect("receipt");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("commerce.order.captured_money_outstanding"));
    assert_eq!(receipt.retry, RetryDisposition::Never);
    let order = db.orders().get(order_id).expect("get").expect("order");
    assert_eq!(receipt.version_before, Some(order.version));
    assert!(receipt.error_message.as_deref().unwrap_or_default().contains("60.00 USD"));
    assert!(receipt.audit_hash.is_some(), "rejection is sealed");

    assert_eq!(order.status, OrderStatus::Pending, "no mutation");
    assert_eq!(status(&db, captured.id), PaymentTransactionStatus::Completed);
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM inventory_reservations WHERE status = 'released'"),
        0,
        "holds are not released by a refused cancel"
    );

    // The retry key is bound to the rejection.
    let mut retry = cancel;
    retry.command_id = Uuid::new_v4();
    let replay = db.kernel_executor(policy()).execute_transition_order(&retry).expect("replay");
    assert_eq!(replay.receipt_id, receipt.receipt_id);
}

#[test]
fn kernel_forced_cancel_voids_in_flight_payments_and_reports_settled_money() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00));
    let settled = payment(&db, order_id, dec!(60.00));
    db.payments().mark_completed(settled.id).expect("complete");
    let in_flight = payment(&db, order_id, dec!(30.00));

    let mut cancel = transition_command("r5-forced-cancel", order_id, OrderStatus::Cancelled);
    cancel.payload.void_payments = true;
    let receipt = db.kernel_executor(policy()).execute_transition_order(&cancel).expect("receipt");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{receipt:?}");
    assert_eq!(receipt.result.as_ref().expect("order").status, OrderStatus::Cancelled);
    assert_eq!(status(&db, in_flight.id), PaymentTransactionStatus::Cancelled, "voided");
    assert_eq!(status(&db, settled.id), PaymentTransactionStatus::Completed, "left for refund");

    let event = db
        .kernel_outbox()
        .pending(100)
        .expect("pending")
        .into_iter()
        .find(|e| e.event_type == "orders.updated.v1" && e.command_id == Some(cancel.command_id))
        .expect("orders.updated.v1");
    assert_eq!(event.payload["void_payments"], true);
    assert_eq!(event.payload["voided_payment_ids"], serde_json::json!([in_flight.id.to_string()]));
    assert_eq!(
        event.payload["outstanding_payment_ids"],
        serde_json::json!([settled.id.to_string()])
    );
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM inventory_reservations WHERE status = 'released'"),
        1,
        "holds are released with the cancel"
    );
}

#[test]
fn void_payments_false_does_not_change_the_semantic_request_hash() {
    let db = db();
    let order_id = order_totalling(&db, dec!(10.00));
    let confirm = transition_command("r5-hash-stable", order_id, OrderStatus::Confirmed);
    let first = db.kernel_executor(policy()).execute_transition_order(&confirm).expect("apply");
    assert_eq!(first.status, ExecutionStatus::Succeeded);
    // A retry from a client that omits the new field entirely still replays.
    let mut json = serde_json::to_value(&confirm).expect("serialize");
    json["command_id"] = serde_json::json!(Uuid::new_v4());
    assert!(json["payload"].get("void_payments").is_none(), "false is omitted on the wire");
    let retry: CommandEnvelope<TransitionOrder> = serde_json::from_value(json).expect("decode");
    let replay = db.kernel_executor(policy()).execute_transition_order(&retry).expect("replay");
    assert_eq!(replay.receipt_id, first.receipt_id);
}

// ---------------------------------------------------------------------------
// Finding 2 — replay verifies the sealed receipt before trusting it
// ---------------------------------------------------------------------------

#[test]
fn replay_refuses_a_tampered_materialized_receipt() {
    let db = db();
    let command = payment_command("r5-tamper-row", dec!(12.34));
    let receipt = db.kernel_executor(policy()).execute_create_payment(&command).expect("apply");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded);

    db.pool()
        .get()
        .expect("connection")
        .execute(
            "UPDATE kernel_receipts SET receipt = json_set(receipt, '$.status', 'rejected')
             WHERE idempotency_key = ?",
            [&command.idempotency_key],
        )
        .expect("tamper");
    let mut retry = command;
    retry.command_id = Uuid::new_v4();
    let error = db
        .kernel_executor(policy())
        .execute_create_payment(&retry)
        .expect_err("tampered receipt must not replay");
    assert!(
        matches!(&error, CommerceError::KernelReceiptTampered { idempotency_key, .. }
            if idempotency_key == "r5-tamper-row"),
        "got {error:?}"
    );
    assert!(error.to_string().contains("kernel.receipt_tampered"));
    assert_eq!(count(&db, "SELECT COUNT(*) FROM payments"), 1, "no second payment");
}

#[test]
fn replay_refuses_a_receipt_whose_audit_entry_was_rewritten() {
    let db = db();
    let command = payment_command("r5-tamper-audit", dec!(5.00));
    db.kernel_executor(policy()).execute_create_payment(&command).expect("apply");
    db.pool()
        .get()
        .expect("connection")
        .execute(
            "UPDATE kernel_receipt_audit_log SET request_hash = 'rebound'
             WHERE idempotency_key = ?",
            [&command.idempotency_key],
        )
        .expect("tamper audit");
    let mut retry = command;
    retry.command_id = Uuid::new_v4();
    let error = db
        .kernel_executor(policy())
        .execute_create_payment(&retry)
        .expect_err("rebound audit entry must not replay");
    assert!(matches!(error, CommerceError::KernelReceiptTampered { .. }), "got {error:?}");
}

#[test]
fn replay_refuses_a_receipt_with_no_audit_entry() {
    let db = db();
    let command = payment_command("r5-tamper-unsealed", dec!(5.00));
    db.kernel_executor(policy()).execute_create_payment(&command).expect("apply");
    db.pool()
        .get()
        .expect("connection")
        .execute(
            "DELETE FROM kernel_receipt_audit_log WHERE idempotency_key = ?",
            [&command.idempotency_key],
        )
        .expect("drop audit entry");
    let mut retry = command;
    retry.command_id = Uuid::new_v4();
    let error = db
        .kernel_executor(policy())
        .execute_create_payment(&retry)
        .expect_err("unsealed receipt must not replay");
    assert!(matches!(error, CommerceError::KernelReceiptTampered { .. }), "got {error:?}");
}

#[test]
fn intact_receipts_still_replay_after_verification() {
    let db = db();
    let command = payment_command("r5-intact", dec!(7.00));
    let first = db.kernel_executor(policy()).execute_create_payment(&command).expect("apply");
    let mut retry = command;
    retry.command_id = Uuid::new_v4();
    let replay = db.kernel_executor(policy()).execute_create_payment(&retry).expect("replay");
    assert_eq!(replay.receipt_id, first.receipt_id);
    assert_eq!(replay.audit_hash, first.audit_hash);
}

// ---------------------------------------------------------------------------
// Envelope guards (shared chain) and the aligned key-mismatch guard
// ---------------------------------------------------------------------------

#[test]
fn envelope_guard_rejects_command_type_mismatch_durably() {
    let db = db();
    let mut command = payment_command("r5-type-mismatch", dec!(1.00));
    command.command_type = "orders.transition".into();
    command.principal.capabilities = vec!["orders.transition".into()];
    let receipt = db.kernel_executor(policy()).execute_create_payment(&command).expect("receipt");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.command_type_mismatch"));
    assert_eq!(receipt.error_message.as_deref(), Some("expected payments.create command type"));
    assert_eq!(receipt.retry, RetryDisposition::Never);
    assert!(receipt.policy.is_some());
    assert_eq!(count(&db, "SELECT COUNT(*) FROM kernel_receipts"), 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM payments"), 0);
}

#[test]
fn envelope_guard_rejects_actor_mismatch() {
    let db = db();
    let mut self_delegated = payment_command("r5-self-delegated", dec!(1.00));
    self_delegated.principal.delegated_by = Some(self_delegated.principal.id.clone());
    let receipt =
        db.kernel_executor(policy()).execute_create_payment(&self_delegated).expect("receipt");
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.actor_mismatch"));

    let mut self_approved = payment_command("r5-self-approved", dec!(1.00));
    self_approved.approval = Some(ApprovalEvidence {
        approval_id: "approval-1".into(),
        approved_by: self_approved.principal.id.clone(),
        scope: "payments.create".into(),
        tenant_id: self_approved.principal.tenant_id.clone(),
        store_id: self_approved.store_id.clone(),
        idempotency_key: Some(self_approved.idempotency_key.clone()),
        approved_at: chrono::Utc::now(),
        expires_at: None,
    });
    let receipt =
        db.kernel_executor(policy()).execute_create_payment(&self_approved).expect("receipt");
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.actor_mismatch"));
    assert_eq!(count(&db, "SELECT COUNT(*) FROM payments"), 0);

    let order_id = order_totalling(&db, dec!(1.00));
    let mut transition = transition_command("r5-actor-order", order_id, OrderStatus::Confirmed);
    transition.principal.delegated_by = Some(transition.principal.id.clone());
    let receipt =
        db.kernel_executor(policy()).execute_transition_order(&transition).expect("receipt");
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.actor_mismatch"));
}

#[test]
fn envelope_guard_rejects_expected_version_on_create_commands() {
    let db = db();
    let mut command = payment_command("r5-expected-version", dec!(1.00));
    command.expected_version = Some(1);
    let receipt = db.kernel_executor(policy()).execute_create_payment(&command).expect("receipt");
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.expected_version_not_applicable"));
    assert_eq!(receipt.retry, RetryDisposition::Never);

    // Aggregate commands honour it instead.
    let order_id = order_totalling(&db, dec!(1.00));
    let mut transition = transition_command("r5-version-ok", order_id, OrderStatus::Confirmed);
    transition.expected_version = Some(1);
    let receipt =
        db.kernel_executor(policy()).execute_transition_order(&transition).expect("receipt");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded);
}

#[test]
fn payment_key_mismatch_guard_is_a_sealed_rejection_with_policy_evidence() {
    let db = db();
    let mut command = payment_command("r5-key-mismatch", dec!(1.00));
    command.payload.idempotency_key = Some("someone-elses-key".into());
    let receipt = db.kernel_executor(policy()).execute_create_payment(&command).expect("receipt");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.idempotency_key_mismatch"));
    assert_eq!(receipt.retry, RetryDisposition::Never);
    assert_eq!(receipt.aggregate_type.as_deref(), Some("payment"));
    let evidence = receipt.policy.expect("policy evidence is recorded");
    assert!(evidence.allowed);
    assert!(receipt.audit_hash.is_some());
    assert_eq!(count(&db, "SELECT COUNT(*) FROM payments"), 0);

    // The key is bound to the rejection, not to a later corrected attempt.
    let mut corrected = command;
    corrected.command_id = Uuid::new_v4();
    corrected.payload.idempotency_key = None;
    let replay = db.kernel_executor(policy()).execute_create_payment(&corrected).expect("replay");
    assert_eq!(replay.error_code.as_deref(), Some("kernel.idempotency_conflict"));
}

/// `payments.create` answered "Previewed" before it ran
/// `check_order_capture_capacity_*`, so a preview promised a capture that
/// apply would refuse — the one thing a preview must never do. The capacity
/// check now runs on both paths, so a preview that cannot be applied fails
/// exactly the way applying it would.
#[test]
fn payment_preview_runs_every_check_apply_runs() {
    let db = db();
    let order_id = order_totalling(&db, dec!(100.00));

    // Take the whole order total with a completed capture.
    let taken = payment(&db, order_id, dec!(100.00));
    db.payments().mark_completed(taken.id).expect("complete the capture");

    // A preview of a second full capture must not answer "Previewed".
    let mut preview = payment_command("preview-over-capture-1", dec!(100.00));
    preview.payload.order_id = Some(order_id);
    preview.mode = ExecutionMode::Preview;
    let previewed = db.kernel_executor(policy()).execute_create_payment(&preview);
    let error = previewed.expect_err("preview must refuse what apply would refuse");
    assert_eq!(error.invariant_code(), Some("commerce.capture.exceeds_order_total"), "{error:?}");

    // Applying the same command fails identically.
    let mut apply = preview;
    apply.command_id = Uuid::new_v4();
    apply.idempotency_key = "preview-over-capture-2".into();
    apply.mode = ExecutionMode::Apply;
    let applied = db.kernel_executor(policy()).execute_create_payment(&apply);
    assert_eq!(
        applied.expect_err("apply refuses").invariant_code(),
        Some("commerce.capture.exceeds_order_total")
    );

    // A preview that *is* within capacity still previews and mutates nothing.
    let before = count(&db, "SELECT COUNT(*) FROM payments");
    let mut ok = payment_command("preview-within-capacity-1", dec!(10.00));
    ok.payload.order_id = Some(order_totalling(&db, dec!(50.00)));
    ok.mode = ExecutionMode::Preview;
    let receipt =
        db.kernel_executor(policy()).execute_create_payment(&ok).expect("preview within capacity");
    assert_eq!(receipt.status, ExecutionStatus::Previewed);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM payments"), before);
}
