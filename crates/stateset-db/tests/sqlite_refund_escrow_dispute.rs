//! A payment held by an A2A escrow under an open dispute cannot be refunded
//! through `payments.create_refund`; the dispute resolution path settles the
//! escrow instead.
#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_core::{
    CommandEnvelope, CreatePayment, CreateRefund, CurrencyCode, ExecutionMode, ExecutionStatus,
    KernelCommandPolicy, KernelPolicy, KernelPrincipal, PaymentMethodType, PaymentRepository,
    PrincipalKind,
};
use stateset_db::SqliteDatabase;

fn policy() -> KernelPolicy {
    [
        "payments.create_refund",
        "a2a.escrow.create",
        "a2a.escrow.fund",
        "a2a.escrow.refund",
        "a2a.dispute.file",
    ]
    .into_iter()
    .fold(KernelPolicy::new("commerce-policy-1"), |policy, command| {
        policy.allow(command, KernelCommandPolicy::requiring([command]))
    })
}

fn governed<T>(command_type: &str, key: &str, actor: &str, payload: T) -> CommandEnvelope<T> {
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
    command.mode = ExecutionMode::Apply;
    command
}

fn refund_command(
    key: &str,
    payment_id: stateset_core::PaymentId,
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
            amount: Some(dec!(1.00)),
            reason: Some("buyer asked".into()),
            ..Default::default()
        },
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command.mode = ExecutionMode::Apply;
    command
}

fn completed_payment(db: &SqliteDatabase) -> stateset_core::Payment {
    let payment = db
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(10.00),
            currency: Some(CurrencyCode::USD),
            ..Default::default()
        })
        .expect("create payment");
    db.payments().mark_completed(payment.id).expect("complete payment")
}

fn insert_escrow(
    db: &SqliteDatabase,
    payment_id: &str,
    status: &str,
    dispute_id: Option<&str>,
) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    db.conn()
        .unwrap()
        .execute(
            "INSERT INTO a2a_escrows (id, status, payment_id, buyer_address, seller_address, amount,
                amount_decimal, asset, network, release_conditions, dispute_id, expires_at,
                created_at, updated_at)
             VALUES (?, ?, ?, '0xbuyer', '0xseller', 1000000, '1', 'USDC', 'set_chain', '[]', ?, ?, ?, ?)",
            rusqlite::params![id, status, payment_id, dispute_id, now, now, now],
        )
        .expect("insert escrow");
    id
}

fn insert_open_dispute(db: &SqliteDatabase, escrow_id: &str) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    db.conn()
        .unwrap()
        .execute(
            "INSERT INTO a2a_disputes (id, tenant_id, store_id, status, escrow_id, claimant_address,
                respondent_address, reason, category, amount_decimal, asset, evidence_deadline,
                review_deadline, created_at, updated_at)
             VALUES (?, 'tenant-1', 'store-1', 'filed', ?, '0xbuyer', '0xseller', 'not delivered',
                'non_delivery', '1', 'USDC', ?, ?, ?, ?)",
            rusqlite::params![id, escrow_id, now, now, now, now],
        )
        .expect("insert dispute");
    id
}

#[test]
fn sqlite_refund_is_refused_while_escrow_is_disputed() {
    let db = SqliteDatabase::in_memory().expect("db");
    let payment = completed_payment(&db);
    let escrow_id = insert_escrow(&db, &payment.id.to_string(), "disputed", None);

    let receipt = db
        .kernel_executor(policy())
        .execute_create_refund(&refund_command("refund-disputed", payment.id))
        .expect("receipt");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("commerce.refund.escrow_disputed"));
    let message = receipt.error_message.clone().unwrap_or_default();
    assert!(message.contains(&escrow_id), "{message}");
    assert!(receipt.result.is_none());
    assert!(db.payments().get_refunds(payment.id).expect("refunds").is_empty());
    let stored = db.payments().get(payment.id).unwrap().unwrap();
    assert_eq!(stored.amount_refunded, dec!(0));
}

#[test]
fn sqlite_refund_is_refused_while_a_filed_dispute_is_open_and_allowed_after_resolution() {
    let db = SqliteDatabase::in_memory().expect("db");
    let payment = completed_payment(&db);
    // Escrow frozen through the formal dispute path: escrow row still `active`
    // but a filed dispute references it.
    let escrow_id = insert_escrow(&db, &payment.id.to_string(), "active", None);
    let dispute_id = insert_open_dispute(&db, &escrow_id);

    let receipt = db
        .kernel_executor(policy())
        .execute_create_refund(&refund_command("refund-filed", payment.id))
        .expect("receipt");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("commerce.refund.escrow_disputed"));
    assert!(receipt.error_message.as_deref().unwrap_or_default().contains(&dispute_id));

    // Resolve the dispute (release to seller): the escrow is no longer frozen
    // and a fresh refund command goes through.
    db.conn()
        .unwrap()
        .execute("UPDATE a2a_disputes SET status = 'resolved' WHERE id = ?", [&dispute_id])
        .unwrap();
    db.conn()
        .unwrap()
        .execute("UPDATE a2a_escrows SET status = 'released' WHERE id = ?", [&escrow_id])
        .unwrap();
    let receipt = db
        .kernel_executor(policy())
        .execute_create_refund(&refund_command("refund-after-resolution", payment.id))
        .expect("receipt");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{receipt:?}");
}

/// Refunding a disputed escrow through the kernel returns everything to the
/// buyer — the outcome `a2a.dispute.resolve` records as `full_refund` — so the
/// open dispute is resolved that way in the same transaction. It used to stay
/// open, which froze the payment for good: `payments.create_refund` refuses
/// while a dispute is open, and the dispute could no longer be resolved
/// because its escrow was no longer `disputed`. Twin of the Postgres
/// `postgres_refunding_a_disputed_escrow_resolves_its_dispute_as_a_full_refund`.
#[test]
fn sqlite_refunding_a_disputed_escrow_resolves_its_dispute_as_a_full_refund() {
    use stateset_core::{
        A2ADisputeResolutionType, A2ADisputeStatus, A2AEscrowStatus, CreateA2AEscrow,
        FileA2ADispute, FundA2AEscrow, RefundA2AEscrow,
    };
    let db = SqliteDatabase::in_memory().expect("db");
    let payment = completed_payment(&db);
    let kernel = db.kernel_executor(policy());
    let escrow = kernel
        .execute_create_a2a_escrow(&governed(
            "a2a.escrow.create",
            "dispute-refund-create",
            "agent:escrow",
            CreateA2AEscrow {
                quote_id: None,
                payment_id: Some(payment.id.to_string()),
                buyer_address: "did:key:buyer".into(),
                seller_address: "did:key:seller".into(),
                amount: dec!(10.00),
                asset: "USDC".into(),
                network: "set_chain".into(),
                release_conditions: vec![],
                expires_at: chrono::Utc::now() + chrono::Duration::days(30),
                auto_release_after: None,
                metadata: None,
            },
        ))
        .expect("create escrow")
        .result
        .expect("escrow");
    let funded = kernel
        .execute_fund_a2a_escrow(&governed(
            "a2a.escrow.fund",
            "dispute-refund-fund",
            "agent:escrow",
            FundA2AEscrow { escrow_id: escrow.id.clone() },
        ))
        .expect("fund escrow");
    assert_eq!(funded.status, ExecutionStatus::Succeeded, "{funded:?}");
    let now = chrono::Utc::now();
    let dispute = kernel
        .execute_file_a2a_dispute(&governed(
            "a2a.dispute.file",
            "dispute-refund-file",
            "did:key:buyer",
            FileA2ADispute {
                escrow_id: escrow.id.clone(),
                claimant_address: "did:key:buyer".into(),
                reason: "never delivered".into(),
                category: "non_delivery".into(),
                evidence_deadline: now + chrono::Duration::days(7),
                review_deadline: now + chrono::Duration::days(14),
                metadata: None,
            },
        ))
        .expect("file dispute")
        .result
        .expect("dispute");

    let refunded = kernel
        .execute_refund_a2a_escrow(&governed(
            "a2a.escrow.refund",
            "dispute-refund-refund",
            "agent:escrow",
            RefundA2AEscrow { escrow_id: escrow.id, reason: Some("buyer refunded".into()) },
        ))
        .expect("refund escrow");
    assert_eq!(refunded.status, ExecutionStatus::Succeeded, "{refunded:?}");
    assert_eq!(refunded.result.as_ref().expect("escrow").status, A2AEscrowStatus::Refunded);
    assert_eq!(refunded.event_ids.len(), 2, "the escrow fact and the dispute fact");

    #[allow(clippy::type_complexity)]
    let (status, resolution, buyer, seller, resolved_by): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = db
        .conn()
        .unwrap()
        .query_row(
            "SELECT status, resolution_type, buyer_amount_decimal, seller_amount_decimal,
                    resolved_by
             FROM a2a_disputes WHERE id = ?",
            [&dispute.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .unwrap();
    assert_eq!(status, A2ADisputeStatus::Resolved.to_string());
    assert_eq!(resolution, Some(A2ADisputeResolutionType::FullRefund.to_string()));
    assert_eq!(
        buyer.as_deref().map(|value| value.parse::<rust_decimal::Decimal>().unwrap()),
        Some(dec!(10))
    );
    assert_eq!(
        seller.as_deref().map(|value| value.parse::<rust_decimal::Decimal>().unwrap()),
        Some(dec!(0))
    );
    assert_eq!(resolved_by.as_deref(), Some("agent:escrow"));

    // The payment is no longer frozen.
    let receipt = db
        .kernel_executor(policy())
        .execute_create_refund(&refund_command("refund-after-escrow-refund", payment.id))
        .expect("receipt");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{receipt:?}");
    assert!(db.kernel_outbox().verify_audit_chain().expect("verify chain").valid);
}

#[test]
fn sqlite_refund_proceeds_for_payment_without_escrow_or_with_settled_escrow() {
    let db = SqliteDatabase::in_memory().expect("db");
    let payment = completed_payment(&db);
    insert_escrow(&db, &payment.id.to_string(), "released", None);
    let receipt = db
        .kernel_executor(policy())
        .execute_create_refund(&refund_command("refund-released", payment.id))
        .expect("receipt");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{receipt:?}");
}
