use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rust_decimal_macros::dec;
use stateset_benches::perf_gate::{run_gate_if_enabled, run_gate_if_enabled_with_iterations};
use stateset_primitives::{CurrencyCode, Money};

fn bench_money_add(c: &mut Criterion) {
    let a = Money::new(dec!(1234.56), CurrencyCode::USD);
    let b = Money::new(dec!(7890.12), CurrencyCode::USD);
    run_gate_if_enabled_with_iterations("money_add", 200_000, || {
        let _ = a.checked_add(b);
    });

    c.bench_function("money_add", |bencher| {
        bencher.iter(|| black_box(a).checked_add(black_box(b)));
    });
}

fn bench_money_sub(c: &mut Criterion) {
    let a = Money::new(dec!(9999.99), CurrencyCode::USD);
    let b = Money::new(dec!(1234.56), CurrencyCode::USD);
    run_gate_if_enabled_with_iterations("money_sub", 200_000, || {
        let _ = a.checked_sub(b);
    });

    c.bench_function("money_sub", |bencher| {
        bencher.iter(|| black_box(a).checked_sub(black_box(b)));
    });
}

fn bench_money_round(c: &mut Criterion) {
    let values = [
        Money::new(dec!(3.14159265), CurrencyCode::USD),
        Money::new(dec!(2.71828182), CurrencyCode::EUR),
        Money::new(dec!(1.41421356), CurrencyCode::GBP),
    ];

    let mut group = c.benchmark_group("money_round");
    for dp in [2u32, 4, 6] {
        let gate_name = format!("money_round_dp_{dp}");
        run_gate_if_enabled_with_iterations(gate_name.as_str(), 50_000, || {
            for v in &values {
                let _ = v.round_dp(dp);
            }
        });

        group.bench_function(format!("dp_{dp}"), |bencher| {
            bencher.iter(|| {
                for v in &values {
                    let _ = black_box(v.round_dp(black_box(dp)));
                }
            });
        });
    }
    group.finish();
}

fn bench_currency_code_parse(c: &mut Criterion) {
    let codes = ["USD", "EUR", "GBP", "JPY", "CAD", "AUD", "CHF", "CNY"];
    run_gate_if_enabled("currency_code_parse", || {
        for code in &codes {
            let _ = code.parse::<CurrencyCode>();
        }
    });

    c.bench_function("currency_code_parse", |bencher| {
        bencher.iter(|| {
            for code in &codes {
                let _ = black_box(*code).parse::<CurrencyCode>();
            }
        });
    });
}

criterion_group!(
    benches,
    bench_money_add,
    bench_money_sub,
    bench_money_round,
    bench_currency_code_parse,
);
criterion_main!(benches);
