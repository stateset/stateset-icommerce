//! Concurrent applications must not spend a vendor credit beyond its balance.
//! Requires `POSTGRES_URL` or `DATABASE_URL`; skipped otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    ApplyVendorCredit, CreateBill, CreateBillItem, CreateSupplier, CreateVendorCredit,
    VendorCreditTargetType,
};
use stateset_db::PostgresDatabase;
use std::sync::Arc;
use tokio::sync::Barrier;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    stateset_test_utils::postgres_url()
}

#[tokio::test]
async fn postgres_competing_applications_cannot_exceed_vendor_credit() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    // Credits settle a real, approved bill from the same supplier.
    let supplier = db
        .purchase_orders()
        .create_supplier_async(CreateSupplier {
            name: format!("Credit race supplier {}", Uuid::new_v4()),
            ..Default::default()
        })
        .await
        .expect("create supplier");
    let bill = db
        .accounts_payable()
        .create_bill_async(CreateBill {
            supplier_id: supplier.id,
            due_date: chrono::Utc::now(),
            items: vec![CreateBillItem {
                description: "Credit race".into(),
                quantity: dec!(1),
                unit_price: dec!(10),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create bill");
    db.accounts_payable().approve_bill_async(bill.id).await.expect("approve bill");
    let credit = db
        .vendor_credits()
        .create_async(CreateVendorCredit {
            supplier_id: supplier.id,
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
        let bill = bill.id;
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            db.vendor_credits()
                .apply_async(
                    id,
                    ApplyVendorCredit {
                        target_type: VendorCreditTargetType::Bill,
                        target_id: bill,
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
