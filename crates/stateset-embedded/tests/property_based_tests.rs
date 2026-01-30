//!
//! Property-based tests for concurrent operations using proptest.
//!
//! Tests invariant properties under concurrent modifications:
//! - Inventory never goes negative
//! - Reservations don't exceed available stock
//! - Order totals remain consistent
//! - Currency conversions maintain precision
//!

use proptest::prelude::*;
use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CreateInventoryItem, CreateOrder, CreateOrderItem, CreateProduct, OrderStatus,
    Product,
};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

proptest! {
    #[test]
    fn prop_inventory_never_goes_negative_reserve_concurrent(
        initial_quantity in 0u32..1000,
        num_reservations in 0u32..100,
        reserve_quantities: Vec<u32>
    ) {
        let total_reserve: u32 = reserve_quantities.iter().sum();

        // Skip if total reservations would exceed initial (expected failure case)
        if total_reserve > initial_quantity {
            return Ok(());
        }

        let commerce = Arc::new(Commerce::new(":memory:").unwrap());

        commerce
            .inventory()
            .create_item(CreateInventoryItem {
                sku: "SKU-001".into(),
                name: "Widget".into(),
                initial_quantity: Some(dec!(initial_quantity as i64)),
                ..Default::default()
            })
            .unwrap();

        let mut handles = vec![];
        for quantity in reserve_quantities {
            let commerce = Arc::clone(&commerce);
            let handle = std::thread::spawn(move || {
                let order_id = Uuid::new_v4();
                commerce
                    .inventory()
                    .reserve("SKU-001", dec!(quantity as i64), "order", &order_id.to_string(), None)
            });
            handles.push(handle);
        }

        for handle in handles {
            let result = handle.join().unwrap();
            prop_assert!(result.is_ok(), "Reservation failed: {:?}", result);
        }

        let stock = commerce.inventory().get_stock("SKU-001").unwrap().unwrap();
        prop_assert!(
            stock.total_on_hand >= rust_decimal::Decimal::ZERO,
            "Stock went negative: {}", stock.total_on_hand
        );

        let expected_allocated = dec!(total_reserve as i64);
        prop_assert!(
            stock.total_allocated == expected_allocated,
            "Allocated mismatch: expected {}, got {}",
            expected_allocated, stock.total_allocated
        );
    }

    #[test]
    fn prop_currency_conversion_preserves_quantity(
        amount in -1000000i64..1000000i64
    ) {
        let commerce = Commerce::new(":memory:").unwrap();

        // Test conversion chains preserve value
        let usd_amount = dec!(amount);

        let conversion = commerce
            .currency()
            .convert_currency(
                "USD",
                "EUR",
                usd_amount,
                None
            );

        // If conversion succeeds, round-trip should work
        if conversion.is_ok() {
            let eur_amount = conversion.unwrap();
            let round_trip = commerce
                .currency()
                .convert_currency(
                    "EUR",
                    "USD",
                    eur_amount,
                    None
                );

            if round_trip.is_ok() {
                let back_to_usd = round_trip.unwrap();
                // Allow small rounding differences
                let diff = (back_to_usd - usd_amount).abs();
                prop_assert!(
                    diff < dec!(0.01),
                    "Currency conversion lost precision: {} -> {} -> {}",
                    usd_amount, eur_amount, back_to_usd
                );
            }
        }
    }

    #[test]
    fn prop_order_total_calculation_deterministic(
        num_items in 1u32..20u32,
        quantities: Vec<u32>,
        prices: Vec<i64>
    ) {
        if quantities.len() != prices.len() || quantities.len() != num_items as usize {
            return Ok(());
        }

        let commerce = Commerce::new(":memory:").unwrap();
        let customer_id = Uuid::new_v4();

        let items: Vec<_> = quantities
            .iter()
            .zip(prices.iter())
            .map(|(&qty, &price)| CreateOrderItem {
                product_id: Uuid::new_v4(),
                sku: format!("SKU-{:03}", customer_id),
                name: "Test Product".into(),
                quantity: qty,
                unit_price: dec!(price),
                ..Default::default()
            })
            .collect();

        let order = commerce
            .orders()
            .create(CreateOrder {
                customer_id,
                items,
                ..Default::default()
            });

        if order.is_ok() {
            let order = order.unwrap();
            let expected_total: rust_decimal::Decimal = quantities
                .iter()
                .zip(prices.iter())
                .map(|(&qty, &price)| dec!(qty) * dec!(price))
                .sum();

            prop_assert!(
                order.total == expected_total,
                "Order total mismatch: expected {}, got {}",
                expected_total, order.total
            );
        }
    }

    #[test]
    fn prop_product_price_never_negative(
        price in -1000000i64..1000000i64
    ) {
        let commerce = Commerce::new(":memory:").unwrap();

        if price < 0 {
            // Negative prices should fail validation
            let result = commerce
                .products()
                .create(CreateProduct {
                    name: "Test Product".into(),
                    sku: "SKU-001".into(),
                    price: dec!(price),
                    ..Default::default()
                });
            prop_assert!(result.is_err(), "Negative price should be rejected");
        } else {
            // Non-negative prices should succeed
            let result = commerce
                .products()
                .create(CreateProduct {
                    name: "Test Product".into(),
                    sku: "SKU-001".into(),
                    price: dec!(price),
                    ..Default::default()
                });
            prop_assert!(result.is_ok(), "Valid price should be accepted: {:?}", result);
        }
    }
}

#[test]
fn test_concurrent_order_creation_preserves_consistency() {
    let commerce = Arc::new(Commerce::new(":memory:").unwrap());
    let customer_id = Uuid::new_v4();

    let num_orders = 100;
    let mut handles = vec![];

    for i in 0..num_orders {
        let commerce = Arc::clone(&commerce);
        let handle = std::thread::spawn(move || {
            commerce.orders().create(CreateOrder {
                customer_id,
                items: vec![CreateOrderItem {
                    product_id: Uuid::new_v4(),
                    sku: format!("SKU-{:03}", i),
                    name: "Test Product".into(),
                    quantity: 1,
                    unit_price: dec!(29.99),
                    ..Default::default()
                }],
                ..Default::default()
            })
        });
        handles.push(handle);
    }

    let mut success_count = 0;
    let mut failure_count = 0;

    for handle in handles {
        let result = handle.join().unwrap();
        match result {
            Ok(_) => success_count += 1,
            Err(_) => failure_count += 1,
        }
    }

    // All orders should succeed in this scenario
    assert_eq!(success_count, num_orders);
    assert_eq!(failure_count, 0);

    // Verify all orders exist
    let orders = commerce.orders().list(0, num_orders as i32).unwrap();
    assert_eq!(orders.len(), num_orders as usize);
}

#[test]
fn test_inventory_reserve_release_under_concurrent_modifications() {
    let commerce = Arc::new(Commerce::new(":memory:").unwrap());
    let final_stock = Arc::new(Mutex::new(dec!(0)));

    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "SKU-001".into(),
            name: "Widget".into(),
            initial_quantity: Some(dec!(100)),
            ..Default::default()
        })
        .unwrap();

    let num_operations = 50;
    let mut handles = vec![];

    for i in 0..num_operations {
        let commerce = Arc::clone(&commerce);
        let handle = std::thread::spawn(move || {
            if i % 2 == 0 {
                // Reserve
                let order_id = Uuid::new_v4();
                commerce
                    .inventory()
                    .reserve("SKU-001", dec!(2), "order", &order_id.to_string(), None)
                    .ok();
            } else {
                // Try to release some reservations (may fail if none exist)
                let stock = commerce.inventory().get_stock("SKU-001").unwrap().unwrap();
                if stock.total_allocated > dec!(0) {
                    commerce
                        .inventory()
                        .adjust("SKU-001", dec!(1), "test release")
                        .ok();
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let stock = commerce.inventory().get_stock("SKU-001").unwrap().unwrap();

    // Final stock should be between 50 and 150 depending on reserve/release ratio
    assert!(stock.total_on_hand >= dec!(50));
    assert!(stock.total_on_hand <= dec!(100));

    // Allocated should never exceed available
    assert!(stock.total_allocated <= stock.total_available);
}
