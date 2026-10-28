use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;

pub(crate) fn filters(db: &dyn Database) {
    let repo = db.cost_accounting();
    let sku = "FILTERED";
    repo.set_item_cost(SetItemCost {
        sku: sku.into(),
        standard_cost: Some(dec!(2)),
        ..Default::default()
    })
    .unwrap();
    for i in 0..3 {
        repo.create_cost_layer(CreateCostLayer {
            sku: sku.into(),
            quantity: dec!(1),
            unit_cost: dec!(2),
            source_type: if i == 1 { CostLayerSource::Opening } else { CostLayerSource::Purchase },
            source_id: None,
            lot_id: None,
            location_id: None,
        })
        .unwrap();
        repo.record_cost_transaction(
            sku,
            if i == 1 { CostTransactionType::Adjustment } else { CostTransactionType::Receipt },
            dec!(1),
            dec!(2),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        repo.record_variance(RecordCostVariance {
            sku: sku.into(),
            variance_type: if i == 1 { VarianceType::Material } else { VarianceType::Purchase },
            standard_cost: dec!(2),
            actual_cost: dec!(3),
            quantity: dec!(1),
            reference_type: None,
            reference_id: None,
            notes: None,
        })
        .unwrap();
        repo.create_adjustment(CreateCostAdjustment {
            sku: sku.into(),
            adjustment_type: if i == 1 {
                CostAdjustmentType::Correction
            } else {
                CostAdjustmentType::Revaluation
            },
            new_cost: dec!(3),
            reason: "Filter fixture".into(),
            created_by: None,
        })
        .unwrap();
    }
    repo.issue_fifo(IssueCostLayers {
        sku: sku.into(),
        quantity: dec!(1),
        reference_type: None,
        reference_id: None,
        notes: None,
    })
    .unwrap();
    let layers = repo.list_cost_layers(CostLayerFilter::default()).unwrap();
    assert_eq!(layers.len(), 3);
    for has in [None, Some(true), Some(false)] {
        let all = repo
            .list_cost_layers(CostLayerFilter { has_remaining: has, ..Default::default() })
            .unwrap();
        let page = repo
            .list_cost_layers(CostLayerFilter {
                has_remaining: has,
                offset: Some(1),
                limit: Some(1),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            page.iter().map(|l| l.id).collect::<Vec<_>>(),
            all.iter().skip(1).take(1).map(|l| l.id).collect::<Vec<_>>()
        );
    }
    assert_eq!(
        repo.list_cost_layers(CostLayerFilter { has_remaining: Some(false), ..Default::default() })
            .unwrap()
            .len(),
        1
    );
    let exact = repo
        .list_cost_layers(CostLayerFilter {
            from_date: Some(layers[1].layer_date),
            to_date: Some(layers[1].layer_date),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(exact.len(), 1);
    assert_eq!(exact[0].id, layers[1].id);
    assert_eq!(
        repo.list_cost_layers(CostLayerFilter {
            source_type: Some(CostLayerSource::Opening),
            ..Default::default()
        })
        .unwrap()
        .len(),
        1
    );
    let txs = repo.list_cost_transactions(CostTransactionFilter::default()).unwrap();
    let page = repo
        .list_cost_transactions(CostTransactionFilter {
            offset: Some(1),
            limit: Some(1),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(page[0].id, txs[1].id);
    let exact = repo
        .list_cost_transactions(CostTransactionFilter {
            from_date: Some(txs[1].created_at),
            to_date: Some(txs[1].created_at),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(exact.len(), 1);
    assert_eq!(exact[0].id, txs[1].id);
    assert_eq!(
        repo.list_cost_transactions(CostTransactionFilter {
            transaction_type: Some(CostTransactionType::Receipt),
            offset: Some(1),
            ..Default::default()
        })
        .unwrap()
        .len(),
        1
    );
    let vars = repo.list_variances(CostVarianceFilter::default()).unwrap();
    assert_eq!(
        repo.list_variances(CostVarianceFilter {
            offset: Some(1),
            limit: Some(1),
            ..Default::default()
        })
        .unwrap()[0]
            .id,
        vars[1].id
    );
    let exact = repo
        .list_variances(CostVarianceFilter {
            from_date: Some(vars[1].variance_date),
            to_date: Some(vars[1].variance_date),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(exact.len(), 1);
    assert_eq!(exact[0].id, vars[1].id);
    assert_eq!(
        repo.list_variances(CostVarianceFilter {
            variance_type: Some(VarianceType::Purchase),
            offset: Some(1),
            ..Default::default()
        })
        .unwrap()
        .len(),
        1
    );
    let adj = repo.list_adjustments(CostAdjustmentFilter::default()).unwrap();
    assert_eq!(
        repo.list_adjustments(CostAdjustmentFilter {
            offset: Some(1),
            limit: Some(1),
            ..Default::default()
        })
        .unwrap()[0]
            .id,
        adj[1].id
    );
    let exact = repo
        .list_adjustments(CostAdjustmentFilter {
            from_date: Some(adj[1].created_at),
            to_date: Some(adj[1].created_at),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(exact.len(), 1);
    assert_eq!(exact[0].id, adj[1].id);
    assert_eq!(
        repo.list_adjustments(CostAdjustmentFilter {
            adjustment_type: Some(CostAdjustmentType::Revaluation),
            offset: Some(1),
            ..Default::default()
        })
        .unwrap()
        .len(),
        1
    );
    assert!(
        repo.list_cost_layers(CostLayerFilter { offset: Some(99), ..Default::default() })
            .unwrap()
            .is_empty()
    );
    assert!(
        repo.list_adjustments(CostAdjustmentFilter {
            sku: Some("MISSING".into()),
            ..Default::default()
        })
        .unwrap()
        .is_empty()
    );
}
