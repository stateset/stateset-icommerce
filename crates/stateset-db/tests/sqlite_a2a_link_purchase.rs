//! `link_purchase_to_order` is conditional: it refuses terminal purchases and
//! never silently re-points a purchase at a different order.
#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_core::{
    A2ACommerceRepository, CommerceError, CreateA2APurchase, ItemAvailability, PurchaseStatus,
    QuotedItem,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

fn purchase(db: &SqliteDatabase) -> Uuid {
    db.a2a_purchases()
        .create_purchase(CreateA2APurchase {
            buyer_agent_id: Uuid::new_v4(),
            seller_agent_id: Uuid::new_v4(),
            items: vec![QuotedItem {
                line_number: 1,
                sku: Some("SKU-1".into()),
                name: "Widget".into(),
                quantity: 1,
                unit_price: dec!(10),
                total: dec!(10),
                availability: ItemAvailability::InStock,
                lead_time_days: None,
            }],
            total: dec!(10),
            ..Default::default()
        })
        .expect("create purchase")
        .id
}

#[test]
fn sqlite_link_purchase_to_order_is_idempotent_and_refuses_relink() {
    let db = SqliteDatabase::in_memory().expect("db");
    let repo = db.a2a_purchases();
    let id = purchase(&db);
    let order = Uuid::new_v4();

    let linked = repo.link_purchase_to_order(id, order).expect("link");
    assert_eq!(linked.order_id, Some(order));
    // Same order again is a no-op.
    let again = repo.link_purchase_to_order(id, order).expect("idempotent relink");
    assert_eq!(again.order_id, Some(order));
    // A different order is refused.
    let err = repo.link_purchase_to_order(id, Uuid::new_v4()).expect_err("relink refused");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    assert_eq!(repo.get_purchase(id).unwrap().unwrap().order_id, Some(order));

    // Unknown purchase.
    let err = repo.link_purchase_to_order(Uuid::new_v4(), order).expect_err("missing");
    assert!(matches!(err, CommerceError::NotFound), "{err:?}");
}

#[test]
fn sqlite_link_purchase_to_order_refuses_terminal_purchases() {
    let db = SqliteDatabase::in_memory().expect("db");
    let repo = db.a2a_purchases();

    let cancelled = purchase(&db);
    repo.update_purchase_status(cancelled, PurchaseStatus::Cancelled).expect("cancel");
    let err = repo.link_purchase_to_order(cancelled, Uuid::new_v4()).expect_err("cancelled");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    assert_eq!(repo.get_purchase(cancelled).unwrap().unwrap().order_id, None);

    let completed = purchase(&db);
    for next in [
        PurchaseStatus::PaymentPending,
        PurchaseStatus::Paid,
        PurchaseStatus::Shipped,
        PurchaseStatus::Delivered,
        PurchaseStatus::Completed,
    ] {
        repo.update_purchase_status(completed, next).expect("walk");
    }
    let err = repo.link_purchase_to_order(completed, Uuid::new_v4()).expect_err("completed");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
}
