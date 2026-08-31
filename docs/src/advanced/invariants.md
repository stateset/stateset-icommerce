# Commerce Invariants

StateSet's guarantee is not that an agent will ask for sensible things. It is that
**the engine refuses to record a state the books cannot justify** — no matter which
agent, binding, MCP tool, or HTTP client asked.

This page is the catalogue of those guarantees: what is promised, where it is
enforced, and how it is proven. Each invariant holds after *every* operation, on
both the SQLite and PostgreSQL backends.

## Branching on a violation

Each guarantee below that an agent can act on has a **stable error code**,
listed in its table row. The codes are defined by the conformance vector
`icp-conformance/vectors/icp-1.0/10-commerce-invariants/` (its `description.md`
is normative) and are returned by the engine:

```rust
match repo.create_refund(input) {
    Err(e) if e.invariant_code() == Some("commerce.refund.exceeds_captured") => {
        // the books say this money was never captured — do not retry
    }
    other => other?,
}
```

Over HTTP the same code appears as `error.invariant`:

```json
{
  "error": {
    "code": "validation_error",
    "message": "Refund amount 10.00 exceeds the refundable balance of payment …",
    "invariant": "commerce.refund.exceeds_captured"
  }
}
```

`error.code` keeps its existing HTTP-level taxonomy and the status code is
unchanged, so adding `error.invariant` breaks no existing client.

## How they are proven

`crates/stateset-integration-tests/tests/invariants.rs` is a property test. It
generates random-but-valid sequences of 32–48 commerce operations — stock
receipts, orders, captures, shipments, returns, refunds, AR invoices and payments
— drives them through the embedded engine, and re-checks **every invariant below
after every single step**, comparing against an independent in-memory model.

Operations are wrapped so that a panic is reported distinctly from a typed error:
a rejected operation must fail *as a typed `CommerceError`*, never by crashing and
never by writing part of its effect.

Run it with:

```bash
cargo test -p stateset-integration-tests --test invariants
PROPTEST_CASES=512 cargo test -p stateset-integration-tests --test invariants  # deeper
```

Each invariant also has a hand-written deterministic regression test, so a
violation reproduces without waiting for the generator to rediscover it.

## Payments and refunds

| # | Guarantee | Error code | Enforced at |
|---|-----------|------------|-------------|
| P1 | Σ completed refunds ≤ amount captured | `commerce.refund.exceeds_captured` | `sqlite/payments.rs`, `postgres/payments.rs` |
| P2 | Σ completed **and in-flight** refunds ≤ amount captured — two concurrent refunds cannot both pass | `commerce.refund.exceeds_captured` | same, inside the write transaction (`BEGIN IMMEDIATE` / `SELECT … FOR UPDATE`) |
| P3 | `payments.amount_refunded` equals Σ completed refunds | — | same |
| P4 | A refund amount is strictly positive | — | same |
| P5 | A **completed** refund cannot be transitioned to failed | — | status guard on the update; violating it silently corrupted `amount_refunded` before v1.28.2 |
| P6 | Σ captures (completed and in-flight) ≤ the order total | `commerce.capture.exceeds_order_total` | `capturing_statuses()` fold in both backends |

P2 and P6 are deliberately computed *inside* the same transaction that writes, not
before it. A check that runs outside the transaction is not a guarantee.

## Orders

| # | Guarantee | Error code | Enforced at |
|---|-----------|------------|-------------|
| O1 | Captured ≤ order total | `commerce.capture.exceeds_order_total` | see P6 |
| O2 | Refunded ≤ captured | `commerce.refund.exceeds_captured` | see P1 |
| O3 | Returned quantity per line ≤ quantity shipped (ordered, before shipment) | `commerce.return.exceeds_shipped` | `validate_return_item_tx` |
| O4 | Order total foots to its line items; each line total = qty × unit price − discount + tax | — | model + engine agree after every op |
| O5 | A cancelled order holds no live inventory reservation | — | reservation release on cancel |

## Returns

| # | Guarantee | Error code | Enforced at |
|---|-----------|------------|-------------|
| R1 | A return may only be requested against an order whose goods have shipped | `commerce.return.order_not_shipped` | `ensure_order_returnable` (`sqlite/returns.rs`, `postgres/returns.rs`) |
| R2 | A return's `refund_amount` foots to its items | — | engine + model |

R1 exists because a return opened against an unfulfilled order can carry a refund
amount for goods that never left the building.

## Inventory

| # | Guarantee | Error code | Enforced at |
|---|-----------|------------|-------------|
| I1 | `on_hand` ≥ 0 | `commerce.inventory.insufficient_available` | reservation and adjustment paths |
| I2 | `allocated` ≥ 0 | — | same |
| I3 | `allocated` ≤ `on_hand` | `commerce.inventory.insufficient_available` | same |
| I4 | `available` = `on_hand` − `allocated` | — | derived, never stored independently |
| I5 | Σ inventory movements reconciles to `on_hand` — the ledger explains the balance | — | movement rows written in the same transaction as the balance change |
| I6 | `allocated` = Σ live order reservations | `commerce.inventory.insufficient_available` | `reserve_in_tx` with an optimistic version guard |

## General ledger and AR

| # | Guarantee | Error code | Enforced at |
|---|-----------|------------|-------------|
| G1 | Every posted journal entry balances: Σ debits = Σ credits | `commerce.ledger.entry_unbalanced` | `JournalEntry::ensure_postable`, called by `post_journal_entry` in both backends |
| G2 | Every journal line is a pure debit or a pure credit, never both | `commerce.ledger.line_not_single_sided` | `create_journal_entry` and `ensure_postable`, both backends |
| G3 | The trial balance nets to zero | — | consequence of G1 |
| G4 | The AR control account balance = Σ open invoice balances | — | auto-posting |
| G5 | `invoice.balance_due` = total − amount paid | — | invoice write path |

## Money

| # | Guarantee | Error code | Enforced at |
|---|-----------|------------|-------------|
| M1 | No stored monetary value carries more decimal places than its currency allows | `commerce.money.scale_exceeds_currency` † | **order creation** — `CreateOrder::validate_money_scale` runs inside `validate_order_input` on both backends, before the first write; other write paths are still unguarded, see the footnote |
| M2 | Monetary arithmetic is exact decimal, never binary floating point | — | `rust_decimal` end to end; `decimal_sum` for SQLite aggregates; see [Money: Storage & Arithmetic](money.md) |

## Atomicity

| # | Guarantee | Error code | Enforced at |
|---|-----------|------------|-------------|
| A1 | A rejected operation writes nothing — entity counts are unchanged | — | every guard validates before the first write, inside the transaction |
| A2 | A rejected operation returns a typed `CommerceError`; it never panics | — | verified by the harness, which distinguishes the two |

A1 is why the harness counts orders, payments, returns, invoices and refunds after
every step: a guard that rejects *after* a partial write would leave the counts
right but the books wrong.

## What this catalogue does not yet cover

Stated plainly, because a trust document that overstates itself is worse than none:

- The invariants marked with an error code above are enforced for every caller
  *and* carry that stable code: `CommerceError::invariant_code()` returns it, and
  the HTTP layer surfaces it as `error.invariant` in the JSON body (the existing
  `error.code` and the HTTP status are unchanged). Rows marked `—` are still
  enforced but surface only as a typed error with a human-readable message; they
  have no code in the conformance vector to branch on yet.
- † M1 is enforced on **order creation only**. `CreateOrder::validate_money_scale`
  checks every line's `unit_price`, `discount` and `tax_amount` against
  `CurrencyCode::decimal_places()` for the order's currency, and both the SQLite
  and Postgres `validate_order_input` call it before the first write, so a
  rejected order persists nothing. Scale is *significant* scale — trailing zeros
  do not count, so `10.9900` is valid USD and `10.999` is not, matching vectors
  `scale07`/`scale08`. Every other money write path is still unguarded: payments,
  refunds, invoices and AP bills, carts, and `orders().add_item` (which takes a
  line after creation) accept an over-scaled amount and round only the derived
  totals. For those paths M1 remains a vector-level requirement, not an
  engine-enforced invariant. The ledger codes (G1, G2) and the payment, return
  and inventory codes are live on `CommerceError`.
- The ICP conformance suite (`icp-conformance/`) verifies protocol-level behaviour
  — AID derivation, canonical JSON, signatures, escrow lifecycle, timing, ceilings.
  It does **not** yet assert the economic invariants on this page.
- Bin-level sub-allocation, where present, reconciles to warehouse on-hand but is
  not itself the reservation source of truth.
