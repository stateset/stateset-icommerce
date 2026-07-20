#![cfg(feature = "sqlite")]

//! Regression: SQLite stored accounts-payable bill money at full precision, while
//! Postgres rounds it to the scale of its `NUMERIC(12,4)` columns (`NUMERIC(12,6)`
//! for `tax_rate`) on insert. So a bill item with sub-4dp inputs read back
//! differently on the two backends. SQLite now rounds AP money to 4dp (`tax_rate`
//! to 6dp) with `MidpointAwayFromZero`, matching Postgres numeric rounding.

use rust_decimal_macros::dec;
use stateset_core::{AccountsPayableRepository, CreateBill, CreateBillItem};
use stateset_db::SqliteDatabase;

#[test]
fn sqlite_ap_bill_money_rounds_to_postgres_numeric_scale() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let ap = db.accounts_payable();

    // unit_price has 5 decimals; amount = 1 * 10.12345 = 10.12345 rounds to 10.1235.
    // tax = 10.12345 * 8.5 / 100 = 0.86049325 rounds to 0.8605.
    let bill = ap
        .create_bill(CreateBill {
            supplier_id: uuid::Uuid::new_v4(),
            bill_date: Some("2026-03-10T00:00:00Z".parse().unwrap()),
            due_date: "2026-04-10T00:00:00Z".parse().unwrap(),
            items: vec![CreateBillItem {
                description: "Widget".into(),
                account_code: None,
                quantity: dec!(1),
                unit_price: dec!(10.12345),
                tax_rate: Some(dec!(8.5)),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create bill");

    let items = ap.get_bill_items(bill.id).expect("get bill items");
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item.unit_price, dec!(10.1235), "unit_price must round to 4dp");
    assert_eq!(item.amount, dec!(10.1235), "amount must round to 4dp");
    assert_eq!(item.tax_amount, dec!(0.8605), "tax_amount must round to 4dp");

    // Bill totals foot from the rounded item values.
    assert_eq!(bill.subtotal, dec!(10.1235), "subtotal must be 4dp");
    assert_eq!(bill.tax_amount, dec!(0.8605), "tax must be 4dp");
    assert_eq!(bill.total_amount, dec!(10.984), "total = subtotal + tax");
}
