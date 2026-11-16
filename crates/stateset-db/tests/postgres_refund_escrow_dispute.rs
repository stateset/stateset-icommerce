//! Postgres twin of `sqlite_refund_escrow_dispute.rs`: a payment held by an
//! A2A escrow under an open dispute cannot be refunded directly.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.
#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    CommandEnvelope, CreatePayment, CreateRefund, CurrencyCode, ExecutionMode, ExecutionStatus,
    KernelCommandPolicy, KernelPolicy, KernelPrincipal, PaymentMethodType, PrincipalKind,
};
use stateset_db::PostgresDatabase;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    stateset_test_utils::postgres_url()
}

fn policy() -> KernelPolicy {
    [
        "payments.create_refund",
        "a2a.escrow.create",
        "a2a.escrow.fund",
        "a2a.escrow.refund",
        "a2a.dispute.file",
        "a2a.dispute.resolve",
    ]
    .into_iter()
    .fold(KernelPolicy::new("commerce-policy-1"), |policy, command| {
        policy.allow(command, KernelCommandPolicy::requiring([command]))
    })
}

fn governed<T>(command_type: &str, actor: &str, payload: T) -> CommandEnvelope<T> {
    let mut command = CommandEnvelope::preview(
        command_type,
        format!("{command_type}-{}", Uuid::new_v4()),
        KernelPrincipal {
            id: actor.into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-pg".into()),
            delegated_by: Some("user-pg".into()),
            capabilities: vec![command_type.into()],
        },
        payload,
    );
    command.store_id = Some("store-pg".into());
    command.policy_version = Some("commerce-policy-1".into());
    command.mode = ExecutionMode::Apply;
    command
}

fn refund_command(payment_id: stateset_core::PaymentId) -> CommandEnvelope<CreateRefund> {
    let mut command = CommandEnvelope::preview(
        "payments.create_refund",
        format!("refund-{}", Uuid::new_v4()),
        KernelPrincipal {
            id: "agent:refunds-pg".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-pg".into()),
            delegated_by: Some("user-pg".into()),
            capabilities: vec!["payments.create_refund".into()],
        },
        CreateRefund {
            payment_id,
            amount: Some(dec!(1.00)),
            reason: Some("buyer asked".into()),
            ..Default::default()
        },
    );
    command.store_id = Some("store-pg".into());
    command.policy_version = Some("commerce-policy-1".into());
    command.mode = ExecutionMode::Apply;
    command
}

async fn completed_payment(db: &PostgresDatabase) -> stateset_core::Payment {
    let payment = db
        .payments()
        .create_async(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(10.00),
            currency: Some(CurrencyCode::USD),
            ..Default::default()
        })
        .await
        .expect("create payment");
    db.payments().mark_completed_async(payment.id.into_uuid()).await.expect("complete")
}

async fn insert_escrow(db: &PostgresDatabase, payment_id: &str, status: &str) -> String {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO a2a_escrows (id, status, payment_id, buyer_address, seller_address, amount,
            amount_decimal, asset, network, expires_at, created_at, updated_at)
         VALUES ($1, $2, $3, '0xbuyer', '0xseller', 1000000, 1, 'USDC', 'set_chain', NOW(), NOW(), NOW())",
    )
    .bind(&id)
    .bind(status)
    .bind(payment_id)
    .execute(db.pool())
    .await
    .expect("insert escrow");
    id
}

async fn insert_open_dispute(db: &PostgresDatabase, escrow_id: &str) -> String {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO a2a_disputes (id, tenant_id, store_id, status, escrow_id, claimant_address,
            respondent_address, reason, category, amount_decimal, asset, evidence_deadline,
            review_deadline, created_at, updated_at)
         VALUES ($1, 'tenant-pg', 'store-pg', 'filed', $2, '0xbuyer', '0xseller', 'not delivered',
            'non_delivery', 1, 'USDC', NOW(), NOW(), NOW(), NOW())",
    )
    .bind(&id)
    .bind(escrow_id)
    .execute(db.pool())
    .await
    .expect("insert dispute");
    id
}

#[tokio::test]
async fn postgres_refund_is_refused_while_escrow_is_disputed() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let payment = completed_payment(&db).await;
    let escrow_id = insert_escrow(&db, &payment.id.to_string(), "disputed").await;

    let receipt = db
        .kernel_executor(policy())
        .execute_create_refund_async(&refund_command(payment.id))
        .await
        .expect("receipt");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("commerce.refund.escrow_disputed"));
    assert!(receipt.error_message.as_deref().unwrap_or_default().contains(&escrow_id));
    let stored = db.payments().get_async(payment.id.into_uuid()).await.unwrap().unwrap();
    assert_eq!(stored.amount_refunded, dec!(0));
}

#[tokio::test]
async fn postgres_refund_is_refused_for_filed_dispute_and_allowed_after_resolution() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let payment = completed_payment(&db).await;
    let escrow_id = insert_escrow(&db, &payment.id.to_string(), "active").await;
    let dispute_id = insert_open_dispute(&db, &escrow_id).await;

    let receipt = db
        .kernel_executor(policy())
        .execute_create_refund_async(&refund_command(payment.id))
        .await
        .expect("receipt");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("commerce.refund.escrow_disputed"));
    assert!(receipt.error_message.as_deref().unwrap_or_default().contains(&dispute_id));

    sqlx::query("UPDATE a2a_disputes SET status = 'resolved' WHERE id = $1")
        .bind(&dispute_id)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE a2a_escrows SET status = 'released' WHERE id = $1")
        .bind(&escrow_id)
        .execute(db.pool())
        .await
        .unwrap();
    let receipt = db
        .kernel_executor(policy())
        .execute_create_refund_async(&refund_command(payment.id))
        .await
        .expect("receipt");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{receipt:?}");
}

/// A funded kernel escrow holding `payment`, with a dispute filed against it.
/// Returns `(escrow_id, dispute_id)`.
async fn disputed_escrow(
    db: &PostgresDatabase,
    payment: &stateset_core::Payment,
) -> (String, String) {
    use stateset_core::{CreateA2AEscrow, FileA2ADispute, FundA2AEscrow};
    let kernel = db.kernel_executor(policy());
    let escrow = kernel
        .execute_create_a2a_escrow_async(&governed(
            "a2a.escrow.create",
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
        .await
        .expect("create escrow")
        .result
        .expect("escrow");
    let funded = kernel
        .execute_fund_a2a_escrow_async(&governed(
            "a2a.escrow.fund",
            "agent:escrow",
            FundA2AEscrow { escrow_id: escrow.id.clone() },
        ))
        .await
        .expect("fund escrow");
    assert_eq!(funded.status, ExecutionStatus::Succeeded, "{funded:?}");
    let now = chrono::Utc::now();
    let dispute = kernel
        .execute_file_a2a_dispute_async(&governed(
            "a2a.dispute.file",
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
        .await
        .expect("file dispute")
        .result
        .expect("dispute");
    (escrow.id, dispute.id)
}

/// Twin of the SQLite
/// `sqlite_refunding_a_disputed_escrow_resolves_its_dispute_as_a_full_refund`:
/// refunding a disputed escrow resolves its open dispute as a `full_refund`
/// in the same transaction, so the payment is not frozen for good.
#[tokio::test]
async fn postgres_refunding_a_disputed_escrow_resolves_its_dispute_as_a_full_refund() {
    use stateset_core::{
        A2ADisputeResolutionType, A2ADisputeStatus, A2AEscrowStatus, RefundA2AEscrow,
    };
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let payment = completed_payment(&db).await;
    let (escrow_id, dispute_id) = disputed_escrow(&db, &payment).await;

    let refunded = db
        .kernel_executor(policy())
        .execute_refund_a2a_escrow_async(&governed(
            "a2a.escrow.refund",
            "agent:escrow",
            RefundA2AEscrow { escrow_id, reason: Some("buyer refunded".into()) },
        ))
        .await
        .expect("refund escrow");
    assert_eq!(refunded.status, ExecutionStatus::Succeeded, "{refunded:?}");
    assert_eq!(refunded.result.as_ref().expect("escrow").status, A2AEscrowStatus::Refunded);
    assert_eq!(refunded.event_ids.len(), 2, "the escrow fact and the dispute fact");

    #[allow(clippy::type_complexity)]
    let (status, resolution, buyer, seller, resolved_by): (
        String,
        Option<String>,
        Option<rust_decimal::Decimal>,
        Option<rust_decimal::Decimal>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT status, resolution_type, buyer_amount_decimal, seller_amount_decimal, resolved_by
         FROM a2a_disputes WHERE id = $1",
    )
    .bind(&dispute_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(status, A2ADisputeStatus::Resolved.to_string());
    assert_eq!(resolution, Some(A2ADisputeResolutionType::FullRefund.to_string()));
    assert_eq!(buyer, Some(dec!(10)));
    assert_eq!(seller, Some(dec!(0)));
    assert_eq!(resolved_by.as_deref(), Some("agent:escrow"));

    // The payment is no longer frozen.
    let receipt = db
        .kernel_executor(policy())
        .execute_create_refund_async(&refund_command(payment.id))
        .await
        .expect("receipt");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{receipt:?}");
}

/// Connect a one-connection pool whose backend carries `name` as its
/// `application_name`, so the test can see when that backend waits on a lock.
async fn tagged_db(url: &str, name: &str) -> PostgresDatabase {
    let separator = if url.contains('?') { '&' } else { '?' };
    PostgresDatabase::connect_with_options(
        format!("{url}{separator}application_name={name}"),
        1,
        30,
    )
    .await
    .expect("connect tagged pool")
}

async fn wait_for_lock(
    db: &PostgresDatabase,
    name: &str,
) -> std::result::Result<(), tokio::time::error::Elapsed> {
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM pg_stat_activity
                                WHERE application_name = $1 AND wait_event_type = 'Lock')",
            )
            .bind(name)
            .fetch_one(db.pool())
            .await
            .expect("poll pg_stat_activity");
            if waiting {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
}

/// `a2a.escrow.refund` and `a2a.dispute.resolve` take the escrow and dispute
/// row locks in the same order (escrow first). Resolution used to lock the
/// dispute first, so a refund holding the escrow and a resolution holding
/// its dispute each waited on the other, and Postgres aborted one with a
/// deadlock error instead of an execution receipt.
///
/// A gate transaction holding the escrow row lines both commands up behind
/// it: the refund queues on the escrow first, then the resolution starts
/// (and, before the fix, locked the dispute on its way to the escrow). When
/// the gate commits, the refund runs first and resolves the dispute itself.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_concurrent_escrow_refund_and_dispute_resolution_do_not_deadlock() {
    use stateset_core::{A2ADisputeResolutionType, RefundA2AEscrow, ResolveA2ADispute};
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let payment = completed_payment(&db).await;
    let (escrow_id, dispute_id) = disputed_escrow(&db, &payment).await;
    let refund_name = format!("escrow_refund_{}", Uuid::new_v4().simple());
    let resolve_name = format!("dispute_resolve_{}", Uuid::new_v4().simple());
    let refund_db = tagged_db(&url, &refund_name).await;
    let resolve_db = tagged_db(&url, &resolve_name).await;

    let mut gate = db.pool().begin().await.expect("begin gate");
    sqlx::query("SELECT id FROM a2a_escrows WHERE id = $1 FOR UPDATE")
        .bind(&escrow_id)
        .fetch_one(gate.as_mut())
        .await
        .expect("gate locks the escrow");

    let refund_kernel = refund_db.kernel_executor(policy());
    let refund_command = governed(
        "a2a.escrow.refund",
        "agent:escrow",
        RefundA2AEscrow { escrow_id: escrow_id.clone(), reason: Some("buyer refunded".into()) },
    );
    let refund = tokio::spawn(async move {
        refund_kernel.execute_refund_a2a_escrow_async(&refund_command).await
    });
    let refund_waiting = wait_for_lock(&db, &refund_name).await;

    let resolve_kernel = resolve_db.kernel_executor(policy());
    let resolve_command = governed(
        "a2a.dispute.resolve",
        "agent:arbiter",
        ResolveA2ADispute {
            dispute_id: dispute_id.clone(),
            resolution_type: A2ADisputeResolutionType::FullRefund,
            buyer_amount: None,
            seller_amount: None,
            note: Some("arbiter refunds the buyer".into()),
        },
    );
    let resolve = tokio::spawn(async move {
        resolve_kernel.execute_resolve_a2a_dispute_async(&resolve_command).await
    });
    let resolve_waiting = wait_for_lock(&db, &resolve_name).await;
    gate.commit().await.expect("release the gate");

    let refund = tokio::time::timeout(std::time::Duration::from_secs(60), refund).await;
    let resolve = tokio::time::timeout(std::time::Duration::from_secs(60), resolve).await;
    refund_db.pool().close().await;
    resolve_db.pool().close().await;
    refund_waiting.expect("the refund queued on the gated escrow");
    resolve_waiting.expect("the resolution queued on a lock");
    let refund = refund
        .expect("refund finished")
        .expect("refund task")
        .expect("refund returned a receipt, not a deadlock error");
    let resolve = resolve
        .expect("resolution finished")
        .expect("resolution task")
        .expect("resolution returned a receipt, not a deadlock error");
    assert_eq!(refund.status, ExecutionStatus::Succeeded, "{refund:?}");
    assert_eq!(resolve.status, ExecutionStatus::Rejected, "{resolve:?}");
    assert_eq!(resolve.error_code.as_deref(), Some("commerce.a2a.dispute_not_resolvable"));

    let escrow_status: String = sqlx::query_scalar("SELECT status FROM a2a_escrows WHERE id = $1")
        .bind(&escrow_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    let (dispute_status, resolved_by): (String, Option<String>) =
        sqlx::query_as("SELECT status, resolved_by FROM a2a_disputes WHERE id = $1")
            .bind(&dispute_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(escrow_status, "refunded");
    assert_eq!(dispute_status, "resolved");
    assert_eq!(resolved_by.as_deref(), Some("agent:escrow"), "the refund closed the dispute");
}
