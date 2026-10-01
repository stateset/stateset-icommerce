//! Build a disposable native store for the Rust-to-VES contract test.
use stateset_core::{
    AddShipmentEvent, ConditionOperator, ConditionType, CreateCustomer, CreateInventoryItem,
    CreateOrder, CreateOrderItem, CreatePromotion, CreatePromotionCondition, CreateShipment,
    CreateShipmentItem, CustomerRepository, InventoryRepository, OrderRepository, OrderStatus,
    ProductId, PromotionType, ShipOrder, ShipmentRepository, UpdateOrder, UpdateShipment,
};
use stateset_db::{DatabaseConfig, SqliteDatabase};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("expected a new fixture database path")?;
    // Refuse to populate an existing store, including a concurrently created one.
    std::fs::OpenOptions::new().write(true).create_new(true).open(&path)?;
    let db = SqliteDatabase::new(&DatabaseConfig::sqlite(&path))?;
    let customer = db.customers().create(CreateCustomer {
        email: "bridge@example.com".into(),
        first_name: "Ada".into(),
        last_name: "L".into(),
        ..Default::default()
    })?;
    db.inventory().create_item(CreateInventoryItem {
        sku: "BRIDGE".into(),
        name: "Bridge fixture".into(),
        initial_quantity: Some(rust_decimal::Decimal::from(5)),
        ..Default::default()
    })?;
    let order = db.orders().create(CreateOrder {
        customer_id: customer.id,
        items: vec![CreateOrderItem {
            product_id: ProductId::new(),
            sku: "BRIDGE".into(),
            name: "Fixture".into(),
            quantity: 2,
            unit_price: "19.99".parse()?,
            ..Default::default()
        }],
        ..Default::default()
    })?;
    let shipment = db.shipments().create(CreateShipment {
        order_id: order.id,
        recipient_name: "Ada L".into(),
        shipping_address: "1 Main".into(),
        ..Default::default()
    })?;
    let item = db.shipments().add_item(
        shipment.id,
        CreateShipmentItem {
            sku: "BRIDGE".into(),
            name: "Fixture".into(),
            quantity: 1,
            ..Default::default()
        },
    )?;
    db.shipments().remove_item(item.id)?;
    db.shipments().update(
        shipment.id,
        UpdateShipment {
            shipping_cost: Some("5.01".parse()?),
            notes: Some("contract fixture".into()),
            ..Default::default()
        },
    )?;
    db.shipments().add_event(
        shipment.id,
        AddShipmentEvent {
            event_type: "carrier_scan".into(),
            location: Some("Vancouver".into()),
            description: None,
            event_time: Some("2026-01-02T03:04:05.123456789Z".parse()?),
        },
    )?;
    for status in [OrderStatus::Confirmed, OrderStatus::Processing] {
        db.orders().update(order.id, UpdateOrder { status: Some(status), ..Default::default() })?;
    }
    db.orders()
        .ship(order.id, ShipOrder { tracking_number: Some("BRIDGE-TRACK".into()), lines: None })?;
    let promotion = db.promotions().create(CreatePromotion {
        name: "Bridge promotion".into(),
        promotion_type: PromotionType::PercentageOff,
        percentage_off: Some(rust_decimal::Decimal::TEN),
        ..Default::default()
    })?;
    db.promotions().add_condition(
        promotion.id,
        CreatePromotionCondition {
            condition_type: ConditionType::MinimumSubtotal,
            operator: ConditionOperator::GreaterThanOrEqual,
            value: "19.99".into(),
            is_required: true,
        },
    )?;
    println!("Native recorded outbox fixture: {path}");
    Ok(())
}
