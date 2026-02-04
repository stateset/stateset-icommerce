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
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem, Currency,
    OrderFilter,
};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

proptest! {
    #[test]
    fn prop_inventory_never_goes_negative_reserve_concurrent(
        initial_quantity in 0u32..1000,
        reserve_quantities in proptest::collection::vec(1u32..1000, 0..50)
    ) {
        let total_reserve: u64 = reserve_quantities
            .iter()
            .map(|&quantity| quantity as u64)
            .sum();

        // Skip if total reservations would exceed initial (expected failure case)
        if total_reserve > initial_quantity as u64 {
            return Ok(());
        }

        let commerce = Arc::new(Commerce::new(":memory:").unwrap());

        commerce
            .inventory()
            .create_item(CreateInventoryItem {
                sku: "SKU-001".into(),
                name: "Widget".into(),
                initial_quantity: Some(Decimal::from(initial_quantity)),
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
                    .reserve("SKU-001", Decimal::from(quantity), "order", &order_id.to_string(), None)
            });
            handles.push(handle);
        }

        for handle in handles {
            let result = handle.join().unwrap();
            prop_assert!(result.is_ok(), "Reservation failed: {:?}", result);
        }

        let stock = commerce.inventory().get_stock("SKU-001").unwrap().unwrap();
        prop_assert!(
            stock.total_on_hand >= Decimal::ZERO,
            "Stock went negative: {}", stock.total_on_hand
        );

        let expected_allocated = Decimal::from(total_reserve);
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
        let usd_amount = Decimal::from(amount);

        let conversion = commerce
            .currency()
            .convert_amount(
                usd_amount,
                Currency::USD,
                Currency::EUR,
            );

        // If conversion succeeds, round-trip should work
        if conversion.is_ok() {
            let eur_amount = conversion.unwrap();
            let round_trip = commerce
                .currency()
                .convert_amount(
                    eur_amount,
                    Currency::EUR,
                    Currency::USD,
                );

            if round_trip.is_ok() {
                let back_to_usd = round_trip.unwrap();
                // Allow small rounding differences
                let diff = (back_to_usd - usd_amount).abs();
                let relative_tolerance = usd_amount.abs() * dec!(0.005);
                let tolerance = if relative_tolerance > dec!(0.01) {
                    relative_tolerance
                } else {
                    dec!(0.01)
                };
                prop_assert!(
                    diff <= tolerance,
                    "Currency conversion lost precision: {} -> {} -> {} (diff {}, tolerance {})",
                    usd_amount, eur_amount, back_to_usd, diff, tolerance
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
                quantity: qty as i32,
                unit_price: Decimal::from(price),
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
            let expected_total: Decimal = quantities
                .iter()
                .zip(prices.iter())
                .map(|(&qty, &price)| Decimal::from(qty) * Decimal::from(price))
                .sum();

            prop_assert!(
                order.total_amount == expected_total,
                "Order total mismatch: expected {}, got {}",
                expected_total, order.total_amount
            );
        }
    }
}

#[test]
fn test_concurrent_order_creation_preserves_consistency() {
    let commerce = Arc::new(Commerce::new(":memory:").unwrap());
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: "test@example.com".into(),
            first_name: "Test".into(),
            last_name: "Customer".into(),
            ..Default::default()
        })
        .unwrap();
    let customer_id = customer.id;

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
    let orders = commerce
        .orders()
        .list(OrderFilter {
            limit: Some(num_orders as u32),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(orders.len(), num_orders as usize);
}

#[test]
fn test_inventory_reserve_release_under_concurrent_modifications() {
    let commerce = Arc::new(Commerce::new(":memory:").unwrap());
    let reservations = Arc::new(Mutex::new(Vec::new()));

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
        let reservations = Arc::clone(&reservations);
        let handle = std::thread::spawn(move || {
            if i % 2 == 0 {
                // Reserve
                let order_id = Uuid::new_v4();
                if let Ok(reservation) = commerce
                    .inventory()
                    .reserve("SKU-001", dec!(2), "order", &order_id.to_string(), None)
                {
                    reservations.lock().unwrap().push(reservation.id);
                }
            } else {
                // Try to release a reservation (may fail if none exist)
                let reservation_id = reservations.lock().unwrap().pop();
                if let Some(reservation_id) = reservation_id {
                    commerce
                        .inventory()
                        .release_reservation(reservation_id)
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

    // Reservations should not change on-hand quantity
    assert_eq!(stock.total_on_hand, dec!(100));

    // Allocated should never exceed available
    assert!(stock.total_allocated <= stock.total_available);
}
