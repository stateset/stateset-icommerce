//! Competing processors may record only one terminal EDI result.
//! Requires `POSTGRES_URL` or `DATABASE_URL`; skipped otherwise.

#![cfg(feature = "postgres")]

use stateset_core::{CreateEdiDocument, EdiDirection, EdiStatus};
use stateset_db::PostgresDatabase;
use std::sync::Arc;
use tokio::sync::Barrier;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_competing_edi_terminal_updates_choose_one_result() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let doc = db
        .edi_documents()
        .create_async(CreateEdiDocument {
            document_type: "850".into(),
            direction: EdiDirection::Inbound,
            partner: None,
            reference: None,
            payload: None,
        })
        .await
        .expect("create document");
    let barrier = Arc::new(Barrier::new(2));
    let mut handles = Vec::new();
    for status in [EdiStatus::Processed, EdiStatus::Acknowledged] {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let id = doc.id;
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            db.edi_documents().set_status_async(id, status, None).await
        }));
    }
    let mut successes = Vec::new();
    for handle in handles {
        if let Ok(updated) = handle.await.expect("join processor") {
            successes.push(updated.status);
        }
    }
    assert_eq!(successes.len(), 1);
    let stored = db.edi_documents().get_async(doc.id).await.expect("get").expect("document");
    assert_eq!(stored.status, successes[0]);
}
