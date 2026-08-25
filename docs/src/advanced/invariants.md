# Commerce Invariants

StateSet's guarantee is not that an agent will ask for sensible things. It is that
**the engine refuses to record a state the books cannot justify** — no matter which
agent, binding, MCP tool, or HTTP client asked.

This page is the catalogue of those guarantees: what is promised, where it is
enforced, and how it is proven. Each invariant holds after *every* operation, on
both the SQLite and PostgreSQL backends.

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

| # | Guarantee | Enforced at |
|---|-----------|-------------|
| P1 | Σ completed refunds ≤ amount captured | `sqlite/payments.rs`, `postgres/payments.rs` |
| P2 | Σ completed **and in-flight** refunds ≤ amount captured — two concurrent refunds cannot both pass | same, inside the write transaction (`BEGIN IMMEDIATE` / `SELECT … FOR UPDATE`) |
| P3 | `payments.amount_refunded` equals Σ completed refunds | same |
| P4 | A refund amount is strictly positive | same |
| P5 | A **completed** refund cannot be transitioned to failed | status guard on the update; violating it silently corrupted `amount_refunded` before v1.25.0 |
| P6 | Σ captures (completed and in-flight) ≤ the order total | `capturing_statuses()` fold in both backends |

P2 and P6 are deliberately computed *inside* the same transaction that writes, not
before it. A check that runs outside the transaction is not a guarantee.

## Orders

| # | Guarantee | Enforced at |
|---|-----------|-------------|
| O1 | Captured ≤ order total | see P6 |
| O2 | Refunded ≤ captured | see P1 |
| O3 | Returned quantity per line ≤ quantity ordered | `validate_return_item_tx` |
| O4 | Order total foots to its line items; each line total = qty × unit price − discount + tax | model + engine agree after every op |
| O5 | A cancelled order holds no live inventory reservation | reservation release on cancel |

## Returns

| # | Guarantee | Enforced at |
|---|-----------|-------------|
| R1 | A return may only be requested against an order whose goods have shipped | `ensure_order_returnable` (`sqlite/returns.rs`, `postgres/returns.rs`) |
| R2 | A return's `refund_amount` foots to its items | engine + model |

R1 exists because a return opened against an unfulfilled order can carry a refund
amount for goods that never left the building.

## Inventory

| # | Guarantee | Enforced at |
|---|-----------|-------------|
| I1 | `on_hand` ≥ 0 | reservation and adjustment paths |
| I2 | `allocated` ≥ 0 | same |
| I3 | `allocated` ≤ `on_hand` | same |
| I4 | `available` = `on_hand` − `allocated` | derived, never stored independently |
| I5 | Σ inventory movements reconciles to `on_hand` — the ledger explains the balance | movement rows written in the same transaction as the balance change |
| I6 | `allocated` = Σ live order reservations | `reserve_in_tx` with an optimistic version guard |

## General ledger and AR

| # | Guarantee | Enforced at |
|---|-----------|-------------|
| G1 | Every posted journal entry balances: Σ debits = Σ credits | GL posting |
| G2 | Every journal line is a pure debit or a pure credit, never both | GL posting |
| G3 | The trial balance nets to zero | consequence of G1 |
| G4 | The AR control account balance = Σ open invoice balances | auto-posting |
| G5 | `invoice.balance_due` = total − amount paid | invoice write path |

## Money

| # | Guarantee | Enforced at |
|---|-----------|-------------|
| M1 | No stored monetary value carries more decimal places than its currency allows | checked on every order, item, payment, refund, return, invoice, journal line and trial-balance figure |
| M2 | Monetary arithmetic is exact decimal, never binary floating point | `rust_decimal` end to end; `decimal_sum` for SQLite aggregates; see [Money: Storage & Arithmetic](money.md) |

## Atomicity

| # | Guarantee | Enforced at |
|---|-----------|-------------|
| A1 | A rejected operation writes nothing — entity counts are unchanged | every guard validates before the first write, inside the transaction |
| A2 | A rejected operation returns a typed `CommerceError`; it never panics | verified by the harness, which distinguishes the two |

A1 is why the harness counts orders, payments, returns, invoices and refunds after
every step: a guard that rejects *after* a partial write would leave the counts
right but the books wrong.

## What this catalogue does not yet cover

Stated plainly, because a trust document that overstates itself is worse than none:

- These are **engine** guarantees. They are enforced for every caller, but they are
  not yet expressed as stable, documented error codes an agent can branch on —
  today a violation surfaces as a typed error with a human-readable message.
- The ICP conformance suite (`icp-conformance/`) verifies protocol-level behaviour
  — AID derivation, canonical JSON, signatures, escrow lifecycle, timing, ceilings.
  It does **not** yet assert the economic invariants on this page.
- Bin-level sub-allocation, where present, reconciles to warehouse on-hand but is
  not itself the reservation source of truth.
