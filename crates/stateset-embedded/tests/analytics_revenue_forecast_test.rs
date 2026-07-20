//! Regression for `revenue_forecast` money precision (SQLite).
//!
//! Money is stored as TEXT; the forecast previously read `AVG(SUM(total_amount))`
//! as `f64` and converted with `Decimal::from_f64_retain`, so exact decimals
//! drifted (0.10 + 0.20 -> 0.30000000000000004). It now averages with the exact
//! `decimal_sum` aggregate + Rust `Decimal` division, matching Postgres.

#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_embedded::{Commerce, CreateCustomer, CreateOrder, CreateOrderItem, TimeGranularity};
use uuid::Uuid;

#[test]
fn revenue_forecast_averages_money_exactly() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("fc-{}@example.com", Uuid::new_v4()),
            first_name: "Fore".into(),
            last_name: "Cast".into(),
            ..Default::default()
        })
        .expect("create customer");

    // Two orders in the current day totalling 0.10 + 0.20 = 0.30 — a sum that
    // f64 cannot represent exactly (0.30000000000000004).
    for price in [dec!(0.10), dec!(0.20)] {
        commerce
            .orders()
            .create(CreateOrder {
                customer_id: customer.id,
                items: vec![CreateOrderItem {
                    product_id: Uuid::new_v4().into(),
                    sku: "FC-SKU".into(),
                    name: "Forecast item".into(),
                    quantity: 1,
                    unit_price: price,
                    ..Default::default()
                }],
                ..Default::default()
            })
            .expect("create order");
    }

    // A single day of history → the forecast average is that day's revenue, 0.30.
    let forecast =
        commerce.analytics().revenue_forecast(1, TimeGranularity::Day).expect("revenue forecast");
    assert_eq!(forecast.len(), 1);
    assert_eq!(
        forecast[0].forecasted_revenue,
        dec!(0.30),
        "forecast revenue must be exact decimal, not an f64-drifted value"
    );
}
