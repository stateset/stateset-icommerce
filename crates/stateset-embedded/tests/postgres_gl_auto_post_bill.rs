//! Postgres parity for `auto_post_bill`.
//!
//! SQLite `auto_post_bill` was doubly broken (selected `FROM bills`, a table that
//! does not exist — AP bills live in `ap_bills` — and parsed the RFC3339
//! `bill_date` as a bare `NaiveDate`). Postgres reads `ap_bills` with a native
//! `DATE`, so it was already correct — this test locks in that behavior so the two
//! backends stay in agreement: posting a vendor bill yields a balanced,
//! correctly-dated journal entry (debit Inventory/Expense, credit AP).
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal_macros::dec;
use stateset_core::{CreateAutoPostingConfig, CreateBill, CreateBillItem, CreateGlPeriod};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_auto_post_bill_posts_balanced_entry() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping bill auto-post parity test");
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

    let bill_date: DateTime<Utc> = "2026-03-10T14:30:00Z".parse().expect("parse date");
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
        auto_post_depreciation: false,
        auto_post_revenue_recognition: false,
    })
    .await
    .expect("set auto-posting config");

    let bill = commerce
        .accounts_payable()
        .create_bill(CreateBill {
            supplier_id: uuid::Uuid::new_v4(),
            bill_date: Some(bill_date),
            due_date: "2026-04-10T00:00:00Z".parse().expect("parse due date"),
            items: vec![CreateBillItem {
                description: "Raw material".into(),
                account_code: None,
                quantity: dec!(2),
                unit_price: dec!(50.00),
                tax_rate: None,
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create bill");
    assert_eq!(bill.total_amount, dec!(100.00), "bill total should foot to 100.00");

    let entry = gl.auto_post_bill(bill.id).await.expect("auto_post_bill must succeed");

    assert_eq!(entry.total_debits, dec!(100.00), "Inventory debit should equal the bill total");
    assert_eq!(entry.total_credits, dec!(100.00), "AP credit should equal the bill total");
    assert_eq!(entry.entry_date, bill_date.date_naive(), "entry date should be the bill date");
}
