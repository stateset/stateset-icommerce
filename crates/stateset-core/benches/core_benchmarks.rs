//! Benchmarks for stateset-core
//!
//! Run with: cargo bench --package stateset-core

use chrono::Utc;
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::thread;
use uuid::Uuid;

use stateset_core::models::{
    Address, Cart, CartItem, CartPaymentStatus, CartStatus, FulfillmentStatus,
    InventoryReservation, Order, OrderItem, OrderStatus, PaymentStatus, ReservationStatus,
};
use stateset_core::{BatchResult, CommerceError};
use stateset_primitives::{CartId, CurrencyCode, CustomerId, OrderId, OrderItemId, ProductId};

fn create_test_order_item(idx: usize) -> OrderItem {
    OrderItem {
        id: OrderItemId::from(Uuid::new_v4()),
        order_id: OrderId::from(Uuid::new_v4()),
        product_id: ProductId::from(Uuid::new_v4()),
        variant_id: None,
        sku: format!("SKU-{:04}", idx),
        name: format!("Product {}", idx),
        quantity: 2,
        shipped_quantity: 0,
        unit_price: dec!(29.99),
        discount: dec!(0.00),
        tax_amount: dec!(2.40),
        total: dec!(62.38),
    }
}

fn create_test_order(item_count: usize) -> Order {
    let items: Vec<OrderItem> = (0..item_count).map(create_test_order_item).collect();
    let total: Decimal = items.iter().map(|i| i.total).sum();

    Order {
        id: OrderId::from(Uuid::new_v4()),
        order_number: "ORD-2024-001".to_string(),
        customer_id: CustomerId::from(Uuid::new_v4()),
        status: OrderStatus::Pending,
        order_date: Utc::now(),
        total_amount: total,
        currency: CurrencyCode::USD,
        payment_status: PaymentStatus::Pending,
        fulfillment_status: FulfillmentStatus::Unfulfilled,
        payment_method: Some("credit_card".to_string()),
        shipping_method: Some("standard".to_string()),
        tracking_number: None,
        notes: None,
        shipping_address: Some(Address {
            line1: "123 Main St".to_string(),
            line2: None,
            city: "San Francisco".to_string(),
            state: Some("CA".to_string()),
            postal_code: "94102".to_string(),
            country: "US".to_string(),
        }),
        billing_address: None,
        items,
        version: 1,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn create_test_cart_item(idx: usize) -> CartItem {
    let now = Utc::now();
    CartItem {
        id: Uuid::new_v4(),
        cart_id: CartId::from(Uuid::new_v4()),
        product_id: Some(ProductId::from(Uuid::new_v4())),
        variant_id: None,
        sku: format!("SKU-{:04}", idx),
        name: format!("Product {}", idx),
        description: Some("A great product".to_string()),
        image_url: None,
        quantity: 2,
        unit_price: dec!(29.99),
        original_price: Some(dec!(39.99)),
        discount_amount: dec!(10.00),
        tax_amount: dec!(2.40),
        total: dec!(52.38),
        weight: Some(dec!(0.5)),
        requires_shipping: true,
        metadata: None,
        created_at: now,
        updated_at: now,
    }
}

fn create_test_cart(item_count: usize) -> Cart {
    let now = Utc::now();
    let items: Vec<CartItem> = (0..item_count).map(create_test_cart_item).collect();
    let subtotal: Decimal = items.iter().map(|i| i.unit_price * Decimal::from(i.quantity)).sum();
    let tax: Decimal = items.iter().map(|i| i.tax_amount).sum();
    let discount: Decimal = items.iter().map(|i| i.discount_amount).sum();

    Cart {
        id: CartId::from(Uuid::new_v4()),
        cart_number: "CART-2024-001".to_string(),
        customer_id: Some(CustomerId::from(Uuid::new_v4())),
        status: CartStatus::Active,
        currency: CurrencyCode::USD,
        items,
        subtotal,
        tax_amount: tax,
        shipping_amount: dec!(5.99),
        discount_amount: discount,
        grand_total: subtotal + tax + dec!(5.99) - discount,
        customer_email: Some("test@example.com".to_string()),
        customer_phone: None,
        customer_name: Some("Test User".to_string()),
        shipping_address: None,
        billing_address: None,
        billing_same_as_shipping: true,
        fulfillment_type: None,
        shipping_method: None,
        shipping_carrier: None,
        estimated_delivery: None,
        payment_method: None,
        payment_token: None,
        payment_status: CartPaymentStatus::None,
        coupon_code: None,
        discount_description: None,
        order_id: None,
        order_number: None,
        notes: None,
        metadata: None,
        inventory_reserved: false,
        reservation_expires_at: None,
        x402_payment: None,
        expires_at: None,
        completed_at: None,
        created_at: now,
        updated_at: now,
    }
}

fn benchmark_order_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_creation");

    for item_count in &[1, 5, 10, 50, 100] {
        group.bench_with_input(BenchmarkId::new("items", item_count), item_count, |b, &count| {
            b.iter(|| create_test_order(black_box(count)));
        });
    }

    group.finish();
}

fn benchmark_cart_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("cart_creation");

    for item_count in &[1, 5, 10, 50, 100] {
        group.bench_with_input(BenchmarkId::new("items", item_count), item_count, |b, &count| {
            b.iter(|| create_test_cart(black_box(count)));
        });
    }

    group.finish();
}

fn benchmark_order_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_serialization");

    for item_count in &[1, 10, 100] {
        let order = create_test_order(*item_count);

        group.bench_with_input(BenchmarkId::new("to_json", item_count), &order, |b, order| {
            b.iter(|| serde_json::to_string(black_box(order)).unwrap());
        });
    }

    for item_count in &[1, 10, 100] {
        let order = create_test_order(*item_count);
        let json = serde_json::to_string(&order).unwrap();

        group.bench_with_input(BenchmarkId::new("from_json", item_count), &json, |b, json| {
            b.iter(|| serde_json::from_str::<Order>(black_box(json)).unwrap());
        });
    }

    group.finish();
}

fn benchmark_order_lifecycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_lifecycle");

    let transitions = [
        (OrderStatus::Pending, OrderStatus::Confirmed),
        (OrderStatus::Confirmed, OrderStatus::Processing),
        (OrderStatus::Processing, OrderStatus::Shipped),
        (OrderStatus::Shipped, OrderStatus::Delivered),
    ];

    group.bench_function("status_transition_checks", |b| {
        b.iter(|| {
            for (from, to) in &transitions {
                black_box(from.can_transition_to(*to));
            }
        });
    });

    let order = create_test_order(3);
    group.bench_function("eligibility_checks", |b| {
        b.iter(|| {
            black_box(order.can_cancel());
            black_box(order.can_refund());
        });
    });

    group.finish();
}

fn benchmark_inventory_concurrent_reservations(c: &mut Criterion) {
    let mut group = c.benchmark_group("inventory_reservation_concurrent");
    let reservations_per_thread = 100;

    for thread_count in &[2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::new("threads", thread_count),
            thread_count,
            |b, &threads| {
                b.iter(|| {
                    let mut handles = Vec::with_capacity(threads);
                    for t in 0..threads {
                        handles.push(thread::spawn(move || {
                            let mut reservations = Vec::with_capacity(reservations_per_thread);
                            for i in 0..reservations_per_thread {
                                reservations.push(InventoryReservation {
                                    id: Uuid::new_v4(),
                                    item_id: i as i64,
                                    location_id: 1,
                                    quantity: dec!(1),
                                    status: ReservationStatus::Pending,
                                    reference_type: "benchmark".to_string(),
                                    reference_id: format!("ref-{}-{}", t, i),
                                    expires_at: None,
                                    created_at: Utc::now(),
                                });
                            }
                            black_box(reservations.len())
                        }));
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

fn benchmark_batch_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_operations");
    let error = CommerceError::ValidationError("invalid".to_string());

    for batch_size in &[10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::new("record_success", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let mut result = BatchResult::with_capacity(size);
                    for i in 0..size {
                        result.record_success(black_box(i));
                    }
                    black_box(result.success_count)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("record_failure", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let mut result: BatchResult<u64> = BatchResult::with_capacity(size);
                    for i in 0..size {
                        result.record_failure(i, None, &error);
                    }
                    black_box(result.failure_count)
                });
            },
        );
    }

    group.finish();
}

fn benchmark_decimal_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("decimal_operations");

    // Simulate order total calculation
    group.bench_function("calculate_order_total_10_items", |b| {
        let items: Vec<(Decimal, i32, Decimal, Decimal)> =
            (0..10).map(|_| (dec!(29.99), 2, dec!(0.00), dec!(2.40))).collect();

        b.iter(|| {
            let total: Decimal = items
                .iter()
                .map(|(price, qty, discount, tax)| (price * Decimal::from(*qty)) - discount + tax)
                .sum();
            black_box(total)
        });
    });

    // Simulate tax calculation
    group.bench_function("calculate_tax", |b| {
        let subtotal = dec!(999.99);
        let tax_rate = dec!(0.0875);

        b.iter(|| {
            let tax = black_box(subtotal) * black_box(tax_rate);
            black_box(tax.round_dp(2))
        });
    });

    // Simulate discount calculation
    group.bench_function("calculate_percentage_discount", |b| {
        let subtotal = dec!(999.99);
        let discount_percent = dec!(0.15);

        b.iter(|| {
            let discount = black_box(subtotal) * black_box(discount_percent);
            black_box(discount.round_dp(2))
        });
    });

    group.finish();
}

fn benchmark_uuid_generation(c: &mut Criterion) {
    c.bench_function("uuid_v4_generation", |b| {
        b.iter(|| black_box(Uuid::new_v4()));
    });
}

criterion_group!(
    benches,
    benchmark_order_creation,
    benchmark_cart_creation,
    benchmark_order_serialization,
    benchmark_order_lifecycle,
    benchmark_inventory_concurrent_reservations,
    benchmark_batch_operations,
    benchmark_decimal_operations,
    benchmark_uuid_generation,
);

criterion_main!(benches);
