//! Postgres parity for the non-positive payment-amount guards (SQLite covered
//! by `record_payment_rejects_nonpositive_amount` and
//! `apply_payment_rejects_nonpositive_amount`). A zero or negative payment must
//! be rejected before it can corrupt a balance on either backend.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    CreateCreditAccount, CreateInvoice, CreateInvoiceItem, RecordInvoicePayment, RiskRating,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_rejects_nonpositive_payment_amounts() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping payment validation test");
        return;
    };
    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");

    let unique = uuid::Uuid::new_v4().to_string();

    // --- Invoice payment ---
    let customer = commerce
        .customers()
        .create(stateset_core::CreateCustomer {
            email: format!("val-{}@example.com", &unique[..8]),
            first_name: "Val".into(),
            last_name: "Idate".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    let invoice = commerce
        .invoices()
        .create(CreateInvoice {
            customer_id: customer.id,
            days_until_due: Some(30),
            items: vec![CreateInvoiceItem {
                description: "Widget".into(),
                quantity: dec!(2),
                unit_price: dec!(50),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create invoice");
    let pay = |amount| RecordInvoicePayment {
        amount,
        payment_id: None,
        payment_method: None,
        reference: None,
        notes: None,
    };
    assert!(
        commerce.invoices().record_payment(invoice.id.into_uuid(), pay(dec!(0))).await.is_err(),
        "zero invoice payment must be rejected"
    );
    assert!(
        commerce.invoices().record_payment(invoice.id.into_uuid(), pay(dec!(-10))).await.is_err(),
        "negative invoice payment must be rejected"
    );
    let fetched =
        commerce.invoices().get(invoice.id.into_uuid()).await.expect("get").expect("found");
    assert_eq!(fetched.amount_paid, dec!(0), "rejected payment must not touch amount_paid");

    // --- Credit payment ---
    let credit_customer = uuid::Uuid::new_v4();
    commerce
        .credit()
        .create_credit_account(CreateCreditAccount {
            customer_id: credit_customer.into(),
            credit_limit: dec!(100),
            currency: None,
            payment_terms: Some("NET30".into()),
            risk_rating: Some(RiskRating::Low),
            notes: None,
        })
        .await
        .expect("create credit account");
    commerce
        .credit()
        .charge_credit(credit_customer, uuid::Uuid::new_v4(), dec!(50))
        .await
        .expect("charge 50");
    assert!(
        commerce.credit().apply_payment(credit_customer, dec!(0), None).await.is_err(),
        "zero credit payment must be rejected"
    );
    assert!(
        commerce.credit().apply_payment(credit_customer, dec!(-25), None).await.is_err(),
        "negative credit payment must be rejected"
    );
    let acct = commerce
        .credit()
        .get_credit_account_by_customer(credit_customer)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(acct.current_balance, dec!(50), "rejected payment must not change the balance");
}
