#![cfg(feature = "sqlite")]

use chrono::{TimeZone, Utc};
use rust_decimal_macros::dec;
use stateset_core::{
    AnalyticsQuery, AnalyticsRepository, CreateCustomer, CreateOrder, CreateOrderItem,
    CustomerRepository, OrderRepository, TimeGranularity,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

#[test]
fn sqlite_revenue_by_period_quarter_groups_correctly() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");

    let customer = db
        .customers()
        .create(CreateCustomer {
            email: "analytics@example.com".into(),
            first_name: "Analytics".into(),
            last_name: "Test".into(),
            ..Default::default()
        })
        .expect("create customer");

    let order1 = db
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4(),
                sku: "SKU-Q1".into(),
                name: "Quarter 1".into(),
                quantity: 1,
                unit_price: dec!(10),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create order1");

    let order2 = db
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4(),
                sku: "SKU-Q2".into(),
                name: "Quarter 2".into(),
                quantity: 1,
                unit_price: dec!(20),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create order2");

    // Force created_at into different quarters so the analytics query can group correctly.
    {
        let conn = db.conn().expect("get sqlite connection");

        let q1 = Utc.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap();
        let q2 = Utc.with_ymd_and_hms(2026, 4, 15, 0, 0, 0).unwrap();

        conn.execute(
            "UPDATE orders SET created_at = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![q1.to_rfc3339(), q1.to_rfc3339(), order1.id.to_string()],
        )
        .expect("update order1 timestamps");
        conn.execute(
            "UPDATE orders SET created_at = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![q2.to_rfc3339(), q2.to_rfc3339(), order2.id.to_string()],
        )
        .expect("update order2 timestamps");
    }

    let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2026, 12, 31, 23, 59, 59).unwrap();

    let results = db
        .analytics()
        .get_revenue_by_period(
            AnalyticsQuery::new()
                .date_range(start, end)
                .granularity(TimeGranularity::Quarter),
        )
        .expect("revenue_by_period");

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].period, "2026-Q1");
    assert_eq!(results[0].order_count, 1);
    assert_eq!(results[0].revenue, dec!(10));
    assert_eq!(results[1].period, "2026-Q2");
    assert_eq!(results[1].order_count, 1);
    assert_eq!(results[1].revenue, dec!(20));
}
