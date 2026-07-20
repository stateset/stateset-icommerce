//! Postgres side of the serial `get_available_for_sku` FIFO parity guard.
//!
//! Both backends must allocate the OLDEST available serial first (FIFO). Postgres
//! orders `created_at ASC`; SQLite used to inherit `list`'s newest-first order and
//! hand out the newest unit. This asserts the FIFO order the SQLite backend now
//! matches (see `sqlite/serials.rs::get_available_for_sku_allocates_oldest_first_fifo`).
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use stateset_core::CreateSerialNumber;
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn make_serial(commerce: &AsyncCommerce, sku: &str, serial: &str) -> uuid::Uuid {
    commerce
        .serials()
        .create(CreateSerialNumber {
            serial: Some(serial.to_string()),
            sku: sku.to_string(),
            lot_id: None,
            lot_number: None,
            location_id: None,
            manufactured_at: None,
            notes: None,
            attributes: None,
        })
        .await
        .expect("create serial")
        .id
}

#[tokio::test]
async fn postgres_get_available_for_sku_is_fifo() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping serial FIFO test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    let unique = uuid::Uuid::new_v4().to_string();
    let sku = format!("SKU-FIFO-{}", &unique[..8]);
    let oldest = make_serial(&commerce, &sku, &format!("OLD-{}", &unique[..6])).await;
    let newest = make_serial(&commerce, &sku, &format!("NEW-{}", &unique[..6])).await;

    let one = commerce.serials().get_available_for_sku(&sku, 1).await.expect("available");
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].id, oldest, "must allocate the oldest serial first (FIFO)");

    let all = commerce.serials().get_available_for_sku(&sku, 10).await.expect("all");
    assert_eq!(
        all.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![oldest, newest],
        "available serials must be FIFO-ordered"
    );
}
