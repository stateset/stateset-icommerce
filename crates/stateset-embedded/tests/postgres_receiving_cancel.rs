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
    CostMethod, CreateLocation, CreateReceipt, CreateReceiptItem, CreateWarehouse, LocationType,
    ReceiptItemStatus, ReceiptStatus, ReceiptType, ReceiveItemLine, ReceiveItems, SetItemCost,
    WarehouseType,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    stateset_test_utils::postgres_url()
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
    // Received stock posts to a receiving location, and the shared database
    // may have ledger posting configured, so the received SKU carries a cost.
    let sku_a = format!("ITEM-A-{}", &unique[..8]);
    let sku_b = format!("ITEM-B-{}", &unique[..8]);
    commerce
        .cost_accounting()
        .set_item_cost(SetItemCost {
            sku: sku_a.clone(),
            cost_method: Some(CostMethod::Average),
            standard_cost: Some(dec!(1)),
            ..Default::default()
        })
        .await
        .expect("cost ITEM-A");
    let dock = commerce
        .warehouse()
        .create_location(CreateLocation {
            warehouse_id: warehouse.id,
            location_type: LocationType::Receiving,
            is_receivable: Some(true),
            ..Default::default()
        })
        .await
        .expect("create receiving location");

    let receipt = commerce
        .receiving()
        .create_receipt(CreateReceipt {
            receipt_type: ReceiptType::PurchaseOrder,
            warehouse_id: warehouse.id,
            items: vec![
                CreateReceiptItem {
                    sku: sku_a.clone(),
                    expected_quantity: dec!(5),
                    ..Default::default()
                },
                CreateReceiptItem {
                    sku: sku_b.clone(),
                    expected_quantity: dec!(3),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .await
        .expect("create receipt with items");

    commerce.receiving().start_receiving(receipt.id).await.expect("start");
    let lines =
        commerce.receiving().get_receipt_items(receipt.id).await.expect("get receipt items");
    let item_a = lines.iter().find(|i| i.sku == sku_a).expect("ITEM-A line").id;
    commerce
        .receiving()
        .receive_items(ReceiveItems {
            receipt_id: receipt.id,
            items: vec![ReceiveItemLine {
                receipt_item_id: item_a,
                quantity_received: dec!(5),
                quantity_rejected: None,
                rejection_reason: None,
                lot_number: None,
                serial_numbers: None,
                expiration_date: None,
                notes: None,
            }],
            receiving_location_id: Some(dock.id),
            received_by: None,
        })
        .await
        .expect("receive ITEM-A");
    commerce.receiving().complete_receiving(receipt.id).await.expect("complete");

    // A line is received only if it received stock; an untouched line stays pending.
    let items =
        commerce.receiving().get_receipt_items(receipt.id).await.expect("get receipt items");
    assert_eq!(items.len(), 2);
    let status = |sku: &str| items.iter().find(|i| i.sku == sku).expect("line").status;
    assert_eq!(status(&sku_a), ReceiptItemStatus::Received);
    assert_eq!(status(&sku_b), ReceiptItemStatus::Pending);
}
