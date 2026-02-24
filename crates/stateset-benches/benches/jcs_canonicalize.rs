use criterion::{Criterion, black_box, criterion_group, criterion_main};
use serde_json::json;
use stateset_crypto::canonicalize::canonicalize_json;

fn make_small_payload() -> serde_json::Value {
    json!({
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "name": "Widget",
        "price": 29.99,
        "currency": "USD",
        "active": true
    })
}

fn make_medium_payload() -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    for i in 0..20 {
        obj.insert(
            format!("field_{i:02}"),
            json!({
                "value": i,
                "label": format!("Label {i}"),
                "nested": { "depth": 1, "index": i }
            }),
        );
    }
    serde_json::Value::Object(obj)
}

fn make_large_payload() -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    for i in 0..50 {
        let mut inner = serde_json::Map::new();
        for j in 0..3 {
            inner.insert(
                format!("sub_{j}"),
                json!({
                    "id": format!("{i}-{j}"),
                    "tags": ["alpha", "beta", "gamma"],
                    "metadata": {
                        "created": "2026-01-01T00:00:00Z",
                        "version": j,
                        "flags": [true, false, true]
                    }
                }),
            );
        }
        obj.insert(format!("section_{i:03}"), serde_json::Value::Object(inner));
    }
    serde_json::Value::Object(obj)
}

fn bench_jcs_canonicalize(c: &mut Criterion) {
    let small = make_small_payload();
    let medium = make_medium_payload();
    let large = make_large_payload();

    let mut group = c.benchmark_group("jcs_canonicalize");

    group.bench_function("jcs_small", |bencher| {
        bencher.iter(|| canonicalize_json(black_box(&small)).unwrap());
    });

    group.bench_function("jcs_medium", |bencher| {
        bencher.iter(|| canonicalize_json(black_box(&medium)).unwrap());
    });

    group.bench_function("jcs_large", |bencher| {
        bencher.iter(|| canonicalize_json(black_box(&large)).unwrap());
    });

    group.finish();
}

criterion_group!(benches, bench_jcs_canonicalize);
criterion_main!(benches);
