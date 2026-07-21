//! Postgres parity for `auto_post_invoice`.
//!
//! SQLite `auto_post_invoice` was doubly broken (read a nonexistent `total_amount`
//! column, and parsed the RFC3339 `invoice_date` as a bare `NaiveDate`). Postgres
//! reads `total` and reduces the `DateTime<Utc>` with `.date_naive()`, so it was
//! already correct — this test locks in that behavior so the two backends stay in
//! agreement: posting an invoice yields a balanced, correctly-dated journal entry.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal_macros::dec;
use stateset_core::{
    CreateAutoPostingConfig, CreateCustomer, CreateGlPeriod, CreateInvoice, CreateInvoiceItem,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_auto_post_invoice_posts_balanced_entry() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping auto_post_invoice parity test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let gl = commerce.general_ledger();

    gl.initialize_chart_of_accounts().await.expect("init chart of accounts");

    let acct = |number: &'static str| {
        let gl = &gl;
        async move {
            gl.get_account_by_number(number)
                .await
                .expect("query account")
                .unwrap_or_else(|| panic!("account {number} exists"))
                .id
        }
    };

    let period = gl
        .create_period(CreateGlPeriod {
            period_name: "2026-03".into(),
            fiscal_year: 2026,
            period_number: 3,
            start_date: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
        })
        .await
        .expect("create period");
    gl.open_period(period.id).await.expect("open period");

    gl.set_auto_posting_config(CreateAutoPostingConfig {
        config_name: "default".into(),
        cash_account_id: acct("1010").await,
        accounts_receivable_account_id: acct("1100").await,
        inventory_account_id: acct("1200").await,
        accounts_payable_account_id: acct("2010").await,
        unearned_revenue_account_id: None,
        sales_revenue_account_id: acct("4010").await,
        shipping_revenue_account_id: None,
        cogs_account_id: acct("5010").await,
        bad_debt_expense_account_id: None,
        fx_gain_loss_account_id: None,
        auto_post_depreciation: false,
        auto_post_revenue_recognition: false,
    })
    .await
    .expect("set auto-posting config");

    // Postgres enforces the invoices → customers FK, so seed a real customer.
    let unique = uuid::Uuid::new_v4().simple().to_string();
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("ap-{}@example.com", &unique[..8]),
            first_name: "Ada".into(),
            last_name: "Lovelace".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer");

    let invoice_date: DateTime<Utc> = "2026-03-10T14:30:00Z".parse().expect("parse date");
    let invoice = commerce
        .invoices()
        .create(CreateInvoice {
            customer_id: customer.id,
            invoice_date: Some(invoice_date),
            items: vec![CreateInvoiceItem {
                description: "Widget".into(),
                quantity: dec!(2),
                unit_price: dec!(50.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create invoice");
    assert_eq!(invoice.total, dec!(100.00), "invoice total should foot to 100.00");

    let entry =
        gl.auto_post_invoice(invoice.id.into()).await.expect("auto_post_invoice must succeed");

    assert_eq!(entry.total_debits, dec!(100.00), "AR debit should equal the invoice total");
    assert_eq!(entry.total_credits, dec!(100.00), "Sales credit should equal the invoice total");
    assert_eq!(
        entry.entry_date,
        invoice_date.date_naive(),
        "entry date should be the invoice date"
    );
}
