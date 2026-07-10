//! Regression tests for the Postgres gift-card / store-credit / loyalty money
//! guards (mirrors the SQLite in-module tests).
//!
//! Guards under test:
//! - charge/apply/refund reject non-positive amounts (a negative charge or
//!   apply previously *minted* balance)
//! - date-expired instruments whose status is still `active` cannot spend
//! - refunds cannot resurrect disabled gift cards
//! - store-credit apply requires `Active` status; adjust rejects voided credits
//! - loyalty redemptions cannot overdraw an account, and unknown accounts error
//!
//! These tests require a live Postgres instance (`POSTGRES_URL` /
//! `DATABASE_URL`) and are skipped otherwise, so they run only in CI with a
//! provisioned database (the Postgres Parity job).

#![cfg(feature = "postgres")]

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AdjustPoints, AdjustStoreCredit, CommerceError, CreateGiftCard, CreateLoyaltyProgram,
    CreateStoreCredit, CurrencyCode, CustomerId, EnrollCustomer, GiftCardStatus, LoyaltyAccountId,
    LoyaltyTransactionType, StoreCreditReason,
};
use stateset_db::PostgresDatabase;
use stateset_db::postgres::{
    PgGiftCardRepository, PgLoyaltyProgramRepository, PgStoreCreditRepository,
};
use std::env;

fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

async fn connect() -> Option<PostgresDatabase> {
    let url = postgres_url()?;
    Some(PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"))
}

fn assert_validation(err: CommerceError) {
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
}

async fn create_card(
    repo: &PgGiftCardRepository,
    balance: Decimal,
    expires_at: Option<chrono::DateTime<Utc>>,
) -> stateset_core::GiftCard {
    repo.create_async(CreateGiftCard {
        code: None,
        initial_balance: balance,
        currency: CurrencyCode::USD,
        recipient_email: None,
        sender_name: None,
        message: None,
        expires_at,
    })
    .await
    .expect("create gift card")
}

#[tokio::test]
async fn postgres_gift_card_charge_guards() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let repo = PgGiftCardRepository::new(db.pool().clone());
    let gc = create_card(&repo, dec!(50.00), None).await;

    // Non-positive amounts must not mint balance.
    assert_validation(repo.charge_async(gc.id, dec!(-10.00), None).await.unwrap_err());
    assert_validation(repo.charge_async(gc.id, Decimal::ZERO, None).await.unwrap_err());

    // Overdraft rejected; exact spend allowed.
    assert_validation(repo.charge_async(gc.id, dec!(60.00), None).await.unwrap_err());
    let txn = repo.charge_async(gc.id, dec!(30.00), None).await.expect("charge 30");
    assert_eq!(txn.balance_after, dec!(20.00));

    // A date-expired card whose status is still 'active' cannot spend.
    let expired = create_card(&repo, dec!(50.00), Some(Utc::now() - Duration::days(1))).await;
    assert_eq!(expired.status, GiftCardStatus::Active);
    assert_validation(repo.charge_async(expired.id, dec!(10.00), None).await.unwrap_err());
}

#[tokio::test]
async fn postgres_gift_card_refund_guards() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let repo = PgGiftCardRepository::new(db.pool().clone());
    let gc = create_card(&repo, dec!(50.00), None).await;

    assert_validation(repo.refund_async(gc.id, dec!(-5.00), None).await.unwrap_err());

    // Refunding a disabled card must not resurrect it.
    repo.disable_async(gc.id).await.expect("disable");
    assert_validation(repo.refund_async(gc.id, dec!(10.00), None).await.unwrap_err());
    let fetched = repo.get_async(gc.id).await.expect("get").expect("found");
    assert_eq!(fetched.status, GiftCardStatus::Disabled);
    assert_eq!(fetched.current_balance, dec!(50.00));
}

/// Postgres enforces `store_credits.customer_id` as a foreign key (unlike the
/// SQLite test schema), so the credit needs a real customer row.
async fn create_customer(db: &PostgresDatabase) -> CustomerId {
    let unique = uuid::Uuid::new_v4();
    db.customers()
        .create_async(stateset_core::CreateCustomer {
            email: format!("guard-{unique}@example.com"),
            first_name: "Guard".into(),
            last_name: "Test".into(),
            phone: None,
            accepts_marketing: Some(false),
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer")
        .id
}

async fn create_credit(
    repo: &PgStoreCreditRepository,
    customer_id: CustomerId,
    amount: Decimal,
) -> stateset_core::StoreCredit {
    repo.create_async(CreateStoreCredit {
        customer_id,
        amount,
        currency: CurrencyCode::USD,
        reason: StoreCreditReason::Return,
        reference_id: None,
        note: None,
        expires_at: None,
    })
    .await
    .expect("create store credit")
}

#[tokio::test]
async fn postgres_store_credit_apply_guards() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let repo = PgStoreCreditRepository::new(db.pool().clone());
    let customer_id = create_customer(&db).await;
    let sc = create_credit(&repo, customer_id, dec!(50.00)).await;
    let sc_uuid = sc.id.into_uuid();

    // Negative apply must not mint balance; overdraft rejected.
    assert_validation(repo.apply_async(sc_uuid, dec!(-10.00), None).await.unwrap_err());
    assert_validation(repo.apply_async(sc_uuid, dec!(99.00), None).await.unwrap_err());

    let txn = repo.apply_async(sc_uuid, dec!(30.00), None).await.expect("apply 30");
    assert_eq!(txn.balance_after, dec!(20.00));

    // Voided credits can be neither applied nor adjusted back to life.
    sqlx::query("UPDATE store_credits SET status = 'voided' WHERE id = $1")
        .bind(sc_uuid)
        .execute(db.pool())
        .await
        .expect("void credit");
    assert_validation(repo.apply_async(sc_uuid, dec!(5.00), None).await.unwrap_err());
    assert_validation(
        repo.adjust_async(
            sc_uuid,
            AdjustStoreCredit { amount: dec!(10.00), note: None, reference_id: None },
        )
        .await
        .unwrap_err(),
    );

    let fetched = repo.get_async(sc_uuid).await.expect("get").expect("found");
    assert_eq!(fetched.current_balance, dec!(20.00));
}

#[tokio::test]
async fn postgres_credit_reserve_and_charge_guards() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let repo = stateset_db::postgres::PgCreditRepository::new(db.pool().clone());
    let customer_id = create_customer(&db).await;
    repo.create_credit_account_async(stateset_core::CreateCreditAccount {
        customer_id,
        credit_limit: dec!(100.00),
        currency: None,
        payment_terms: Some("NET30".into()),
        risk_rating: None,
        notes: None,
    })
    .await
    .expect("create credit account");
    let cust = customer_id.into_uuid();

    // Non-positive and over-line reservations rejected.
    assert_validation(
        repo.reserve_credit_async(cust, uuid::Uuid::new_v4(), dec!(-10)).await.unwrap_err(),
    );
    assert_validation(
        repo.reserve_credit_async(cust, uuid::Uuid::new_v4(), dec!(150)).await.unwrap_err(),
    );

    // Reservations respect existing holds.
    let order = uuid::Uuid::new_v4();
    repo.reserve_credit_async(cust, order, dec!(60)).await.expect("reserve 60");
    assert_validation(
        repo.reserve_credit_async(cust, uuid::Uuid::new_v4(), dec!(50)).await.unwrap_err(),
    );

    // Charges respect the limit; a rejected charge keeps its reservation.
    repo.charge_credit_async(cust, order, dec!(60)).await.expect("charge 60");
    assert_validation(
        repo.charge_credit_async(cust, uuid::Uuid::new_v4(), dec!(50)).await.unwrap_err(),
    );
    let acct = repo.get_credit_account_by_customer_async(cust).await.expect("get").expect("found");
    assert_eq!(acct.current_balance, dec!(60.00));
}

#[tokio::test]
async fn postgres_lot_transfer_guards() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let repo = stateset_db::postgres::PgLotRepository::new(db.pool().clone());
    let lot = repo
        .create_async(stateset_core::CreateLot {
            sku: format!("XFER-{}", uuid::Uuid::new_v4()),
            quantity: dec!(5),
            initial_location_id: Some(1),
            ..Default::default()
        })
        .await
        .expect("create lot");

    let base = stateset_core::TransferLot {
        lot_id: lot.id,
        quantity: dec!(10),
        from_location_id: 99,
        to_location_id: 2,
        reason: None,
        performed_by: None,
    };

    // Missing source location must not mint quantity at the destination.
    assert_validation(
        repo.transfer_async(stateset_core::TransferLot { ..base.clone() }).await.unwrap_err(),
    );
    // Short source rejected; non-positive quantity rejected.
    assert_validation(
        repo.transfer_async(stateset_core::TransferLot { from_location_id: 1, ..base.clone() })
            .await
            .unwrap_err(),
    );
    assert_validation(
        repo.transfer_async(stateset_core::TransferLot {
            from_location_id: 1,
            quantity: dec!(-1),
            ..base.clone()
        })
        .await
        .unwrap_err(),
    );

    // A covered transfer still works.
    repo.transfer_async(stateset_core::TransferLot {
        from_location_id: 1,
        quantity: dec!(3),
        ..base
    })
    .await
    .expect("valid transfer");
}

#[tokio::test]
async fn postgres_loyalty_adjust_guards() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let repo = PgLoyaltyProgramRepository::new(db.pool().clone());
    let program = repo
        .create_async(CreateLoyaltyProgram {
            name: format!("Guard Program {}", uuid::Uuid::new_v4()),
            description: None,
            points_per_dollar: 1,
            tiers: vec![],
        })
        .await
        .expect("create program");
    let account = repo
        .enroll_async(EnrollCustomer { customer_id: CustomerId::new(), program_id: program.id })
        .await
        .expect("enroll");

    repo.adjust_points_async(AdjustPoints {
        account_id: account.id,
        points: 50,
        transaction_type: LoyaltyTransactionType::Earn,
        reference_id: None,
        description: None,
    })
    .await
    .expect("earn 50");

    // Overdraft redemption rejected.
    assert_validation(
        repo.adjust_points_async(AdjustPoints {
            account_id: account.id,
            points: -100,
            transaction_type: LoyaltyTransactionType::Redeem,
            reference_id: None,
            description: None,
        })
        .await
        .unwrap_err(),
    );

    // Unknown accounts error instead of minting orphaned transactions.
    let err = repo
        .adjust_points_async(AdjustPoints {
            account_id: LoyaltyAccountId::new(),
            points: 10,
            transaction_type: LoyaltyTransactionType::Earn,
            reference_id: None,
            description: None,
        })
        .await
        .unwrap_err();
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");

    let fetched = repo.get_account_async(account.id).await.expect("get account").expect("found");
    assert_eq!(fetched.points_balance, 50);
}
