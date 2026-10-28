use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;

pub(crate) fn valuation(db: &dyn Database, second_balance: impl Fn(i64)) {
    let sku = format!("ERP-{}", uuid::Uuid::new_v4());
    let costs = db.cost_accounting();
    let before = costs.get_inventory_valuation(CostMethod::Standard).unwrap();
    let health_before = db.analytics().get_inventory_health().unwrap();
    let _ = product(db, &sku);
    let item = db
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: sku.clone(),
            initial_quantity: Some(dec!(0.1)),
            reorder_point: Some(dec!(10)),
            ..Default::default()
        })
        .unwrap();
    second_balance(item.id);
    costs
        .set_item_cost(SetItemCost {
            sku: sku.clone(),
            standard_cost: Some(dec!(3.25)),
            cost_method: Some(CostMethod::Standard),
            ..Default::default()
        })
        .unwrap();
    let after = costs.get_inventory_valuation(CostMethod::Standard).unwrap();
    assert_eq!(after.total_quantity - before.total_quantity, dec!(0.3));
    assert_eq!(after.total_value - before.total_value, dec!(0.975));
    let summary = costs.get_sku_cost_summary(&sku).unwrap().unwrap();
    assert_eq!(summary.quantity_on_hand, dec!(0.3));
    assert_eq!(summary.total_value, dec!(0.975));
    let low = db.analytics().get_low_stock_items(Some(dec!(1))).unwrap();
    let matching: Vec<_> = low.iter().filter(|row| row.sku == sku).collect();
    assert_eq!(matching.len(), 1, "low stock must aggregate locations");
    assert_eq!(matching[0].on_hand, dec!(0.3));
    assert_eq!(
        matching[0].reorder_point,
        Some(dec!(10)),
        "reorder points must compare numerically"
    );
    let health = db.analytics().get_inventory_health().unwrap();
    assert_eq!(health.total_skus - health_before.total_skus, 1);
    assert_eq!(health.low_stock_skus - health_before.low_stock_skus, 1);
    assert_eq!(health.total_value - health_before.total_value, dec!(0.975));
}

fn product(db: &dyn Database, sku: &str) -> ProductId {
    db.products()
        .create(CreateProduct {
            name: sku.into(),
            slug: None,
            description: None,
            product_type: None,
            attributes: None,
            seo: None,
            variants: Some(vec![CreateProductVariant {
                sku: sku.into(),
                price: dec!(1),
                cost: Some(dec!(3.25)),
                ..Default::default()
            }]),
        })
        .unwrap()
        .id
}

fn component(sku: &str, quantity: rust_decimal::Decimal) -> CreateBomComponent {
    CreateBomComponent {
        name: sku.into(),
        component_sku: Some(sku.into()),
        quantity,
        ..Default::default()
    }
}

pub(crate) fn rollups(db: &dyn Database) {
    let tag = uuid::Uuid::new_v4();
    let raw = format!("RAW-{tag}");
    let sub = format!("SUB-{tag}");
    let finished = format!("FIN-{tag}");
    let sub_product = product(db, &sub);
    let finished_product = product(db, &finished);
    let costs = db.cost_accounting();
    costs
        .set_item_cost(SetItemCost {
            sku: raw.clone(),
            standard_cost: Some(dec!(0.125)),
            cost_method: Some(CostMethod::Standard),
            ..Default::default()
        })
        .unwrap();
    costs
        .set_item_cost(SetItemCost {
            sku: sub.clone(),
            standard_cost: Some(dec!(999)),
            labor_cost: Some(dec!(0.25)),
            ..Default::default()
        })
        .unwrap();
    costs
        .set_item_cost(SetItemCost {
            sku: finished.clone(),
            standard_cost: Some(dec!(20)),
            labor_cost: Some(dec!(1)),
            overhead_cost: Some(dec!(0.5)),
            ..Default::default()
        })
        .unwrap();
    let child = db
        .bom()
        .create(CreateBom {
            product_id: sub_product,
            name: sub.clone(),
            components: Some(vec![component(&raw, dec!(2))]),
            ..Default::default()
        })
        .unwrap();
    db.bom()
        .update(child.id, UpdateBom { status: Some(BomStatus::Active), ..Default::default() })
        .unwrap();
    let parent = db
        .bom()
        .create(CreateBom {
            product_id: finished_product,
            name: finished.clone(),
            components: Some(vec![component(&sub, dec!(3))]),
            ..Default::default()
        })
        .unwrap();
    for expected_previous in [dec!(20), dec!(3)] {
        let rollup = costs.calculate_rollup(&finished, Some(parent.id)).unwrap();
        assert_eq!(rollup.material_cost, dec!(1.5), "3 * (2 * .125 + .25)");
        assert_eq!(rollup.labor_cost, dec!(1));
        assert_eq!(rollup.overhead_cost, dec!(0.5));
        assert_eq!(rollup.total_cost, dec!(3));
        assert_eq!(rollup.previous_cost, expected_previous);
        assert_eq!(costs.get_item_cost(&finished).unwrap().unwrap().standard_cost, dec!(3));
    }
    let saved_rollup = costs.get_rollup(&finished).unwrap().unwrap();
    let missing =
        db.bom().add_component(parent.id, component("MISSING-ERP-COST", dec!(1))).unwrap();
    assert!(matches!(
        costs.calculate_rollup(&finished, Some(parent.id)),
        Err(CommerceError::ValidationError(_))
    ));
    assert_eq!(costs.get_rollup(&finished).unwrap().unwrap().id, saved_rollup.id);
    assert_eq!(costs.get_item_cost(&finished).unwrap().unwrap().standard_cost, dec!(3));
    db.bom().remove_component(missing.id).unwrap();
    db.bom()
        .update(parent.id, UpdateBom { status: Some(BomStatus::Active), ..Default::default() })
        .unwrap();
    assert!(
        db.bom().add_component(child.id, component(&finished, dec!(1))).is_err(),
        "Published assemblies cannot be edited into a cycle"
    );
    assert_eq!(costs.get_item_cost(&finished).unwrap().unwrap().standard_cost, dec!(3));
    assert!(costs.calculate_rollup(&finished, Some(uuid::Uuid::new_v4())).is_err());
}
