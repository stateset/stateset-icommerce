//! Postgres serial `delete` must enforce the same guards as SQLite.
//!
//! SQLite's `delete` rejects a missing serial (`NotFound`) and a non-`Available`
//! serial (`ValidationError`) before deleting. Postgres's `delete_async` had
//! neither guard — it returned `Ok(())` for a missing id and permanently deleted
//! a sold serial (data loss). Postgres now mirrors the SQLite guards.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use stateset_core::{CommerceError, CreateSerialNumber, SerialStatus, UpdateSerialNumber};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_delete_missing_serial_is_notfound() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping serial delete test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    let err = commerce
        .serials()
        .delete(uuid::Uuid::new_v4())
        .await
        .expect_err("deleting a non-existent serial must be an error, not Ok");
    assert!(matches!(err, CommerceError::NotFound), "expected NotFound, got {err:?}");
}

#[tokio::test]
async fn postgres_cannot_delete_non_available_serial() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping serial delete test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    let unique = uuid::Uuid::new_v4().to_string();
    let serial = commerce
        .serials()
        .create(CreateSerialNumber {
            serial: Some(format!("SN-{}", &unique[..12])),
            sku: "WIDGET".into(),
            lot_id: None,
            lot_number: None,
            location_id: None,
            manufactured_at: None,
            notes: None,
            attributes: None,
        })
        .await
        .expect("create serial");

    // Move it out of Available without writing post-creation history.
    commerce
        .serials()
        .update(
            serial.id,
            UpdateSerialNumber { status: Some(SerialStatus::Sold), ..Default::default() },
        )
        .await
        .expect("update to sold");

    let err = commerce
        .serials()
        .delete(serial.id)
        .await
        .expect_err("deleting a sold serial must be rejected");
    assert!(
        matches!(err, CommerceError::ValidationError(_)),
        "expected ValidationError, got {err:?}"
    );

    // The sold serial must still exist — deletion must not have destroyed it.
    let still_there = commerce.serials().get(serial.id).await.expect("get");
    assert!(still_there.is_some(), "a rejected delete must not remove the serial");
    assert_eq!(still_there.unwrap().status, SerialStatus::Sold);
}
