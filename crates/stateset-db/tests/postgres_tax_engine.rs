//! Postgres mirrors of the SQLite tax-engine tests in `sqlite/tax.rs`:
//! per-line rounding that sums exactly, tax-inclusive pricing, exemption
//! verification / validity window / jurisdiction scoping, and
//! case-insensitive jurisdiction codes. Both backends feed the shared
//! `stateset_core::compute_tax`, so these must agree with SQLite cent for
//! cent.
//!
//! Runs only when `POSTGRES_URL` (or `DATABASE_URL`) is set.

#![cfg(feature = "postgres")]

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CreateCustomer, CreateTaxExemption, CreateTaxJurisdiction, CreateTaxRate, CurrencyCode,
    ExemptionType, JurisdictionLevel, ProductTaxCategory, TaxAddress, TaxCalculationMethod,
    TaxCalculationRequest, TaxJurisdictionFilter, TaxLineItem, TaxType,
};
use stateset_db::PostgresDatabase;
use stateset_db::postgres::PgTaxRepository;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn connect() -> Option<PostgresDatabase> {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres tax engine test");
        return None;
    };
    Some(PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"))
}

/// A per-run country code so rates from other test binaries on the shared
/// database never stack onto ours (letters only, outside real ISO codes).
fn unique_country() -> String {
    Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .take(2)
        .map(|c| char::from(b'G' + (c.to_digit(16).unwrap_or(0) as u8)))
        .collect()
}

async fn make_jurisdiction(
    repo: &PgTaxRepository,
    country: &str,
    state: Option<&str>,
) -> stateset_core::TaxJurisdiction {
    repo.create_jurisdiction_async(CreateTaxJurisdiction {
        parent_id: None,
        name: format!("Test {country} {}", state.unwrap_or("country")),
        code: state.map_or_else(|| country.to_string(), |s| format!("{country}-{s}")),
        level: if state.is_some() { JurisdictionLevel::State } else { JurisdictionLevel::Country },
        country_code: country.to_string(),
        state_code: state.map(str::to_string),
        county: None,
        city: None,
        postal_codes: vec![],
    })
    .await
    .expect("create jurisdiction")
}

async fn make_rate(repo: &PgTaxRepository, jurisdiction_id: Uuid, rate: Decimal) {
    repo.create_rate_async(CreateTaxRate {
        jurisdiction_id,
        tax_type: TaxType::SalesTax,
        product_category: ProductTaxCategory::Standard,
        rate,
        name: "Sales Tax".into(),
        description: None,
        is_compound: false,
        priority: 1,
        threshold_min: None,
        threshold_max: None,
        fixed_amount: None,
        effective_from: NaiveDate::from_ymd_opt(2026, 1, 1).expect("date"),
        effective_to: None,
    })
    .await
    .expect("create rate");
}

async fn make_customer(db: &PostgresDatabase) -> Uuid {
    db.customers()
        .create_async(CreateCustomer {
            email: format!("tax-{}@example.com", Uuid::new_v4()),
            first_name: "Tax".into(),
            last_name: "Payer".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer")
        .id
        .into_uuid()
}

async fn make_exemption(
    repo: &PgTaxRepository,
    customer_id: Uuid,
    jurisdiction_ids: Vec<Uuid>,
    from: NaiveDate,
) -> stateset_core::TaxExemption {
    repo.create_exemption_async(CreateTaxExemption {
        customer_id,
        exemption_type: ExemptionType::Resale,
        certificate_number: Some("RES-1".into()),
        issuing_authority: None,
        jurisdiction_ids,
        exempt_categories: vec![],
        effective_from: from,
        expires_at: None,
        notes: None,
    })
    .await
    .expect("create exemption")
}

fn request(country: &str, state: &str, prices: &[Decimal]) -> TaxCalculationRequest {
    TaxCalculationRequest {
        line_items: prices
            .iter()
            .enumerate()
            .map(|(i, price)| TaxLineItem {
                id: format!("line-{i}"),
                unit_price: *price,
                ..Default::default()
            })
            .collect(),
        shipping_address: TaxAddress {
            state: Some(state.into()),
            country: country.into(),
            ..Default::default()
        },
        transaction_date: Some(NaiveDate::from_ymd_opt(2026, 6, 1).expect("date")),
        ..Default::default()
    }
}

#[tokio::test]
async fn postgres_calculate_tax_rounds_per_line_and_sums_exactly() {
    let Some(db) = connect().await else { return };
    let repo = db.tax();
    let country = unique_country();
    let j = make_jurisdiction(&repo, &country, Some("RL")).await;
    make_rate(&repo, j.id, dec!(0.0825)).await;

    let mut req = request(&country, "RL", &[dec!(1.11), dec!(1.11), dec!(1.11)]);
    req.shipping_amount = Some(dec!(4.99));
    let res = repo.calculate_tax_async(req).await.expect("calc");
    let lines: Vec<Decimal> = res.line_item_taxes.iter().map(|l| l.tax_amount).collect();
    assert_eq!(lines, vec![dec!(0.09), dec!(0.09), dec!(0.09)], "{res:?}");
    assert_eq!(res.shipping_tax, dec!(0.41));
    assert_eq!(res.total_tax, dec!(0.68));
    assert_eq!(lines.iter().sum::<Decimal>() + res.shipping_tax, res.total_tax);
    assert_eq!(res.tax_breakdown.iter().map(|b| b.tax_amount).sum::<Decimal>(), res.total_tax);
    assert_eq!(res.total, dec!(3.33) + dec!(4.99) + dec!(0.68));
}

#[tokio::test]
async fn postgres_calculate_tax_backs_out_inclusive_prices() {
    let Some(db) = connect().await else { return };
    let repo = db.tax();
    let country = unique_country();
    let j = make_jurisdiction(&repo, &country, Some("IN")).await;
    make_rate(&repo, j.id, dec!(0.19)).await;

    let mut req = request(&country, "IN", &[dec!(19.99)]);
    req.currency = CurrencyCode::EUR;
    req.prices_include_tax = true;
    let res = repo.calculate_tax_async(req.clone()).await.expect("calc");
    assert_eq!(res.total_tax, dec!(3.19), "{res:?}");
    assert_eq!(res.subtotal, dec!(16.80));
    assert_eq!(res.total, dec!(19.99));

    // Store-level inclusive setting has the same effect. Settings are a
    // shared singleton row, so restore them afterwards.
    let original = repo.get_settings_async().await.expect("settings");
    let mut inclusive = original.clone();
    inclusive.calculation_method = TaxCalculationMethod::Inclusive;
    repo.update_settings_async(inclusive).await.expect("update settings");
    req.prices_include_tax = false;
    let res = repo.calculate_tax_async(req).await;
    repo.update_settings_async(original).await.expect("restore settings");
    let res = res.expect("calc");
    assert_eq!(res.total_tax, dec!(3.19));
    assert_eq!(res.total, dec!(19.99));
}

#[tokio::test]
async fn postgres_calculate_tax_ignores_unverified_exemption() {
    let Some(db) = connect().await else { return };
    let repo = db.tax();
    let country = unique_country();
    let j = make_jurisdiction(&repo, &country, Some("UV")).await;
    make_rate(&repo, j.id, dec!(0.05)).await;
    let customer = make_customer(&db).await;
    let exemption =
        make_exemption(&repo, customer, vec![], NaiveDate::from_ymd_opt(2026, 1, 1).expect("d"))
            .await;
    assert!(!exemption.verified, "exemptions are created unverified");

    let mut req = request(&country, "UV", &[dec!(100)]);
    req.customer_id = Some(customer);
    let res = repo.calculate_tax_async(req.clone()).await.expect("calc");
    assert_eq!(res.total_tax, dec!(5.00), "unverified exemption must not apply: {res:?}");
    assert!(!res.exemptions_applied);

    let verified = repo.verify_exemption_async(exemption.id, true).await.expect("verify");
    assert!(verified.verified && verified.verified_at.is_some());
    let res = repo.calculate_tax_async(req).await.expect("calc");
    assert_eq!(res.total_tax, Decimal::ZERO);
    assert!(res.exemptions_applied);
    assert_eq!(res.exemption_details.expect("details").tax_saved, dec!(5.00));
}

#[tokio::test]
async fn postgres_calculate_tax_ignores_exemption_outside_window_at_transaction_date() {
    let Some(db) = connect().await else { return };
    let repo = db.tax();
    let country = unique_country();
    let j = make_jurisdiction(&repo, &country, Some("WN")).await;
    make_rate(&repo, j.id, dec!(0.05)).await;
    let customer = make_customer(&db).await;
    let exemption =
        make_exemption(&repo, customer, vec![], NaiveDate::from_ymd_opt(2026, 7, 1).expect("d"))
            .await;
    repo.verify_exemption_async(exemption.id, true).await.expect("verify");

    let mut req = request(&country, "WN", &[dec!(100)]);
    req.customer_id = Some(customer);
    let res = repo.calculate_tax_async(req.clone()).await.expect("calc");
    assert_eq!(res.total_tax, dec!(5.00), "not yet effective on 2026-06-01: {res:?}");

    req.transaction_date = Some(NaiveDate::from_ymd_opt(2026, 7, 15).expect("date"));
    let res = repo.calculate_tax_async(req).await.expect("calc");
    assert_eq!(res.total_tax, Decimal::ZERO, "effective on 2026-07-15: {res:?}");

    let listed = repo.get_customer_exemptions_async(customer).await.expect("list");
    assert_eq!(listed.len(), 1, "all active exemptions are listed; the engine filters");
}

#[tokio::test]
async fn postgres_calculate_tax_applies_jurisdiction_scoped_exemption_only_there() {
    let Some(db) = connect().await else { return };
    let repo = db.tax();
    let country = unique_country();
    let country_jur = make_jurisdiction(&repo, &country, None).await;
    make_rate(&repo, country_jur.id, dec!(0.05)).await;
    let state = make_jurisdiction(&repo, &country, Some("JS")).await;
    make_rate(&repo, state.id, dec!(0.03)).await;
    let customer = make_customer(&db).await;
    let exemption = make_exemption(
        &repo,
        customer,
        vec![state.id],
        NaiveDate::from_ymd_opt(2026, 1, 1).expect("d"),
    )
    .await;
    repo.verify_exemption_async(exemption.id, true).await.expect("verify");

    let mut req = request(&country, "JS", &[dec!(100)]);
    req.customer_id = Some(customer);
    let res = repo.calculate_tax_async(req).await.expect("calc");
    assert_eq!(res.total_tax, dec!(5.00), "only the state rate is exempt: {res:?}");
    assert!(res.exemptions_applied);
    assert!(!res.line_item_taxes[0].is_exempt);
    assert_eq!(
        res.jurisdictions.iter().map(|j| j.code.as_str()).collect::<Vec<_>>(),
        [country.as_str()]
    );
}

#[tokio::test]
async fn postgres_jurisdiction_codes_are_case_insensitive() {
    let Some(db) = connect().await else { return };
    let repo = db.tax();
    let country = unique_country();
    let lower = country.to_ascii_lowercase();
    let created = make_jurisdiction(&repo, &lower, Some("lc")).await;
    assert_eq!(created.code, format!("{country}-LC"), "stored upper-case");
    assert_eq!(created.country_code, country);
    assert_eq!(created.state_code.as_deref(), Some("LC"));

    for code in [format!("{lower}-lc"), format!("{country}-LC"), format!(" {lower}-Lc ")] {
        let found = repo.get_jurisdiction_by_code_async(&code).await.expect("ok").expect("found");
        assert_eq!(found.id, created.id, "lookup by {code:?}");
    }
    let listed = repo
        .list_jurisdictions_async(TaxJurisdictionFilter {
            country_code: Some(lower.clone()),
            state_code: Some("Lc".into()),
            ..Default::default()
        })
        .await
        .expect("list");
    assert_eq!(listed.len(), 1);

    make_rate(&repo, created.id, dec!(0.10)).await;
    for (c, s) in [(lower.as_str(), "lc"), (country.as_str(), "LC")] {
        let res = repo.calculate_tax_async(request(c, s, &[dec!(10)])).await.expect("calc");
        assert_eq!(res.total_tax, dec!(1.00), "address {c}/{s} must resolve");
    }
}
