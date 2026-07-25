# stateset-pricing

[![crates.io](https://img.shields.io/crates/v/stateset-pricing.svg)](https://crates.io/crates/stateset-pricing)
[![docs.rs](https://docs.rs/stateset-pricing/badge.svg)](https://docs.rs/stateset-pricing)

A deterministic, WASM-compatible pricing engine: line items, order totals,
promotions, currency conversion, tax, and monetary rounding — all pure functions.

Pricing is the one place in commerce where a rounding difference becomes a support
ticket. Every function here is side-effect free and deterministic, so the total your
storefront shows, the total your server charges, and the total your agent quotes are
computed by the same code and agree to the cent.

## Design Goals

- **Pure functions** — no database, no network, no I/O
- **Deterministic** — identical inputs always produce identical outputs
- **WASM-compatible** — no system dependencies, runs in the browser
- **Configurable rounding** — per-currency minor units

## Modules

| Module | Purpose |
|--------|---------|
| `line_item` | Per-line subtotal, discount, tax, and total |
| `order_total` | Aggregate order totals with shipping and fees |
| `promotions` | Rule-based promotion evaluation |
| `currency` | Exchange rates, conversion, triangulation |
| `tax` | Multi-jurisdiction, compound tax calculation |
| `rounding` | Configurable rounding policies per currency |
| `error` | Pricing error types |

## Usage

```rust
use stateset_pricing::{
    LineItem, LineDiscount, Fee, OrderTotalInput, RoundingPolicy,
    compute_order_total,
};
use rust_decimal_macros::dec;

let input = OrderTotalInput {
    items: vec![LineItem {
        sku: "WIDGET-001".into(),
        name: "Blue Widget".into(),
        unit_price: dec!(25.00),
        quantity: 4,
        discount: Some(LineDiscount::Percentage(dec!(0.10))),
        tax_rate: Some(dec!(0.08)),
    }],
    shipping_cost: dec!(5.99),
    shipping_tax_rate: Some(dec!(0.08)),
    order_discount: None,
    fees: vec![Fee { name: "Handling".into(), amount: dec!(2.00) }],
    rounding: RoundingPolicy::usd(),
};

let total = compute_order_total(&input);
assert!(total.grand_total > dec!(0));
```

## Rounding

`RoundingPolicy` carries the minor-unit count for a currency, which is why JPY (0
decimals) and USD (2) round correctly without special-casing at the call site. Use
`RoundingPolicy::usd()` and friends, or `minor_units_for_currency` to build one for
an arbitrary currency code.

## Part of StateSet iCommerce

Used by [`stateset-embedded`](https://crates.io/crates/stateset-embedded) for order
totals. Because it's pure and WASM-safe, it can also be compiled into a storefront to
quote prices client-side using the same logic the server bills with. Part of the
[StateSet iCommerce](https://github.com/stateset/stateset-icommerce) engine.

## License

MIT OR Apache-2.0
