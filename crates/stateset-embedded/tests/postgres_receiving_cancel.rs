//! Postgres parity for the `cancel_receipt` guard.
//!
//! Cancellation is now gated on the shared `ReceiptStatus::can_cancel()`
//! (cancellable only from `Expected`/`InProgress`) on both backends. SQLite
//! historically checked the wrong status (`Completed`, never reached by the
//! normal flow) so a received receipt was cancellable; Postgres checked only
//! `Received`. This pins the converged Postgres behavior.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    CreateReceipt, CreateReceiptItem, CreateWarehouse, ReceiptItemStatus, ReceiptStatus,
    ReceiptType, WarehouseType,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_cancel_receipt_guards() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let unique = uuid::Uuid::new_v4().to_string();

    let warehouse = commerce
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: format!("WH-{}", &unique[..8]),
            name: "Recv WH".into(),
            warehouse_type: WarehouseType::Distribution,
            ..Default::default()
        })
        .await
        .expect("create warehouse");

    let mk = || async {
        commerce
            .receiving()
            .create_receipt(CreateReceipt {
                receipt_type: ReceiptType::PurchaseOrder,
                warehouse_id: warehouse.id,
                ..Default::default()
            })
            .await
            .expect("create receipt")
    };

    // Received (goods on hand) → not cancellable.
    let received = mk().await;
    commerce.receiving().start_receiving(received.id).await.expect("start");
    commerce.receiving().complete_receiving(received.id).await.expect("complete");
    let err = commerce
        .receiving()
        .cancel_receipt(received.id)
        .await
        .expect_err("a received receipt must not be cancellable");
    assert!(matches!(err, stateset_core::CommerceError::ValidationError(_)), "got {err:?}");

    // In progress → cancellable.
    let in_prog = mk().await;
    commerce.receiving().start_receiving(in_prog.id).await.expect("start");
    let cancelled =
        commerce.receiving().cancel_receipt(in_prog.id).await.expect("in-progress is cancellable");
    assert_eq!(cancelled.status, ReceiptStatus::Cancelled);

    // Already cancelled → not cancellable again.
    let err = commerce
        .receiving()
        .cancel_receipt(in_prog.id)
        .await
        .expect_err("an already-cancelled receipt must not be cancellable again");
    assert!(matches!(err, stateset_core::CommerceError::ValidationError(_)), "got {err:?}");
}

/// Completing a receipt must mark its non-rejected line items `received`
/// (matching the SQLite backend, which the Postgres path previously did not do —
/// it only updated the receipt header, leaving items in their prior status).
#[tokio::test]
async fn postgres_complete_receiving_marks_items_received() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let unique = uuid::Uuid::new_v4().to_string();

    let warehouse = commerce
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: format!("WHI-{}", &unique[..8]),
            name: "Recv Items WH".into(),
            warehouse_type: WarehouseType::Distribution,
            ..Default::default()
        })
        .await
        .expect("create warehouse");

    let receipt = commerce
        .receiving()
        .create_receipt(CreateReceipt {
            receipt_type: ReceiptType::PurchaseOrder,
            warehouse_id: warehouse.id,
            items: vec![
                CreateReceiptItem {
                    sku: "ITEM-A".into(),
                    expected_quantity: dec!(5),
                    ..Default::default()
                },
                CreateReceiptItem {
                    sku: "ITEM-B".into(),
                    expected_quantity: dec!(3),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .await
        .expect("create receipt with items");

    commerce.receiving().start_receiving(receipt.id).await.expect("start");
    commerce.receiving().complete_receiving(receipt.id).await.expect("complete");

    let items =
        commerce.receiving().get_receipt_items(receipt.id).await.expect("get receipt items");
    assert_eq!(items.len(), 2);
    for item in &items {
        assert_eq!(
            item.status,
            ReceiptItemStatus::Received,
            "item {} must be marked received on completion",
            item.sku
        );
    }
}
