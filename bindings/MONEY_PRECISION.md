# Money precision in the language bindings — decision needed

**Status: open decision.** This documents a known correctness issue on the
Node and Python binding surfaces and lays out migration options. It is a
breaking change, so it needs an explicit owner decision rather than a silent
edit.

## The problem

The Rust core represents every monetary amount as `rust_decimal::Decimal` —
exact base-10, no rounding error. The engine, the HTTP DTOs (fixed in
v1.7.0), and the newest binding domains (`gift_cards`, `loyalty`, added this
cycle) all preserve that: money crosses the FFI boundary as an **exact decimal
string** (e.g. `"30.01"`).

The **older binding output structs do not.** They convert `Decimal` → `f64`:

- **Node** (`bindings/node/src/lib.rs`): ~50 money fields typed `f64` —
  `PaymentOutput.amount`, `OrderResponse.total_amount`, `InvoiceOutput`
  subtotal/tax/paid, cart and line-item amounts, refund amounts, etc.
- **Python** (`bindings/python/src/lib.rs`): the same ~50 fields converted via
  `to_f64_result(...)` on payment/order/invoice/cart/refund amounts.

`f64` cannot represent most decimal cents exactly (`0.1 + 0.2 != 0.3`). For a
commerce engine whose whole value proposition includes exact settlement, money
that round-trips through IEEE-754 on the primary consumer surface is a real
defect: sub-cent drift, and values beyond 2⁵³ minor units lose integer
precision outright.

## Why it isn't already fixed

Flipping a public field's type (`amount: number` → `amount: string` in TS;
`float` → `str` in Python) **breaks every consumer** that does arithmetic or
comparison on it. That is a semver-major change and a migration burden for
downstream code, so it needs a deliberate call on *how* and *when*, not a
drive-by edit. New domains already use strings, so the direction is set — the
question is only how to migrate the existing surface.

## Options

### A. Big-bang break at the next major (recommended end state)
Flip all money fields to exact strings in binding `2.0`. Ship a migration
guide (`money_field.toString()` was already a string; consumers wrap reads in
`Decimal`/`parseFloat` as appropriate). Clean final state, one break, honest
types.
- **Pro:** correct everywhere; no lingering wrong fields; matches the new
  domains and the HTTP layer.
- **Con:** a hard break for all binding consumers at once.

### B. Dual fields as a bridge
Add `*_decimal` string fields next to the existing `f64` ones (e.g.
`amount` + `amount_decimal`), mark `f64` deprecated in docs/stubs, remove the
`f64` variant in the next major.
- **Pro:** non-breaking; consumers migrate at their pace.
- **Con:** doubles the money surface; the wrong field stays callable; two
  sources of truth until the major.

### C. Gradual per-domain migration (in-flight already)
New domains use strings (done: `gift_cards`, `loyalty`). Migrate existing
domains to strings one at a time across minors **with the dual-field bridge
(B)** so each step is non-breaking, then drop `f64` at `2.0` (A).
- **Pro:** steady progress, no flag day; each domain independently shippable.
- **Con:** longest tail; mixed representation until complete.

## Recommendation

**C → A:** keep new code on strings (already the norm), add string fields
alongside the existing `f64` on the high-value money domains first (payments,
orders, invoices) with the `f64` variants documented as deprecated, and
complete the flip to string-only at binding `2.0`. This gives consumers a
non-breaking migration path today and a correct, un-doubled surface at the
next major.

Whichever path is chosen, the **cross-binding parity vectors**
(`bindings/test-vectors/`) should gain money-precision cases (a value like
`"0.30"` that `f64` cannot represent) so the two bindings can never silently
diverge on money again.

## Extent (for scoping)

```
Node:   ~50 f64 money fields   (grep 'f64' bindings/node/src/lib.rs, money-named)
Python: ~51 to_f64_result call sites on money fields
```
