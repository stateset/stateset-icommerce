#![cfg(feature = "sqlite")]

//! Regression: SQLite stored invoice money at full precision while Postgres
//! rounds every money column to `DECIMAL(12, 2)`, so line totals, invoice totals,
//! and balances diverged between backends (and could flip an invoice between
//! Paid and `PartiallyPaid`). SQLite now rounds money to cents (half away from
//! zero, matching Postgres NUMERIC), so both backends store identical values.

use rust_decimal_macros::dec;
use stateset_core::{CreateInvoice, CreateInvoiceItem, CustomerId, InvoiceRepository};
use stateset_db::SqliteDatabase;

#[test]
fn sqlite_invoice_totals_round_to_cents() {
    let db = SqliteDatabase::in_memory().expect("in-memory");
    let repo = db.invoices();

    // 3 × 3.333 = 9.999 must foot to 10.00 (the classic 9.999-vs-10.00 case).
    let invoice = repo
        .create(CreateInvoice {
            customer_id: CustomerId::new(),
            items: vec![CreateInvoiceItem {
                description: "Widget".into(),
                quantity: dec!(3),
                unit_price: dec!(3.333),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create invoice");

    assert_eq!(invoice.items[0].line_total, dec!(10.00), "line_total must round to cents");
    assert_eq!(invoice.subtotal, dec!(10.00));
    assert_eq!(invoice.total, dec!(10.00));
    assert_eq!(invoice.balance_due, dec!(10.00));
}

#[test]
fn sqlite_invoice_money_rounds_half_away_from_zero() {
    let db = SqliteDatabase::in_memory().expect("in-memory");
    let repo = db.invoices();

    // 10.005 at the cent boundary must round to 10.01 (matching Postgres
    // NUMERIC's half-away-from-zero), not 10.00 (banker's rounding).
    let invoice = repo
        .create(CreateInvoice {
            customer_id: CustomerId::new(),
            items: vec![CreateInvoiceItem {
                description: "Widget".into(),
                quantity: dec!(1),
                unit_price: dec!(10.005),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create invoice");

    assert_eq!(invoice.total, dec!(10.01), "must round half away from zero to match Postgres");
}
