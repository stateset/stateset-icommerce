//! Lifetime-earned points must reject overflow without recording an adjustment.
//! Requires `POSTGRES_URL` or `DATABASE_URL`; skipped otherwise.

#![cfg(feature = "postgres")]

use stateset_core::{
    AdjustPoints, CommerceError, CreateLoyaltyProgram, CustomerId, EnrollCustomer,
    LoyaltyTransactionType,
};
use stateset_db::PostgresDatabase;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_lifetime_points_overflow_preserves_balance_and_ledger() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let repo = db.loyalty();
    let program = repo
        .create_async(CreateLoyaltyProgram {
            name: "Lifetime Limit".into(),
            description: None,
            points_per_dollar: 1,
            tiers: vec![],
        })
        .await
        .expect("program");
    let account = repo
        .enroll_async(EnrollCustomer { customer_id: CustomerId::new(), program_id: program.id })
        .await
        .expect("enroll");
    for (points, transaction_type) in
        [(i64::MAX, LoyaltyTransactionType::Earn), (-i64::MAX, LoyaltyTransactionType::Redeem)]
    {
        repo.adjust_points_async(AdjustPoints {
            account_id: account.id,
            points,
            transaction_type,
            reference_id: None,
            description: None,
        })
        .await
        .expect("seed lifetime points");
    }
    let err = repo
        .adjust_points_async(AdjustPoints {
            account_id: account.id,
            points: 1,
            transaction_type: LoyaltyTransactionType::Earn,
            reference_id: None,
            description: None,
        })
        .await
        .expect_err("lifetime overflow");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    let stored = repo.get_account_async(account.id).await.expect("get").expect("account");
    assert_eq!(stored.points_balance, 0);
    assert_eq!(stored.lifetime_points, i64::MAX as u64);
    assert_eq!(repo.get_transactions_async(account.id, None).await.expect("transactions").len(), 2);
}
