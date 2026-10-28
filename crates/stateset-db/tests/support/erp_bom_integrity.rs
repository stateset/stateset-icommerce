use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;

pub(crate) fn revisions_and_cycles(db: &dyn Database) {
    let product = ProductId::new();
    let child = ProductId::new();
    let component = |product, quantity| CreateBomComponent {
        component_product_id: Some(product),
        name: "Material".into(),
        quantity,
        ..Default::default()
    };
    let input = |revision: &str, components| CreateBom {
        product_id: product,
        name: format!("BOM-{}", Uuid::new_v4()),
        revision: Some(revision.into()),
        components: Some(components),
        ..Default::default()
    };
    let before =
        db.bom().count(BomFilter { product_id: Some(product), ..Default::default() }).unwrap();
    assert!(
        db.bom()
            .create(input("INVALID", vec![component(child, dec!(1)), component(child, dec!(0))]))
            .is_err()
    );
    assert!(db.bom().create(input("SELF", vec![component(product, dec!(1))])).is_err());
    assert_eq!(
        db.bom().count(BomFilter { product_id: Some(product), ..Default::default() }).unwrap(),
        before
    );
    let bom = db.bom().create(input("A", vec![component(child, dec!(1.25))])).unwrap();
    assert!(db.bom().create(input("A", vec![])).is_err());
    assert!(db.bom().add_component(bom.id, component(child, dec!(-1))).is_err());
    assert!(db.bom().update_component(bom.components[0].id, component(child, dec!(0))).is_err());
    assert_eq!(db.bom().get(bom.id).unwrap().unwrap().components[0].quantity, dec!(1.25));
    // A cycle through a different draft assembly is rejected in the same creation transaction.
    assert!(
        db.bom()
            .create(CreateBom {
                product_id: child,
                name: "Cyclic child".into(),
                components: Some(vec![component(product, dec!(1))]),
                ..Default::default()
            })
            .is_err()
    );
    db.bom().activate(bom.id).unwrap();
    assert!(db.bom().add_component(bom.id, component(child, dec!(1))).is_err());
    assert!(db.bom().update_component(bom.components[0].id, component(child, dec!(2))).is_err());
    assert!(db.bom().remove_component(bom.components[0].id).is_err());
    assert!(
        db.bom()
            .update(bom.id, UpdateBom { revision: Some("B".into()), ..Default::default() })
            .is_err()
    );
    assert!(
        db.bom()
            .update(bom.id, UpdateBom { status: Some(BomStatus::Draft), ..Default::default() })
            .is_err()
    );
    let revised = db.bom().create(input("B", vec![component(child, dec!(2.5))])).unwrap();
    db.bom().activate(revised.id).unwrap();
    let old = db.bom().get(bom.id).unwrap().unwrap();
    assert_eq!(old.status, BomStatus::Obsolete);
    assert_eq!(old.components[0].quantity, dec!(1.25));
    assert_eq!(db.bom().get(revised.id).unwrap().unwrap().components[0].quantity, dec!(2.5));
    let before =
        db.bom().count(BomFilter { product_id: Some(product), ..Default::default() }).unwrap();
    assert!(
        db.bom()
            .create_batch_atomic(vec![
                input("C", vec![component(child, dec!(1))]),
                input("D", vec![component(child, dec!(0))])
            ])
            .is_err()
    );
    assert_eq!(
        db.bom().count(BomFilter { product_id: Some(product), ..Default::default() }).unwrap(),
        before
    );
    let draft = db.bom().create(input("C", vec![])).unwrap();
    assert!(
        db.bom()
            .update_batch_atomic(vec![
                (draft.id, UpdateBom { name: Some("Must roll back".into()), ..Default::default() }),
                (revised.id, UpdateBom { revision: Some("X".into()), ..Default::default() })
            ])
            .is_err()
    );
    assert_ne!(db.bom().get(draft.id).unwrap().unwrap().name, "Must roll back");
}
