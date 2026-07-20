#![cfg(feature = "sqlite")]

//! Regression: SQLite `list` only honored `customer_id` / `order_id` / status /
//! `overdue_only` and silently ignored the other `InvoiceFilter` fields
//! (`invoice_type`, date ranges, `min_total`, `max_total`, `min_balance`,
//! `invoice_number`), while Postgres applies them all. A collections query filtered
//! to invoices over a threshold therefore returned everything on SQLite. SQLite
//! now honors the same filters.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CreateInvoice, CreateInvoiceItem, CustomerId, InvoiceFilter, InvoiceRepository,
};
use stateset_db::SqliteDatabase;

fn item(price: Decimal) -> CreateInvoiceItem {
    CreateInvoiceItem {
        description: "Widget".into(),
        quantity: dec!(1),
        unit_price: price,
        ..Default::default()
    }
}

#[test]
fn sqlite_invoice_list_honors_money_filters() {
    let db = SqliteDatabase::in_memory().expect("in-memory");
    let repo = db.invoices();
    let cust = CustomerId::new();

    let small = repo
        .create(CreateInvoice {
            customer_id: cust,
            items: vec![item(dec!(20))],
            ..Default::default()
        })
        .expect("small invoice");
    let big = repo
        .create(CreateInvoice {
            customer_id: cust,
            items: vec![item(dec!(200))],
            ..Default::default()
        })
        .expect("big invoice");

    // No filter → both.
    assert_eq!(repo.list(InvoiceFilter::default()).unwrap().len(), 2);

    // min_total = 100 → only the $200 invoice.
    let r = repo
        .list(InvoiceFilter { min_total: Some(dec!(100)), ..Default::default() })
        .expect("min_total");
    assert_eq!(r.len(), 1, "min_total must filter: {r:?}");
    assert_eq!(r[0].id, big.id);

    // max_total = 100 → only the $20 invoice.
    let r = repo
        .list(InvoiceFilter { max_total: Some(dec!(100)), ..Default::default() })
        .expect("max_total");
    assert_eq!(r.len(), 1, "max_total must filter: {r:?}");
    assert_eq!(r[0].id, small.id);

    // min_balance = 100 → only the $200 invoice (both fully unpaid).
    let r = repo
        .list(InvoiceFilter { min_balance: Some(dec!(100)), ..Default::default() })
        .expect("min_balance");
    assert_eq!(r.len(), 1, "min_balance must filter: {r:?}");
    assert_eq!(r[0].id, big.id);
}

#[test]
fn sqlite_invoice_list_honors_invoice_number_search() {
    let db = SqliteDatabase::in_memory().expect("in-memory");
    let repo = db.invoices();

    let inv = repo
        .create(CreateInvoice {
            customer_id: CustomerId::new(),
            items: vec![item(dec!(10))],
            ..Default::default()
        })
        .expect("invoice");

    // The invoice's own (auto-generated) number matches.
    let r = repo
        .list(InvoiceFilter {
            invoice_number: Some(inv.invoice_number.clone()),
            ..Default::default()
        })
        .expect("by number");
    assert_eq!(r.len(), 1, "exact number must match: {r:?}");
    assert_eq!(r[0].id, inv.id);

    // A number that does not exist matches nothing.
    let r = repo
        .list(InvoiceFilter {
            invoice_number: Some("NOPE-does-not-exist".into()),
            ..Default::default()
        })
        .expect("bogus number");
    assert!(r.is_empty(), "unknown number must match nothing: {r:?}");
}
