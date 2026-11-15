use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;

pub(crate) fn lifecycle(db: &dyn Database) {
    let sku = format!("VENDOR-STOCK-{}", Uuid::new_v4());
    let supplier = db
        .purchase_orders()
        .create_supplier(CreateSupplier { name: sku.clone(), ..Default::default() })
        .unwrap();
    let product = db
        .products()
        .create(CreateProduct {
            name: sku.clone(),
            variants: Some(vec![CreateProductVariant {
                sku: sku.clone(),
                price: dec!(10),
                is_default: Some(true),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .unwrap();
    let warehouse = db
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: sku.clone(),
            name: sku.clone(),
            ..Default::default()
        })
        .unwrap();
    let location = db
        .warehouse()
        .create_location(CreateLocation { warehouse_id: warehouse.id, ..Default::default() })
        .unwrap();
    db.warehouse()
        .adjust_inventory(AdjustLocationInventory {
            location_id: location.id,
            sku: sku.clone(),
            lot_id: None,
            quantity: dec!(10),
            reason: "receipt".into(),
            reference_type: None,
            reference_id: None,
            performed_by: None,
        })
        .unwrap();
    let line = |quantity, lot_id| CreateVendorReturnItem {
        product_id: product.id,
        sku: sku.clone(),
        source_location_id: location.id,
        lot_id,
        quantity,
        unit_cost: dec!(10),
        reason: VendorReturnReason::Defective,
    };
    let create = |items| {
        db.vendor_returns()
            .create(CreateVendorReturn {
                supplier_id: supplier.id,
                warehouse_id: warehouse.id,
                purchase_order_id: None,
                currency: Some(CurrencyCode::USD),
                items,
                notes: None,
            })
            .unwrap()
    };
    let stock = || db.inventory().get_stock(&sku).unwrap().unwrap();
    let credits = || {
        db.vendor_credits()
            .list(VendorCreditFilter { supplier_id: Some(supplier.id), ..Default::default() })
            .unwrap()
    };

    let returned = create(vec![line(dec!(3), None)]);
    assert!(db.vendor_returns().process(returned.id, true).is_err());
    db.vendor_returns().submit(returned.id).unwrap();
    assert!(db.vendor_returns().process(returned.id, false).is_err());
    assert_eq!(stock().total_on_hand, dec!(10));
    let done = db.vendor_returns().process(returned.id, true).unwrap();
    assert_eq!(done.status, VendorReturnStatus::Processed);
    assert!(done.credit_generated);
    assert_eq!(stock().total_on_hand, dec!(7));
    let linked = credits();
    assert_eq!(linked.len(), 1);
    assert_eq!(linked[0].vendor_return_id, Some(returned.id.into()));
    assert_eq!((linked[0].amount, linked[0].remaining), (dec!(30), dec!(30)));
    assert!(db.vendor_returns().process(returned.id, true).is_err());
    assert!(db.vendor_returns().cancel(returned.id).is_err());
    assert_eq!(stock().total_on_hand, dec!(7));
    assert_eq!(credits().len(), 1);
    for (supplier_id, amount, currency) in [
        (supplier.id, dec!(30), CurrencyCode::USD),
        (Uuid::new_v4(), dec!(30), CurrencyCode::USD),
        (supplier.id, dec!(31), CurrencyCode::USD),
        (supplier.id, dec!(30), CurrencyCode::EUR),
    ] {
        assert!(
            db.vendor_credits()
                .create(CreateVendorCredit {
                    supplier_id,
                    vendor_return_id: Some(returned.id.into()),
                    amount,
                    currency: Some(currency),
                    memo: None,
                })
                .is_err()
        );
    }
    // Failure after the first line must roll back that stock movement too.
    let short = create(vec![line(dec!(1), None), line(dec!(8), None)]);
    assert!(
        db.vendor_credits()
            .create(CreateVendorCredit {
                supplier_id: supplier.id,
                vendor_return_id: Some(short.id.into()),
                amount: dec!(90),
                currency: Some(CurrencyCode::USD),
                memo: None,
            })
            .is_err()
    );
    db.vendor_returns().submit(short.id).unwrap();
    assert!(db.vendor_returns().process(short.id, true).is_err());
    assert_eq!(
        db.vendor_returns().get(short.id).unwrap().unwrap().status,
        VendorReturnStatus::Pending
    );
    assert_eq!(stock().total_on_hand, dec!(7));
    assert_eq!(credits().len(), 1);

    // Defective lot stock can leave quarantine without releasing an order's hold.
    let lot = db
        .lots()
        .create(CreateLot { sku: sku.clone(), quantity: dec!(4), ..Default::default() })
        .unwrap();
    db.warehouse()
        .adjust_inventory(AdjustLocationInventory {
            location_id: location.id,
            sku: sku.clone(),
            lot_id: Some(lot.id),
            quantity: dec!(4),
            reason: "receipt".into(),
            reference_type: None,
            reference_id: None,
            performed_by: None,
        })
        .unwrap();
    db.lots().quarantine(lot.id, "defective").unwrap();
    let defect = create(vec![line(dec!(2), Some(lot.id))]);
    db.vendor_returns().submit(defect.id).unwrap();
    db.vendor_returns().process(defect.id, true).unwrap();
    assert_eq!((stock().total_on_hand, stock().total_allocated), (dec!(9), dec!(2)));
    let lot = db.lots().get(lot.id).unwrap().unwrap();
    assert_eq!((lot.quantity_remaining, lot.quantity_quarantined), (dec!(2), dec!(2)));
    assert_eq!(credits().len(), 2);
    db.lots().release_quarantine(lot.id).unwrap();
    let hold = db
        .lots()
        .reserve(ReserveLot {
            lot_id: lot.id,
            quantity: dec!(2),
            reference_type: "order".into(),
            reference_id: Uuid::new_v4(),
            expires_in_seconds: None,
        })
        .unwrap();
    let reserved = create(vec![line(dec!(1), Some(lot.id))]);
    db.vendor_returns().submit(reserved.id).unwrap();
    assert!(db.vendor_returns().process(reserved.id, true).is_err());
    db.lots().release_reservation(hold).unwrap();
    assert_eq!(stock().total_on_hand, dec!(9));
    assert_eq!(credits().len(), 2);
    let physical = db.warehouse().get_inventory_for_sku(warehouse.id, &sku).unwrap();
    let bins = db.bins().get_bin_levels_for_sku(warehouse.id, &sku).unwrap();
    assert_eq!(
        physical.iter().map(|row| row.quantity_on_hand).sum::<rust_decimal::Decimal>(),
        dec!(9)
    );
    assert_eq!(bins.iter().map(|row| row.quantity_on_hand).sum::<rust_decimal::Decimal>(), dec!(9));
}
