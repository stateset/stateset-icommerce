#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_core::{CreateWorkOrder, ProductId, WorkOrderRepository, WorkOrderStatus};
use stateset_db::SqliteDatabase;

#[test]
fn sqlite_work_order_complete_transitions_partial_then_completed() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let repo = db.work_orders();

    let work_order = repo
        .create(CreateWorkOrder {
            product_id: ProductId::new(),
            quantity_to_build: dec!(100),
            ..Default::default()
        })
        .expect("create work order");

    let started = repo.start(work_order.id).expect("start work order");
    assert_eq!(started.status, WorkOrderStatus::InProgress);

    let partial = repo.complete(work_order.id, dec!(98)).expect("partially complete work order");
    assert_eq!(partial.status, WorkOrderStatus::PartiallyCompleted);
    assert_eq!(partial.quantity_completed, dec!(98));

    let completed = repo.complete(work_order.id, dec!(2)).expect("finish work order");
    assert_eq!(completed.status, WorkOrderStatus::Completed);
    assert_eq!(completed.quantity_completed, dec!(100));
    assert!(completed.actual_end.is_some());
}
