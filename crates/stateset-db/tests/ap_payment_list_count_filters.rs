#![cfg(feature = "sqlite")]

//! Regression: SQLite AP payment listing/counting ignored filters.
//!
//! - `count_payments` ignored its filter entirely (`_filter`) — it always returned
//!   `SELECT COUNT(*) FROM ap_payments`, so a filtered count never matched the
//!   corresponding filtered list. Postgres applies `supplier_id`/`status`/`payment_method`.
//! - `list_payments` applied only `supplier_id` and `status`, silently dropping
//!   `payment_method` (Postgres applies it) and `offset` (no pagination past the
//!   first page).
//!
//! Both now share a WHERE-builder covering `supplier_id`/`status`/`payment_method`
//! and the `from_date`/`to_date` range, so a filtered `count_payments` matches the
//! filtered `list_payments`, and both agree with Postgres.

use chrono::{DateTime, Utc};
use rust_decimal_macros::dec;
use stateset_core::{
    AccountsPayableRepository, BillPaymentFilter, CreateBill, CreateBillItem, CreateBillPayment,
    PaymentAllocationInput, PaymentMethodAP,
};
use stateset_db::SqliteDatabase;

fn make_payment(
    ap: &dyn AccountsPayableRepository,
    supplier: uuid::Uuid,
    method: PaymentMethodAP,
    amount: rust_decimal::Decimal,
) {
    let date: DateTime<Utc> = "2026-03-10T14:30:00Z".parse().unwrap();
    let bill = ap
        .create_bill(CreateBill {
            supplier_id: supplier,
            bill_date: Some(date),
            due_date: "2026-04-10T00:00:00Z".parse().unwrap(),
            items: vec![CreateBillItem {
                description: "item".into(),
                account_code: None,
                quantity: dec!(1),
                unit_price: amount,
                tax_rate: None,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create bill");
    ap.approve_bill(bill.id).expect("approve bill");
    ap.create_payment(CreateBillPayment {
        supplier_id: supplier,
        payment_date: Some(date),
        payment_method: method,
        amount,
        currency: None,
        reference_number: None,
        bank_account: None,
        check_number: None,
        memo: None,
        allocations: vec![PaymentAllocationInput { bill_id: bill.id, amount }],
    })
    .expect("create payment");
}

#[test]
fn sqlite_ap_payment_list_and_count_apply_filters() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let ap = db.accounts_payable();

    let supplier_a = uuid::Uuid::new_v4();
    let supplier_b = uuid::Uuid::new_v4();
    make_payment(&ap, supplier_a, PaymentMethodAP::Check, dec!(100.00));
    make_payment(&ap, supplier_b, PaymentMethodAP::Wire, dec!(100.00));

    // Baseline: no filter counts/lists everything.
    assert_eq!(ap.count_payments(BillPaymentFilter::default()).unwrap(), 2);
    assert_eq!(ap.list_payments(BillPaymentFilter::default()).unwrap().len(), 2);

    // count_payments must honor supplier_id (previously ignored the whole filter).
    assert_eq!(
        ap.count_payments(BillPaymentFilter {
            supplier_id: Some(supplier_a),
            ..Default::default()
        })
        .unwrap(),
        1,
        "count_payments must filter by supplier"
    );

    // count_payments must honor payment_method.
    assert_eq!(
        ap.count_payments(BillPaymentFilter {
            payment_method: Some(PaymentMethodAP::Check),
            ..Default::default()
        })
        .unwrap(),
        1,
        "count_payments must filter by payment_method"
    );

    // list_payments must honor payment_method (previously dropped).
    let wires = ap
        .list_payments(BillPaymentFilter {
            payment_method: Some(PaymentMethodAP::Wire),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(wires.len(), 1, "list_payments must filter by payment_method");
    assert_eq!(wires[0].supplier_id, supplier_b);

    // list_payments must honor offset (previously ignored → no pagination).
    let page2 =
        ap.list_payments(BillPaymentFilter { offset: Some(1), ..Default::default() }).unwrap();
    assert_eq!(page2.len(), 1, "list_payments must skip `offset` rows");
}
