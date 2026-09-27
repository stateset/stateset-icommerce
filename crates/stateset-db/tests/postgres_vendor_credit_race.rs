//! Concurrent applications must not spend a vendor credit beyond its balance.
//! Requires `POSTGRES_URL` or `DATABASE_URL`; skipped otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{ApplyVendorCredit, CreateVendorCredit, VendorCreditTargetType};
use stateset_db::PostgresDatabase;
use std::sync::Arc;
use tokio::sync::Barrier;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_competing_applications_cannot_exceed_vendor_credit() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let credit = db
        .vendor_credits()
        .create_async(CreateVendorCredit {
            supplier_id: Uuid::new_v4(),
            vendor_return_id: None,
            amount: dec!(3),
            currency: None,
            memo: None,
        })
        .await
        .expect("create vendor credit");
    let barrier = Arc::new(Barrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let id = credit.id;
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            db.vendor_credits()
                .apply_async(
                    id,
                    ApplyVendorCredit {
                        target_type: VendorCreditTargetType::Bill,
                        target_id: Uuid::new_v4(),
                        amount: dec!(2),
                    },
                )
                .await
        }));
    }
    let mut successes = 0;
    for handle in handles {
        if handle.await.expect("join application").is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, 1);
    let stored = db.vendor_credits().get_async(credit.id).await.expect("get").expect("credit");
    assert_eq!(stored.remaining, dec!(1));
    let apps = db
        .vendor_credits()
        .list_applications_async(credit.id)
        .await
        .unwrap_or_else(|_| panic!("application lookup failed"));
    assert_eq!(apps.len(), 1);
    assert_eq!(stored.remaining + apps[0].amount, credit.amount);
}
