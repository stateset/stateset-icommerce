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
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

fn policy() -> KernelPolicy {
    KernelPolicy::new("commerce-policy-1")
        .allow("payments.create_refund", KernelCommandPolicy::requiring(["payments.create_refund"]))
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
