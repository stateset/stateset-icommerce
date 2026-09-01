#![cfg(feature = "sqlite")]

//! Regression: SQLite `list_payment_runs` ignored its filter entirely (`_filter`)
//! — it always returned `SELECT * FROM ap_payment_runs ORDER BY created_at DESC`,
//! dropping `status` (Postgres applies it) and `limit`/`offset` (no pagination).
//! It now applies `status` and `LIMIT`/`OFFSET`, matching Postgres.
//! (`from_date`/`to_date` remain deferred — entangled with the AP date-storage
//! divergence, as with the other AP list filters.)

use rust_decimal_macros::dec;
use stateset_core::{
    AccountsPayableRepository, CreateBill, CreateBillItem, CreatePaymentRun, PaymentRunFilter,
    PaymentRunStatus,
};
use stateset_db::SqliteDatabase;

#[test]
fn sqlite_list_payment_runs_applies_status_and_pagination() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let ap = db.accounts_payable();

    // Each run needs a payable (approved) bill: `create_payment_run` now
    // validates bill_ids.
    let approved_bill = || {
        let bill = ap
            .create_bill(CreateBill {
                supplier_id: uuid::Uuid::new_v4(),
                due_date: "2026-12-31T00:00:00Z".parse().expect("parse date"),
                items: vec![CreateBillItem {
                    description: "x".into(),
                    account_code: None,
                    quantity: dec!(1),
                    unit_price: dec!(10),
                    tax_rate: None,
                    ..Default::default()
                }],
                ..Default::default()
            })
            .expect("create bill");
        ap.approve_bill(bill.id).expect("approve bill");
        bill.id
    };

    let run_date = "2026-03-10T00:00:00Z".parse().expect("parse date");
    let draft = ap
        .create_payment_run(CreatePaymentRun {
            payment_date: run_date,
            bill_ids: vec![approved_bill()],
            ..Default::default()
        })
        .expect("create draft run");
    let to_cancel = ap
        .create_payment_run(CreatePaymentRun {
            payment_date: run_date,
            bill_ids: vec![approved_bill()],
            ..Default::default()
        })
        .expect("create second run");
    ap.cancel_payment_run(to_cancel.id).expect("cancel run");

    // Baseline: no filter returns both.
    assert_eq!(ap.list_payment_runs(PaymentRunFilter::default()).unwrap().len(), 2);

    // status filter (previously ignored → returned both).
    let drafts = ap
        .list_payment_runs(PaymentRunFilter {
            status: Some(PaymentRunStatus::Draft),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(drafts.len(), 1, "list_payment_runs must filter by status");
    assert_eq!(drafts[0].id, draft.id);

    let cancelled = ap
        .list_payment_runs(PaymentRunFilter {
            status: Some(PaymentRunStatus::Cancelled),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(cancelled.len(), 1, "list_payment_runs must filter by status");
    assert_eq!(cancelled[0].id, to_cancel.id);

    // limit (previously ignored → returned both).
    let limited =
        ap.list_payment_runs(PaymentRunFilter { limit: Some(1), ..Default::default() }).unwrap();
    assert_eq!(limited.len(), 1, "list_payment_runs must honor `limit`");

    // offset (previously ignored → returned both).
    let offset =
        ap.list_payment_runs(PaymentRunFilter { offset: Some(1), ..Default::default() }).unwrap();
    assert_eq!(offset.len(), 1, "list_payment_runs must skip `offset` rows");
}
