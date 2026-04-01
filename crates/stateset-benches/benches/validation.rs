use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rust_decimal_macros::dec;
use stateset_benches::perf_gate::{run_gate_if_enabled, run_gate_if_enabled_with_iterations};
use stateset_core::ValidationBuilder;
use stateset_primitives::{CurrencyCode, Money};

/// Benchmark: `ValidationBuilder` with multiple field checks.
///
/// Exercises the full chain: required, email, max_length, positive, sku,
/// phone, postal_code, non_empty_list, uuid_not_nil.
fn bench_validation_builder(c: &mut Criterion) {
    let items = vec!["a", "b", "c"];
    let item_id = uuid::Uuid::new_v4();

    run_gate_if_enabled_with_iterations("validation_builder_full", 50_000, || {
        let _ = ValidationBuilder::new()
            .required("first_name", "Alice")
            .required("last_name", "Smith")
            .email("email", "alice@example.com")
            .max_length("name", "Alice Smith", 100)
            .min_length("name", "Alice Smith", 2)
            .positive("price", dec!(29.99))
            .non_negative("discount", dec!(0))
            .sku("sku", "WIDGET-BLUE-XL")
            .phone("phone", "+1-555-0100")
            .postal_code("postal", "94102")
            .non_empty_list("items", &items)
            .uuid_not_nil("item_id", item_id)
            .build();
    });

    c.bench_function("validation_builder_full", |bencher| {
        bencher.iter(|| {
            black_box(
                ValidationBuilder::new()
                    .required("first_name", black_box("Alice"))
                    .required("last_name", black_box("Smith"))
                    .email("email", black_box("alice@example.com"))
                    .max_length("name", black_box("Alice Smith"), 100)
                    .min_length("name", black_box("Alice Smith"), 2)
                    .positive("price", black_box(dec!(29.99)))
                    .non_negative("discount", black_box(dec!(0)))
                    .sku("sku", black_box("WIDGET-BLUE-XL"))
                    .phone("phone", black_box("+1-555-0100"))
                    .postal_code("postal", black_box("94102"))
                    .non_empty_list("items", black_box(&items))
                    .uuid_not_nil("item_id", black_box(item_id))
                    .build(),
            )
        });
    });
}

/// Benchmark: `ValidationBuilder` when validation fails early.
///
/// Tests the fast path where the first field is invalid.
fn bench_validation_builder_fail_fast(c: &mut Criterion) {
    run_gate_if_enabled_with_iterations("validation_builder_fail_fast", 100_000, || {
        let _ = ValidationBuilder::new()
            .required("name", "")
            .email("email", "alice@example.com")
            .positive("price", dec!(10.00))
            .build();
    });

    c.bench_function("validation_builder_fail_fast", |bencher| {
        bencher.iter(|| {
            black_box(
                ValidationBuilder::new()
                    .required("name", black_box(""))
                    .email("email", black_box("alice@example.com"))
                    .positive("price", black_box(dec!(10.00)))
                    .build(),
            )
        });
    });
}

/// Benchmark: `ValidationBuilder.build_all()` with multiple errors.
fn bench_validation_builder_build_all(c: &mut Criterion) {
    run_gate_if_enabled_with_iterations("validation_builder_build_all", 50_000, || {
        let _ = ValidationBuilder::new()
            .required("name", "")
            .email("email", "not-valid")
            .positive("price", dec!(-5))
            .sku("sku", "")
            .phone("phone", "abc")
            .build_all();
    });

    c.bench_function("validation_builder_build_all", |bencher| {
        bencher.iter(|| {
            black_box(
                ValidationBuilder::new()
                    .required("name", black_box(""))
                    .email("email", black_box("not-valid"))
                    .positive("price", black_box(dec!(-5)))
                    .sku("sku", black_box(""))
                    .phone("phone", black_box("abc"))
                    .build_all(),
            )
        });
    });
}

/// Benchmark: Money arithmetic — 1000 add/sub/mul operations.
fn bench_money_arithmetic_1000(c: &mut Criterion) {
    let base = Money::new(dec!(100.00), CurrencyCode::USD);
    let increment = Money::new(dec!(0.01), CurrencyCode::USD);

    run_gate_if_enabled_with_iterations("money_arith_1000", 100, || {
        let mut acc = base;
        for _ in 0..500 {
            acc = acc.checked_add(increment).unwrap();
        }
        for _ in 0..500 {
            acc = acc.checked_sub(increment).unwrap();
        }
    });

    c.bench_function("money_arith_1000", |bencher| {
        bencher.iter(|| {
            let mut acc = black_box(base);
            for _ in 0..500 {
                acc = acc.checked_add(black_box(increment)).unwrap();
            }
            for _ in 0..500 {
                acc = acc.checked_sub(black_box(increment)).unwrap();
            }
            black_box(acc)
        });
    });
}

/// Benchmark: Money multiply with different scalar values.
fn bench_money_multiply(c: &mut Criterion) {
    let prices = [
        Money::new(dec!(29.99), CurrencyCode::USD),
        Money::new(dec!(149.95), CurrencyCode::EUR),
        Money::new(dec!(9999.99), CurrencyCode::GBP),
    ];
    let quantities = [dec!(1), dec!(5), dec!(100), dec!(1000)];

    run_gate_if_enabled_with_iterations("money_multiply_batch", 50_000, || {
        for price in &prices {
            for &qty in &quantities {
                let _ = price.checked_mul_scalar(qty);
            }
        }
    });

    c.bench_function("money_multiply_batch", |bencher| {
        bencher.iter(|| {
            for price in &prices {
                for &qty in &quantities {
                    let _ = black_box(price).checked_mul_scalar(black_box(qty));
                }
            }
        });
    });
}

/// Benchmark: `CurrencyCode` parsing with valid and invalid codes.
fn bench_currency_code_parsing(c: &mut Criterion) {
    let valid = ["USD", "EUR", "GBP", "JPY", "CAD", "AUD", "CHF", "CNY"];
    let invalid = ["XYZ", "usd", "US", "USDD", "", "123"];
    let all_codes: Vec<&str> = valid.iter().chain(invalid.iter()).copied().collect();

    run_gate_if_enabled("currency_parse_mixed", || {
        for code in &all_codes {
            let _ = code.parse::<CurrencyCode>();
        }
    });

    c.bench_function("currency_parse_mixed", |bencher| {
        bencher.iter(|| {
            for code in &all_codes {
                let _ = black_box(*code).parse::<CurrencyCode>();
            }
        });
    });
}

/// Benchmark: Money round + comparison chain.
fn bench_money_round_compare(c: &mut Criterion) {
    let a = Money::new(dec!(123.456789), CurrencyCode::USD);
    let b = Money::new(dec!(123.46), CurrencyCode::USD);

    run_gate_if_enabled_with_iterations("money_round_compare", 100_000, || {
        let rounded = a.round_dp(2);
        let _ = rounded == b;
    });

    c.bench_function("money_round_compare", |bencher| {
        bencher.iter(|| {
            let rounded = black_box(a).round_dp(black_box(2));
            black_box(rounded == b)
        });
    });
}

criterion_group!(
    benches,
    bench_validation_builder,
    bench_validation_builder_fail_fast,
    bench_validation_builder_build_all,
    bench_money_arithmetic_1000,
    bench_money_multiply,
    bench_currency_code_parsing,
    bench_money_round_compare,
);
criterion_main!(benches);
