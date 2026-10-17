#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem, OrderStatus, ProductId,
    ReservationStatus, UpdateOrder,
};
use stateset_db::PostgresDatabase;
use uuid::Uuid;

#[tokio::test]
async fn physical_posting_serializes_concurrent_retries_and_preserves_unshipped_stock() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        return;
    };
    let db = PostgresDatabase::connect(&url).await.unwrap();
    let unique = Uuid::new_v4();
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("posting-{unique}@example.test"),
            first_name: "Stock".into(),
            last_name: "Posting".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let sku = format!("POST-{unique}");
    db.inventory()
        .create_item_async(CreateInventoryItem {
            sku: sku.clone(),
            name: "Posting".into(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .await
        .unwrap();
    let order = db
        .orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: sku.clone(),
                name: "Posting".into(),
                quantity: 2,
                unit_price: dec!(1.25),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(db.inventory().fulfill_order_stock_async(order.id.into_uuid()).await.is_err());
    for status in [OrderStatus::Confirmed, OrderStatus::Processing, OrderStatus::Shipped] {
        db.orders()
            .update_async(
                order.id.into_uuid(),
                UpdateOrder { status: Some(status), ..Default::default() },
            )
            .await
            .unwrap();
    }
    let inventory = db.inventory();
    let (first, second) = tokio::join!(
        inventory.fulfill_order_stock_async(order.id.into_uuid()),
        inventory.fulfill_order_stock_async(order.id.into_uuid())
    );
    first.unwrap();
    second.unwrap();
    let stock = db.inventory().get_stock_async(&sku).await.unwrap().unwrap();
    assert_eq!(
        (stock.total_on_hand, stock.total_allocated, stock.total_available),
        (dec!(8), dec!(0), dec!(8))
    );
}

#[tokio::test]
async fn stock_posting_refuses_suppressed_or_rewritten_inventory_writes() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        return;
    };
    let db = PostgresDatabase::connect(&url).await.unwrap();
    for case in 0..6 {
        let unique = Uuid::new_v4();
        let customer = db
            .customers()
            .create_async(CreateCustomer {
                email: format!("posting-failure-{unique}@example.test"),
                first_name: "Stock".into(),
                last_name: "Posting".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        let sku = format!("POST-FAIL-{unique}");
        db.inventory()
            .create_item_async(CreateInventoryItem {
                sku: sku.clone(),
                name: "Posting".into(),
                initial_quantity: Some(dec!(10)),
                ..Default::default()
            })
            .await
            .unwrap();
        let order = db
            .orders()
            .create_async(CreateOrder {
                customer_id: customer.id,
                items: vec![CreateOrderItem {
                    product_id: ProductId::new(),
                    sku: sku.clone(),
                    name: "Posting".into(),
                    quantity: 2,
                    unit_price: dec!(1.25),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .await
            .unwrap();
        for status in [OrderStatus::Confirmed, OrderStatus::Processing, OrderStatus::Shipped] {
            db.orders()
                .update_async(
                    order.id.into_uuid(),
                    UpdateOrder { status: Some(status), ..Default::default() },
                )
                .await
                .unwrap();
        }
        let hold_id: Uuid = sqlx::query_scalar("SELECT id FROM inventory_reservations WHERE reference_type = 'order' AND reference_id = $1")
            .bind(order.id.to_string()).fetch_one(db.pool()).await.unwrap();
        let event_count = || {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM kernel_outbox WHERE aggregate_id = $1",
            )
            .bind(hold_id.to_string())
            .fetch_one(db.pool())
        };
        let facts_before = event_count().await.unwrap();
        let name = format!("posting_{}", unique.simple());
        // Select only this locally generated fixture SKU. Keep database-derived
        // identifiers in bound queries rather than interpolating them into DDL.
        let item_selector = format!("(SELECT id FROM inventory_items WHERE sku = '{sku}')");
        let (table, operation, timing, body) = match case {
            0 => (
                "inventory_reservations",
                "UPDATE",
                "BEFORE",
                format!(
                    "IF NEW.item_id = {item_selector} AND NEW.status = 'fulfilled' THEN RETURN NULL; END IF;"
                ),
            ),
            1 => (
                "inventory_transactions",
                "INSERT",
                "BEFORE",
                format!(
                    "IF NEW.item_id = {item_selector} AND NEW.transaction_type = 'shipment' THEN RETURN NULL; END IF;"
                ),
            ),
            2 => (
                "inventory_reservations",
                "UPDATE",
                "BEFORE",
                format!(
                    "IF NEW.item_id = {item_selector} AND NEW.status = 'fulfilled' THEN NEW.status := OLD.status; END IF;"
                ),
            ),
            3 => (
                "inventory_balances",
                "UPDATE",
                "BEFORE",
                format!(
                    "IF NEW.item_id = {item_selector} THEN NEW.quantity_on_hand := 99; END IF;"
                ),
            ),
            4 => (
                "inventory_transactions",
                "INSERT",
                "BEFORE",
                format!(
                    "IF NEW.item_id = {item_selector} AND NEW.transaction_type = 'shipment' THEN NEW.quantity := 0; END IF;"
                ),
            ),
            _ => (
                "kernel_outbox",
                "INSERT",
                "AFTER",
                format!(
                    "IF NEW.aggregate_id IN (SELECT id::text FROM inventory_reservations WHERE item_id = {item_selector}) AND NEW.event_type = 'inventory.reservation_fulfilled.v1' THEN UPDATE inventory_balances SET quantity_on_hand = 99 WHERE item_id = {item_selector}; END IF;"
                ),
            ),
        };
        sqlx::raw_sql(&format!("CREATE FUNCTION {name}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN {body} RETURN NEW; END; $$; CREATE TRIGGER {name} {timing} {operation} ON {table} FOR EACH ROW EXECUTE FUNCTION {name}();"))
            .execute(db.pool()).await.unwrap();
        let result = db.inventory().fulfill_order_stock_async(order.id.into_uuid()).await;
        sqlx::raw_sql(&format!("DROP TRIGGER {name} ON {table}; DROP FUNCTION {name}();"))
            .execute(db.pool())
            .await
            .unwrap();
        assert!(result.is_err(), "must refuse silently changed stock posting: case {case}");
        let stock = db.inventory().get_stock_async(&sku).await.unwrap().unwrap();
        assert_eq!(
            (stock.total_on_hand, stock.total_allocated, stock.total_available),
            (dec!(10), dec!(2), dec!(8)),
            "case {case}"
        );
        let (status, quantity): (String, rust_decimal::Decimal) =
            sqlx::query_as("SELECT status, quantity FROM inventory_reservations WHERE id = $1")
                .bind(hold_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(status, ReservationStatus::Confirmed.to_string());
        assert_eq!(quantity, dec!(2));
        let shipments: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_transactions WHERE reference_type = 'order' AND reference_id = $1 AND transaction_type = 'shipment'").bind(order.id.to_string()).fetch_one(db.pool()).await.unwrap();
        assert_eq!(shipments, 0, "case {case}");
        assert_eq!(event_count().await.unwrap(), facts_before, "case {case}");
        for _ in 0..2 {
            db.inventory().fulfill_order_stock_async(order.id.into_uuid()).await.unwrap();
        }
        assert_eq!(
            db.inventory().get_stock_async(&sku).await.unwrap().unwrap().total_on_hand,
            dec!(8)
        );
    }
}
