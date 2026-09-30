#![cfg(feature = "postgres")]

//! A fresh Postgres store seeds the same tax jurisdictions and rates as a
//! fresh SQLite store (migration 105), so it charges tax instead of zero.
//!
//! The Canadian cases mirror `canada_tax_test.rs` (the SQLite version) and
//! assert the same amounts. Each test migrates its OWN throwaway database: the
//! seed only runs on a store with no tax rates, and the shared test database
//! already has rates other tests created.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`) whose
//! role may create databases; skipped when neither variable is set.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use sqlx::Connection;
use stateset_embedded::{
    AsyncCommerce, ProductTaxCategory, TaxAddress, TaxCalculationRequest, TaxLineItem,
};

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

/// A freshly created, empty database, dropped when the value is dropped.
struct FreshDatabase {
    admin_url: String,
    name: String,
    url: String,
}

impl FreshDatabase {
    async fn create(admin_url: &str) -> Self {
        let name = format!("stateset_taxseed_{}", uuid::Uuid::new_v4().simple());
        let mut conn = sqlx::PgConnection::connect(admin_url).await.expect("connect admin");
        sqlx::raw_sql(&format!("CREATE DATABASE \"{name}\""))
            .execute(&mut conn)
            .await
            .expect("create fresh database");
        conn.close().await.ok();
        let url = with_database(admin_url, &name);
        Self { admin_url: admin_url.to_string(), name, url }
    }
}

impl Drop for FreshDatabase {
    fn drop(&mut self) {
        let admin_url = self.admin_url.clone();
        let name = self.name.clone();
        // Drop from a separate runtime: the test's runtime is shutting down.
        let _ = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().ok()?;
            rt.block_on(async {
                let mut conn = sqlx::PgConnection::connect(&admin_url).await.ok()?;
                sqlx::raw_sql(&format!("DROP DATABASE IF EXISTS \"{name}\" WITH (FORCE)"))
                    .execute(&mut conn)
                    .await
                    .ok()
            })
        })
        .join();
    }
}

/// `url` with its database name replaced by `name`.
fn with_database(url: &str, name: &str) -> String {
    let (base, query) = url.split_once('?').map_or((url, None), |(b, q)| (b, Some(q)));
    let scheme_end = base.find("://").map_or(0, |i| i + 3);
    let path_start = base[scheme_end..].find('/').map_or(base.len(), |i| scheme_end + i);
    let mut out = format!("{}/{name}", &base[..path_start]);
    if let Some(query) = query {
        out.push('?');
        out.push_str(query);
    }
    out
}

async fn fresh_store() -> Option<(FreshDatabase, AsyncCommerce)> {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return None;
    };
    let db = FreshDatabase::create(&url).await;
    let commerce = AsyncCommerce::connect(&db.url).await.expect("connect + migrate fresh store");
    Some((db, commerce))
}

fn sale_in(country: &str, state: &str, on: NaiveDate) -> TaxCalculationRequest {
    TaxCalculationRequest {
        line_items: vec![TaxLineItem {
            id: "item-1".into(),
            quantity: dec!(1),
            unit_price: dec!(100),
            tax_category: ProductTaxCategory::Standard,
            ..Default::default()
        }],
        shipping_address: TaxAddress {
            country: country.into(),
            state: Some(state.into()),
            ..Default::default()
        },
        transaction_date: Some(on),
        ..Default::default()
    }
}

async fn tax_on_100(
    commerce: &AsyncCommerce,
    country: &str,
    state: &str,
    on: NaiveDate,
) -> Decimal {
    commerce.tax().calculate_tax(sale_in(country, state, on)).await.expect("calculate").total_tax
}

fn today() -> NaiveDate {
    chrono::Utc::now().date_naive()
}

#[tokio::test]
async fn fresh_postgres_store_charges_seeded_us_state_tax() {
    let Some((_db, commerce)) = fresh_store().await else { return };
    assert_eq!(tax_on_100(&commerce, "US", "CA", today()).await, dec!(7.25), "California 7.25%");
    assert_eq!(tax_on_100(&commerce, "US", "OR", today()).await, dec!(0), "Oregon: no sales tax");
}

#[tokio::test]
async fn fresh_postgres_store_charges_canadian_sales_tax() {
    let Some((_db, commerce)) = fresh_store().await else { return };
    assert_eq!(tax_on_100(&commerce, "CA", "ON", today()).await, dec!(13.00), "ON: HST only");
    assert_eq!(tax_on_100(&commerce, "CA", "NB", today()).await, dec!(15.00), "NB HST");
    assert_eq!(tax_on_100(&commerce, "CA", "NL", today()).await, dec!(15.00), "NL HST");
    assert_eq!(tax_on_100(&commerce, "CA", "PE", today()).await, dec!(15.00), "PE HST");
    assert_eq!(tax_on_100(&commerce, "CA", "NS", today()).await, dec!(14.00), "NS HST today");
    let april_2025 = NaiveDate::from_ymd_opt(2025, 4, 1).expect("date");
    assert_eq!(
        tax_on_100(&commerce, "CA", "NS", april_2025).await,
        dec!(14.00),
        "NS from 2025-04-01"
    );
    let march_2025 = NaiveDate::from_ymd_opt(2025, 3, 31).expect("date");
    assert_eq!(
        tax_on_100(&commerce, "CA", "NS", march_2025).await,
        dec!(15.00),
        "NS until 2025-03-31"
    );
    assert_eq!(tax_on_100(&commerce, "CA", "AB", today()).await, dec!(5.00), "AB: GST only");
    for territory in ["NT", "NU", "YT"] {
        assert_eq!(
            tax_on_100(&commerce, "CA", territory, today()).await,
            dec!(5.00),
            "{territory}: GST only"
        );
    }
    assert_eq!(tax_on_100(&commerce, "CA", "BC", today()).await, dec!(12.00), "BC: GST + PST");
    assert_eq!(tax_on_100(&commerce, "CA", "SK", today()).await, dec!(11.00), "SK: GST + PST");
    assert_eq!(tax_on_100(&commerce, "CA", "MB", today()).await, dec!(12.00), "MB: GST + RST");
    assert_eq!(
        tax_on_100(&commerce, "CA", "QC", today()).await,
        dec!(14.98),
        "QC: GST 5% + QST 9.975%, not compounded"
    );
}

#[tokio::test]
async fn rerunning_the_seed_on_a_store_with_rates_adds_nothing() {
    let Some((db, commerce)) = fresh_store().await else { return };
    let seeded: i64 = {
        let mut conn = sqlx::PgConnection::connect(&db.url).await.expect("connect");
        // Re-running the seed on a store that now has rates must add nothing.
        let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tax_rates")
            .fetch_one(&mut conn)
            .await
            .expect("count");
        let sql = include_str!("../../stateset-db/src/postgres/migrations/105_seed_tax_rates.sql");
        sqlx::raw_sql(sql).execute(&mut conn).await.expect("re-run seed");
        let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tax_rates")
            .fetch_one(&mut conn)
            .await
            .expect("count");
        assert_eq!(before, after, "re-running the seed must not duplicate rates");
        before
    };
    assert!(seeded > 0, "a fresh store must be seeded");
    drop(commerce);
}

/// The same jurisdictions and the same active rates as a fresh SQLite store.
#[cfg(feature = "sqlite")]
#[tokio::test]
async fn fresh_postgres_and_sqlite_stores_seed_the_same_tax_tables() {
    use stateset_embedded::{Commerce, TaxJurisdictionFilter, TaxRateFilter};
    use std::collections::{BTreeSet, HashMap};

    let Some((_db, pg)) = fresh_store().await else { return };
    let sqlite = Commerce::new(":memory:").expect("sqlite store");

    let all_jurisdictions = TaxJurisdictionFilter { limit: Some(1000), ..Default::default() };
    let pg_j = pg.tax().list_jurisdictions(all_jurisdictions.clone()).await.expect("pg list");
    let sq_j = sqlite.tax().list_jurisdictions(all_jurisdictions).expect("sqlite list");

    let jurisdiction_key = |j: &stateset_embedded::TaxJurisdiction,
                            by_id: &HashMap<uuid::Uuid, String>| {
        (
            j.code.clone(),
            j.name.clone(),
            j.level.to_string(),
            j.country_code.clone(),
            j.state_code.clone(),
            j.parent_id.and_then(|p| by_id.get(&p).cloned()),
            j.active,
        )
    };
    let pg_codes: HashMap<_, _> = pg_j.iter().map(|j| (j.id, j.code.clone())).collect();
    let sq_codes: HashMap<_, _> = sq_j.iter().map(|j| (j.id, j.code.clone())).collect();
    let pg_set: BTreeSet<_> = pg_j.iter().map(|j| jurisdiction_key(j, &pg_codes)).collect();
    let sq_set: BTreeSet<_> = sq_j.iter().map(|j| jurisdiction_key(j, &sq_codes)).collect();
    assert_eq!(pg_j.len(), sq_j.len(), "jurisdiction count");
    assert_eq!(pg_set, sq_set, "jurisdictions (with parent links)");

    let rate_key = |r: &stateset_embedded::TaxRate, codes: &HashMap<uuid::Uuid, String>| {
        (
            codes.get(&r.jurisdiction_id).cloned().unwrap_or_default(),
            r.tax_type.to_string(),
            r.product_category.to_string(),
            r.rate.normalize(),
            r.name.clone(),
            r.is_compound,
            r.effective_to,
        )
    };
    let active = TaxRateFilter { active_only: true, limit: Some(1000), ..Default::default() };
    let pg_rates = pg.tax().list_rates(active.clone()).await.expect("pg rates");
    let sq_rates = sqlite.tax().list_rates(active).expect("sqlite rates");
    let pg_set: BTreeSet<_> = pg_rates.iter().map(|r| rate_key(r, &pg_codes)).collect();
    let sq_set: BTreeSet<_> = sq_rates.iter().map(|r| rate_key(r, &sq_codes)).collect();
    assert_eq!(pg_rates.len(), sq_rates.len(), "active rate count");
    assert_eq!(pg_set, sq_set, "active rates");

    // The rates in force today agree on their start dates too.
    let in_force = TaxRateFilter {
        active_only: true,
        effective_date: Some(today()),
        limit: Some(1000),
        ..Default::default()
    };
    let dated = |rates: Vec<stateset_embedded::TaxRate>, codes: &HashMap<uuid::Uuid, String>| {
        rates.iter().map(|r| (rate_key(r, codes), r.effective_from)).collect::<BTreeSet<_>>()
    };
    let pg_today = dated(pg.tax().list_rates(in_force.clone()).await.expect("pg"), &pg_codes);
    let sq_today = dated(sqlite.tax().list_rates(in_force).expect("sqlite"), &sq_codes);
    assert_eq!(pg_today, sq_today, "rates in force today");
}
