//! `validate_batch_size` parity between the SQLite and Postgres backends.
//!
//! `MAX_BATCH_SIZE` is 1000. The Postgres bulk creates for receiving, serials,
//! lots and fulfillment all called `validate_batch_size` up front; the SQLite
//! twins called it nowhere. A 250,000-item `create_batch` was therefore rejected
//! on Postgres but, on SQLite, looped 250,000 individual `BEGIN IMMEDIATE`
//! writes — holding the single writer lock for minutes and starving every
//! concurrent checkout on the same file.
//!
//! These tests assert the guard fires on BOTH backends. The Postgres half is
//! skipped without `POSTGRES_URL` / `DATABASE_URL`.

use stateset_core::{
    CommerceError, CreateLot, CreateReceipt, CreateSerialNumber, CreateWave, MAX_BATCH_SIZE,
};

/// One over the limit: large enough to be rejected, small enough that a missing
/// guard still finishes the test in reasonable time rather than hanging CI.
const fn oversized() -> usize {
    MAX_BATCH_SIZE + 1
}

fn receipts() -> Vec<CreateReceipt> {
    (0..oversized()).map(|_| CreateReceipt { warehouse_id: 1, ..Default::default() }).collect()
}

fn serials() -> Vec<CreateSerialNumber> {
    (0..oversized())
        .map(|i| CreateSerialNumber { sku: format!("BATCH-SKU-{i}"), ..Default::default() })
        .collect()
}

fn lots() -> Vec<CreateLot> {
    (0..oversized())
        .map(|i| CreateLot {
            sku: format!("BATCH-SKU-{i}"),
            quantity: rust_decimal::Decimal::ONE,
            ..Default::default()
        })
        .collect()
}

fn waves() -> Vec<CreateWave> {
    (0..oversized()).map(|_| CreateWave { warehouse_id: 1, ..Default::default() }).collect()
}

/// Assert the error is the `validate_batch_size` rejection and not some other
/// validation failure that happens to fire first.
#[track_caller]
fn assert_batch_size_rejection<T>(label: &str, result: stateset_core::Result<T>) {
    match result {
        Err(CommerceError::ValidationError(msg)) => assert!(
            msg.contains("Batch size") && msg.contains(&MAX_BATCH_SIZE.to_string()),
            "{label}: expected a batch-size rejection, got {msg}"
        ),
        Err(other) => panic!("{label}: expected a batch-size ValidationError, got {other}"),
        Ok(_) => panic!(
            "{label}: an oversized batch of {} was accepted; validate_batch_size is missing",
            oversized()
        ),
    }
}

#[cfg(feature = "sqlite")]
mod sqlite {
    use super::{assert_batch_size_rejection, lots, receipts, serials, waves};
    use stateset_core::{
        FulfillmentRepository, LotRepository, ReceivingRepository, SerialRepository,
    };
    use stateset_db::SqliteDatabase;

    fn db() -> SqliteDatabase {
        SqliteDatabase::in_memory().expect("in-memory db")
    }

    #[test]
    fn sqlite_create_receipts_batch_rejects_oversized_batch() {
        assert_batch_size_rejection(
            "sqlite receiving",
            db().receiving().create_receipts_batch(receipts()),
        );
    }

    #[test]
    fn sqlite_serial_create_batch_rejects_oversized_batch() {
        assert_batch_size_rejection("sqlite serials", db().serials().create_batch(serials()));
    }

    #[test]
    fn sqlite_lot_create_batch_rejects_oversized_batch() {
        assert_batch_size_rejection("sqlite lots", db().lots().create_batch(lots()));
    }

    #[test]
    fn sqlite_create_waves_batch_rejects_oversized_batch() {
        assert_batch_size_rejection(
            "sqlite fulfillment",
            db().fulfillment().create_waves_batch(waves()),
        );
    }
}

#[cfg(feature = "postgres")]
mod postgres {
    use super::{assert_batch_size_rejection, lots, receipts, serials, waves};
    use stateset_db::PostgresDatabase;

    fn postgres_url() -> Option<String> {
        std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
    }

    #[tokio::test]
    async fn postgres_bulk_creates_reject_oversized_batches() {
        let Some(url) = postgres_url() else {
            eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
            return;
        };
        let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");

        assert_batch_size_rejection(
            "postgres receiving",
            db.receiving().create_receipts_batch_async(receipts()).await,
        );
        assert_batch_size_rejection(
            "postgres serials",
            db.serials().create_batch_async(serials()).await,
        );
        assert_batch_size_rejection("postgres lots", db.lots().create_batch_async(lots()).await);
        assert_batch_size_rejection(
            "postgres fulfillment",
            db.fulfillment().create_waves_batch_async(waves()).await,
        );
    }
}
