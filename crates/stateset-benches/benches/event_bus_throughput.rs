use chrono::Utc;
use criterion::{Criterion, criterion_group, criterion_main};
use rust_decimal_macros::dec;
use stateset_benches::perf_gate::run_gate_if_enabled_with_iterations;
use stateset_core::{CommerceEvent, CustomerId, OrderId};
use stateset_embedded::EventBus;

/// Create a sample event for benchmarking.
fn sample_event() -> CommerceEvent {
    CommerceEvent::OrderCreated {
        order_id: OrderId::new(),
        customer_id: CustomerId::new(),
        total_amount: dec!(99.99),
        item_count: 3,
        timestamp: Utc::now(),
    }
}

fn bench_publish_no_subscribers(c: &mut Criterion) {
    let bus = EventBus::new(4096);
    run_gate_if_enabled_with_iterations("publish_1000_no_sub", 25, || {
        for _ in 0..1_000 {
            bus.publish(sample_event());
        }
    });

    c.bench_function("publish_1000_no_sub", |bencher| {
        bencher.iter(|| {
            for _ in 0..1_000 {
                bus.publish(sample_event());
            }
        });
    });
}

fn bench_publish_one_subscriber(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    run_gate_if_enabled_with_iterations("publish_subscribe_1000", 10, || {
        let bus = EventBus::new(4096);
        let mut sub = bus.subscribe();
        for _ in 0..1_000 {
            bus.publish(sample_event());
        }
        rt.block_on(async {
            for _ in 0..1_000 {
                sub.try_recv();
            }
        });
    });

    c.bench_function("publish_subscribe_1000", |bencher| {
        bencher.iter(|| {
            let bus = EventBus::new(4096);
            let mut sub = bus.subscribe();

            // Publish 1000 events
            for _ in 0..1_000 {
                bus.publish(sample_event());
            }

            // Drain from the single subscriber synchronously
            rt.block_on(async {
                for _ in 0..1_000 {
                    sub.try_recv();
                }
            });
        });
    });
}

fn bench_publish_multi_subscriber(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    run_gate_if_enabled_with_iterations("publish_multi_sub_1000", 5, || {
        let bus = EventBus::new(4096);
        let mut subs: Vec<_> = (0..10).map(|_| bus.subscribe()).collect();
        for _ in 0..1_000 {
            bus.publish(sample_event());
        }
        rt.block_on(async {
            for sub in &mut subs {
                for _ in 0..1_000 {
                    sub.try_recv();
                }
            }
        });
    });

    c.bench_function("publish_multi_sub_1000", |bencher| {
        bencher.iter(|| {
            let bus = EventBus::new(4096);

            // Create 10 subscribers
            let mut subs: Vec<_> = (0..10).map(|_| bus.subscribe()).collect();

            // Publish 1000 events
            for _ in 0..1_000 {
                bus.publish(sample_event());
            }

            // Drain all subscribers
            rt.block_on(async {
                for sub in &mut subs {
                    for _ in 0..1_000 {
                        sub.try_recv();
                    }
                }
            });
        });
    });
}

criterion_group!(
    benches,
    bench_publish_no_subscribers,
    bench_publish_one_subscriber,
    bench_publish_multi_subscriber,
);
criterion_main!(benches);
