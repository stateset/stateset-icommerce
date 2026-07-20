#![cfg(feature = "sqlite")]

//! Regression: SQLite `count_bills` applied only `status`, so a filtered count did
//! not match the corresponding filtered `list_bills` — it ignored `supplier_id`,
//! `purchase_order_id`, `overdue_only`, and `min_amount`/`max_amount`. A count is
//! meant to report how many rows the same filtered list would return, so the two
//! must agree. `count_bills` now shares `list_bills`' matching logic (all filters
//! except the deferred `from_date`/`to_date`), matching Postgres.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{AccountsPayableRepository, BillFilter, CreateBill, CreateBillItem};
use stateset_db::SqliteDatabase;

fn make_bill(
    ap: &dyn AccountsPayableRepository,
    supplier: uuid::Uuid,
    po: Option<uuid::Uuid>,
    amount: Decimal,
) {
    ap.create_bill(CreateBill {
        supplier_id: supplier,
        purchase_order_id: po,
        bill_date: Some("2026-03-10T00:00:00Z".parse().unwrap()),
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
}

#[test]
fn sqlite_count_bills_matches_list_bills_across_filters() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let ap = db.accounts_payable();

    let supplier_a = uuid::Uuid::new_v4();
    let supplier_b = uuid::Uuid::new_v4();
    let po = uuid::Uuid::new_v4();
    make_bill(&ap, supplier_a, Some(po), dec!(100.00));
    make_bill(&ap, supplier_a, None, dec!(300.00));
    make_bill(&ap, supplier_b, Some(po), dec!(50.00));

    // For every filter, the count must equal the length of the filtered list.
    let check = |label: &str, filter: BillFilter, expected: u64| {
        let count = ap.count_bills(filter.clone()).expect("count");
        let listed = ap.list_bills(filter).expect("list").len() as u64;
        assert_eq!(count, expected, "count_bills wrong for {label}");
        assert_eq!(count, listed, "count_bills must match list_bills length for {label}");
    };

    check("no filter", BillFilter::default(), 3);
    check("supplier_a", BillFilter { supplier_id: Some(supplier_a), ..Default::default() }, 2);
    check("purchase_order", BillFilter { purchase_order_id: Some(po), ..Default::default() }, 2);
    check("min_amount>=100", BillFilter { min_amount: Some(dec!(100)), ..Default::default() }, 2);
    check("max_amount<=100", BillFilter { max_amount: Some(dec!(100)), ..Default::default() }, 2);
    check(
        "supplier_a + min 200",
        BillFilter {
            supplier_id: Some(supplier_a),
            min_amount: Some(dec!(200)),
            ..Default::default()
        },
        1,
    );
}
