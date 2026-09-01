#![cfg(feature = "sqlite")]

//! Regression tests for Accounts Payable status-transition and filter guards
//! (SQLite backend, sync `Commerce` engine).
//!
//! Covers:
//! - `clear_payment` refuses voided payments and clears pending ones;
//! - `cancel_bill` refuses paid bills;
//! - `dispute_bill` refuses cancelled bills;
//! - `approve_bill` errors (instead of silently succeeding) on cancelled bills;
//! - `count_payments` applies the same `from_date`/`to_date` filters as
//!   `list_payments`.

use chrono::{DateTime, Duration, TimeZone, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_embedded::{
    Bill, BillPayment, BillPaymentFilter, BillStatus, Commerce, CreateBill, CreateBillItem,
    CreateBillPayment, PaymentAllocationInput, PaymentMethodAP, PaymentStatusAP,
};
use uuid::Uuid;

fn commerce() -> Commerce {
    Commerce::new(":memory:").expect("Failed to create in-memory Commerce")
}

fn make_bill(commerce: &Commerce, supplier: Uuid, qty: Decimal, price: Decimal) -> Bill {
    commerce
        .accounts_payable()
        .create_bill(CreateBill {
            supplier_id: supplier,
            due_date: Utc::now() + Duration::days(30),
            items: vec![CreateBillItem {
                description: "Widget".into(),
                account_code: Some("5000".into()),
                quantity: qty,
                unit_price: price,
                tax_rate: None,
                po_line_id: None,
            }],
            ..Default::default()
        })
        .expect("create bill")
}

fn pay_bill(
    commerce: &Commerce,
    supplier: Uuid,
    bill_id: Uuid,
    amount: Decimal,
    payment_date: Option<DateTime<Utc>>,
) -> BillPayment {
    commerce
        .accounts_payable()
        .create_payment(CreateBillPayment {
            supplier_id: supplier,
            payment_date,
            payment_method: PaymentMethodAP::Ach,
            amount,
            currency: None,
            reference_number: None,
            bank_account: None,
            check_number: None,
            memo: None,
            allocations: vec![PaymentAllocationInput { bill_id, amount }],
        })
        .expect("create payment")
}

#[test]
fn clear_payment_succeeds_for_pending_payment() {
    let commerce = commerce();
    let supplier = Uuid::new_v4();
    let bill = make_bill(&commerce, supplier, dec!(2), dec!(50));
    commerce.accounts_payable().approve_bill(bill.id).expect("approve");
    let payment = pay_bill(&commerce, supplier, bill.id, dec!(100), None);
    assert_eq!(payment.status, PaymentStatusAP::Pending);

    let cleared = commerce.accounts_payable().clear_payment(payment.id).expect("clear");
    assert_eq!(cleared.status, PaymentStatusAP::Cleared);
}

#[test]
fn clear_payment_rejects_voided_payment() {
    let commerce = commerce();
    let supplier = Uuid::new_v4();
    let bill = make_bill(&commerce, supplier, dec!(2), dec!(50));
    commerce.accounts_payable().approve_bill(bill.id).expect("approve");
    let payment = pay_bill(&commerce, supplier, bill.id, dec!(100), None);

    commerce.accounts_payable().void_payment(payment.id).expect("void");

    let result = commerce.accounts_payable().clear_payment(payment.id);
    assert!(result.is_err(), "clearing a voided payment must be rejected");

    let after = commerce
        .accounts_payable()
        .get_payment(payment.id)
        .expect("get payment")
        .expect("payment exists");
    assert_eq!(after.status, PaymentStatusAP::Voided, "status must remain voided");
}

#[test]
fn cancel_bill_rejects_paid_bill() {
    let commerce = commerce();
    let supplier = Uuid::new_v4();
    let bill = make_bill(&commerce, supplier, dec!(2), dec!(50));
    commerce.accounts_payable().approve_bill(bill.id).expect("approve");
    pay_bill(&commerce, supplier, bill.id, dec!(100), None);

    let paid = commerce.accounts_payable().get_bill(bill.id).expect("get").expect("bill");
    assert_eq!(paid.status, BillStatus::Paid);

    let result = commerce.accounts_payable().cancel_bill(bill.id);
    assert!(result.is_err(), "cancelling a paid bill must be rejected");

    let after = commerce.accounts_payable().get_bill(bill.id).expect("get").expect("bill");
    assert_eq!(after.status, BillStatus::Paid, "status must remain paid");
}

#[test]
fn cancel_bill_rejects_partially_paid_bill() {
    let commerce = commerce();
    let supplier = Uuid::new_v4();
    let bill = make_bill(&commerce, supplier, dec!(2), dec!(50));
    commerce.accounts_payable().approve_bill(bill.id).expect("approve");
    pay_bill(&commerce, supplier, bill.id, dec!(40), None);

    let partial = commerce.accounts_payable().get_bill(bill.id).expect("get").expect("bill");
    assert_eq!(partial.status, BillStatus::PartiallyPaid);

    let result = commerce.accounts_payable().cancel_bill(bill.id);
    assert!(result.is_err(), "cancelling a partially-paid bill must be rejected");
}

#[test]
fn cancel_bill_succeeds_from_draft() {
    let commerce = commerce();
    let bill = make_bill(&commerce, Uuid::new_v4(), dec!(1), dec!(10));
    let cancelled = commerce.accounts_payable().cancel_bill(bill.id).expect("cancel");
    assert_eq!(cancelled.status, BillStatus::Cancelled);
}

#[test]
fn dispute_bill_rejects_cancelled_bill() {
    let commerce = commerce();
    let bill = make_bill(&commerce, Uuid::new_v4(), dec!(1), dec!(10));
    commerce.accounts_payable().cancel_bill(bill.id).expect("cancel");

    let result = commerce.accounts_payable().dispute_bill(bill.id);
    assert!(result.is_err(), "disputing a cancelled bill must be rejected");

    let after = commerce.accounts_payable().get_bill(bill.id).expect("get").expect("bill");
    assert_eq!(after.status, BillStatus::Cancelled, "status must remain cancelled");
}

#[test]
fn dispute_bill_rejects_paid_bill() {
    let commerce = commerce();
    let supplier = Uuid::new_v4();
    let bill = make_bill(&commerce, supplier, dec!(2), dec!(50));
    commerce.accounts_payable().approve_bill(bill.id).expect("approve");
    pay_bill(&commerce, supplier, bill.id, dec!(100), None);

    let result = commerce.accounts_payable().dispute_bill(bill.id);
    assert!(result.is_err(), "disputing a paid bill must be rejected");
}

#[test]
fn dispute_bill_succeeds_from_approved() {
    let commerce = commerce();
    let bill = make_bill(&commerce, Uuid::new_v4(), dec!(1), dec!(10));
    commerce.accounts_payable().approve_bill(bill.id).expect("approve");
    let disputed = commerce.accounts_payable().dispute_bill(bill.id).expect("dispute");
    assert_eq!(disputed.status, BillStatus::Disputed);
}

#[test]
fn approve_bill_errors_on_cancelled_bill() {
    let commerce = commerce();
    let bill = make_bill(&commerce, Uuid::new_v4(), dec!(1), dec!(10));
    commerce.accounts_payable().cancel_bill(bill.id).expect("cancel");

    let result = commerce.accounts_payable().approve_bill(bill.id);
    assert!(result.is_err(), "approving a cancelled bill must error, not silently succeed");

    let after = commerce.accounts_payable().get_bill(bill.id).expect("get").expect("bill");
    assert_eq!(after.status, BillStatus::Cancelled, "status must remain cancelled");
}

#[test]
fn approve_bill_errors_on_missing_bill() {
    let commerce = commerce();
    assert!(commerce.accounts_payable().approve_bill(Uuid::new_v4()).is_err());
}

#[test]
fn count_payments_respects_date_filters() {
    let commerce = commerce();
    let supplier = Uuid::new_v4();

    let early = Utc.with_ymd_and_hms(2026, 1, 10, 0, 0, 0).unwrap();
    let late = Utc.with_ymd_and_hms(2026, 3, 10, 0, 0, 0).unwrap();

    let bill_a = make_bill(&commerce, supplier, dec!(1), dec!(10));
    commerce.accounts_payable().approve_bill(bill_a.id).expect("approve a");
    pay_bill(&commerce, supplier, bill_a.id, dec!(10), Some(early));

    let bill_b = make_bill(&commerce, supplier, dec!(1), dec!(20));
    commerce.accounts_payable().approve_bill(bill_b.id).expect("approve b");
    pay_bill(&commerce, supplier, bill_b.id, dec!(20), Some(late));

    let ap = commerce.accounts_payable();

    // Range covering only the early payment.
    let early_filter = BillPaymentFilter {
        supplier_id: Some(supplier),
        from_date: Some(early - Duration::days(1)),
        to_date: Some(early + Duration::days(1)),
        ..Default::default()
    };
    let listed = ap.list_payments(early_filter.clone()).expect("list early");
    let counted = ap.count_payments(early_filter).expect("count early");
    assert_eq!(listed.len(), 1, "list must return only the early payment");
    assert_eq!(counted, 1, "count must match the filtered list");

    // Open-ended from_date covering only the late payment.
    let late_filter = BillPaymentFilter {
        supplier_id: Some(supplier),
        from_date: Some(late - Duration::days(1)),
        ..Default::default()
    };
    let listed = ap.list_payments(late_filter.clone()).expect("list late");
    let counted = ap.count_payments(late_filter).expect("count late");
    assert_eq!(listed.len(), 1, "list must return only the late payment");
    assert_eq!(counted, 1, "count must match the filtered list");

    // Range covering both.
    let both_filter = BillPaymentFilter {
        supplier_id: Some(supplier),
        from_date: Some(early - Duration::days(1)),
        to_date: Some(late + Duration::days(1)),
        ..Default::default()
    };
    assert_eq!(ap.count_payments(both_filter).expect("count both"), 2);

    // Range covering neither.
    let none_filter = BillPaymentFilter {
        supplier_id: Some(supplier),
        to_date: Some(early - Duration::days(1)),
        ..Default::default()
    };
    assert_eq!(ap.count_payments(none_filter).expect("count none"), 0);
}
