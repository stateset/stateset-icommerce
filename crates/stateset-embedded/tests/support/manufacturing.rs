#![allow(dead_code)]
use rust_decimal::Decimal;
use stateset_embedded::*;
use uuid::Uuid;
pub(crate) fn input(commerce: &Commerce, quantity: Decimal) -> CreateWorkOrder {
    let tag = Uuid::new_v4().to_string();
    let product = commerce
        .products()
        .create(CreateProduct {
            name: tag.clone(),
            variants: Some(vec![CreateProductVariant {
                sku: tag.clone(),
                price: Decimal::ONE,
                ..Default::default()
            }]),
            ..Default::default()
        })
        .unwrap();
    let warehouse = commerce
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: tag.clone(),
            name: tag.clone(),
            ..Default::default()
        })
        .unwrap();
    let location = commerce
        .warehouse()
        .create_location(CreateLocation { warehouse_id: warehouse.id, ..Default::default() })
        .unwrap();
    CreateWorkOrder {
        product_id: product.id,
        quantity_to_build: quantity,
        stock: Some(WorkOrderStock {
            source_location_id: location.id,
            output_location_id: location.id,
            output_sku: tag,
        }),
        ..Default::default()
    }
}
pub(crate) fn seed(commerce: &Commerce, stock: &WorkOrderStock, sku: &str, quantity: Decimal) {
    commerce
        .warehouse()
        .adjust_inventory(AdjustLocationInventory {
            location_id: stock.source_location_id,
            sku: sku.into(),
            lot_id: None,
            quantity,
            reason: "fixture".into(),
            reference_type: None,
            reference_id: None,
            performed_by: None,
        })
        .unwrap();
}
