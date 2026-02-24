use criterion::{Criterion, criterion_group, criterion_main};
use stateset_benches::{create_temp_commerce, create_test_customers, create_test_orders};

fn bench_batch_orders(c: &mut Criterion) {
    let mut group = c.benchmark_group("sqlite_batch_insert_orders");

    for size in [100, 1_000] {
        // Pre-generate order inputs outside the benchmark loop.
        // Each iteration gets a fresh Commerce + temp database.
        group.bench_function(format!("batch_orders_{size}"), |bencher| {
            bencher.iter_with_setup(
                || {
                    let (commerce, dir) = create_temp_commerce();
                    let orders = create_test_orders(size);
                    // We must create a customer for each order first, because
                    // orders reference customer IDs. To keep it simple, we
                    // create one shared customer and rewrite the orders to
                    // point at it.
                    let customer = commerce
                        .customers()
                        .create(stateset_core::models::customer::CreateCustomer {
                            email: format!("batch-bench-{}@example.com", uuid::Uuid::new_v4()),
                            first_name: "Bench".into(),
                            last_name: "User".into(),
                            phone: None,
                            accepts_marketing: None,
                            tags: None,
                            metadata: None,
                        })
                        .expect("customer creation");

                    let orders: Vec<_> = orders
                        .into_iter()
                        .map(|mut o| {
                            o.customer_id = customer.id;
                            o
                        })
                        .collect();

                    (commerce, dir, orders)
                },
                |(commerce, _dir, orders)| {
                    for order in orders {
                        commerce.orders().create(order).expect("order insert");
                    }
                },
            );
        });
    }

    group.finish();
}

fn bench_batch_customers(c: &mut Criterion) {
    let mut group = c.benchmark_group("sqlite_batch_insert_customers");

    for size in [100, 1_000] {
        group.bench_function(format!("batch_customers_{size}"), |bencher| {
            bencher.iter_with_setup(
                || {
                    let (commerce, dir) = create_temp_commerce();
                    let customers = create_test_customers(size);
                    (commerce, dir, customers)
                },
                |(commerce, _dir, customers)| {
                    for customer in customers {
                        commerce.customers().create(customer).expect("customer insert");
                    }
                },
            );
        });
    }

    group.finish();
}

criterion_group!(benches, bench_batch_orders, bench_batch_customers);
criterion_main!(benches);
