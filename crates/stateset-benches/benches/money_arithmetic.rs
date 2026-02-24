use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rust_decimal_macros::dec;
use stateset_primitives::{CurrencyCode, Money};

fn bench_money_add(c: &mut Criterion) {
    let a = Money::new(dec!(1234.56), CurrencyCode::USD);
    let b = Money::new(dec!(7890.12), CurrencyCode::USD);

    c.bench_function("money_add", |bencher| {
        bencher.iter(|| black_box(a).checked_add(black_box(b)));
    });
}

fn bench_money_sub(c: &mut Criterion) {
    let a = Money::new(dec!(9999.99), CurrencyCode::USD);
    let b = Money::new(dec!(1234.56), CurrencyCode::USD);

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
