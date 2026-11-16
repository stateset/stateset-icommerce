use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;
#[path = "erp_manufacturing.rs"]
mod manufacturing;

pub(crate) fn linked_statuses(db: &dyn Database) {
    let (mut input, _) = manufacturing::input(db);
    input.quantity_to_build = dec!(1.25);
    let first = db.work_orders().create(input.clone()).unwrap();
    let second = db.work_orders().create(input).unwrap();
    let make = |ids| CreateProductionBatch {
        name: format!("BATCH-{}", Uuid::new_v4()),
        vendor_id: None,
        work_order_ids: ids,
        notes: None,
        scheduled_start: None,
        scheduled_end: None,
    };
    assert!(db.production_batches().create(make(vec![first.id, Uuid::new_v4()])).is_err());
    let batch = db.production_batches().create(make(vec![first.id, second.id, first.id])).unwrap();
    assert_eq!(batch.work_order_count(), 2);
    assert_eq!(batch.status, ProductionBatchStatus::Planned);
    assert!(db.production_batches().add_work_orders(batch.id, vec![Uuid::new_v4()]).is_err());
    assert_eq!(db.production_batches().get(batch.id).unwrap().unwrap().work_order_count(), 2);
    let changed = db
        .work_orders()
        .update(
            first.id,
            UpdateWorkOrder { quantity_to_build: Some(dec!(2.25)), ..Default::default() },
        )
        .unwrap();
    assert_eq!(changed.quantity_to_build, dec!(2.25));
    assert_eq!(changed.materials[0].reserved_quantity, dec!(4.5));
    db.work_orders().start(first.id).unwrap();
    assert_eq!(
        db.production_batches().get(batch.id).unwrap().unwrap().status,
        ProductionBatchStatus::InProgress
    );
    assert!(db.production_batches().remove_work_order(batch.id, second.id).is_err());
    assert!(
        db.production_batches()
            .update(
                batch.id,
                UpdateProductionBatch {
                    status: Some(ProductionBatchStatus::Completed),
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert!(
        db.production_batches()
            .update(
                batch.id,
                UpdateProductionBatch {
                    status: Some(ProductionBatchStatus::Cancelled),
                    ..Default::default()
                }
            )
            .is_err()
    );
    db.work_orders().complete(first.id, dec!(1.125)).unwrap();
    db.work_orders().complete(first.id, dec!(1.125)).unwrap();
    assert_eq!(
        db.production_batches().get(batch.id).unwrap().unwrap().status,
        ProductionBatchStatus::InProgress
    );
    db.work_orders().start(second.id).unwrap();
    db.work_orders().complete(second.id, dec!(1.25)).unwrap();
    assert_eq!(
        db.production_batches().get(batch.id).unwrap().unwrap().status,
        ProductionBatchStatus::Completed
    );
    assert!(
        db.production_batches()
            .list(ProductionBatchFilter {
                status: Some(ProductionBatchStatus::Completed),
                ..Default::default()
            })
            .unwrap()
            .iter()
            .any(|row| row.id == batch.id)
    );
    assert!(db.production_batches().delete(batch.id).is_err());
    let empty = db.production_batches().create(make(vec![])).unwrap();
    let cancelled = db
        .production_batches()
        .update(
            empty.id,
            UpdateProductionBatch {
                status: Some(ProductionBatchStatus::Cancelled),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(cancelled.status, ProductionBatchStatus::Cancelled);
    assert!(db.production_batches().add_work_orders(empty.id, vec![first.id]).is_err());
}
