# Economic kernel release gates

Status: engineering acceptance criteria, not a production certification.

A high tool count or passing unit suite does not establish safe autonomous
commerce. Release readiness requires evidence for the full economic lifecycle.
These gates track the source tree, including unreleased changes.

The [Set integration review](set-integration-gates.md) tracks the coordinated
intent-identity handshake and the remaining admission, gateway and real settlement
verification. Local cryptography and the opt-in handshake do not establish
cross-service production readiness.

The machine-readable implementation backlog is
[`kernel/foundation-program.json`](../../kernel/foundation-program.json). An item
requires its acceptance evidence before it can be marked complete; local test
success does not substitute for provider or independent implementation evidence.

| Gate                      | Current evidence                                                                                                                         | Required before claiming production completeness                                                                           |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Exact money               | Rust decimal primitives; integer reference arithmetic; CLI provider decimal capture/refund, tax, shipping and reconciliation regressions | Audit remaining binding/import/provider conversions and share asset conformance vectors across runtimes                    |
| Inventory consistency     | Duplicate-SKU reservations are checked together; reference snapshots use current balances; kernel reservation receipt tests              | Concurrent buyers against durable merchant storage; recovery across acceptance, expiration, fulfillment and release        |
| Durable purchases         | Persisted dispatch, shared asset holds, authoritative lookup, fenced workers, scoped recovery and cancellation tests                     | Kill processes at each external commit boundary; prove no duplicate effects with actual providers                          |
| End-to-end authority      | Operator-owned identity/policy, signed acceptance, governed command adapters                                                             | Cover purchase, fulfillment, return, refund and dispute workflows; enforce budgets across all enabled entrypoints          |
| Settlement and accounting | Simulated settlement plus local receipt evidence                                                                                         | Real provider reconciliation, finality/reorg handling where applicable, double-entry reconciliation and governed reversals |
| Verifiable receipts       | Core signature and role/key checks; reference co-signing                                                                                 | Operated key registry, rotation/revocation distribution, independent verification against real provider evidence           |
| Portable runtime          | Native engine bindings and standalone Node purchase entrypoint                                                                           | Equivalent purchase/recovery contracts in supported languages and PostgreSQL, including migration and concurrency tests    |
| Operational reliability   | Local recovery and test harnesses                                                                                                        | Measured SLOs, load/soak results, backup restoration, outage runbooks and alerts for unresolved money                      |
| Independent assurance     | Repository trust inventory                                                                                                               | External security review, remediation verification and adversarial agent-task evaluation                                   |

## Immediate implementation order

1. Adopt native Commerce aggregates in merchant transactions. Optional reference
   SQLite persistence now covers protocol projections, atomic acceptance, restart
   replay and two-worker inventory contention (see [durable merchant](durable-merchant.md)).
   A separate `NativeMerchantCheckout` bridge now commits native orders, stock,
   budgets, outbox and receipts with lost-response recovery tests. It is now
   connected to an opt-in native HTTP integration via
   `cli/examples/native-merchant.mjs`: signed quotes bind native snapshots,
   strict-stock acceptance creates native orders, and receipt recovery repairs
   protocol checkpoints after process death. Payment remains pending unless the
   optional provider reconciliation records verified capture; demo
   settlement endpoints are disabled in native mode. The source tree supports a versioned cart
   fingerprint checked inside native checkout; hosts must save it when issuing
   the quote and enable `requireCartFingerprint` on the bridge. This binds full
   cart terms, not just total and currency. A source-tree transactional checkout
   stock policy is implemented
   for SQLite and PostgreSQL. The Node bridge supports strict-stock acceptance
   for tracked SKUs and checks native feature support before dispatch. The HTTP
   integration has separate native and protocol commit boundaries, with signed
   retries recovering from the native ledger. A single transaction spanning
   order, reservation, escrow intent and protocol event, live PostgreSQL
   verification, and cross-store restore reconciliation remain open.
2. Connect one real payment provider using an operator-owned adapter. Define
   authoritative lookup, idempotency lifetime, recipient/asset checks and finality
   before enabling spend. Never treat a timeout or cache miss as proof of failure.
   The native HTTP path now has an opt-in Stripe GET-only adapter for existing
   direct USD card PaymentIntents. It validates account, quote binding and full
   capture before governed `payments.create`/`payments.complete` recording.
   Native-backed tests exercise concurrent retries, process death after both
   payment steps, and protocol checkpoint recovery using controlled provider
   responses. External charging, live-provider drills, refunds/disputes and
   accounting reconciliation remain open; this does not satisfy the live gate.
3. Complete governed fulfillment, refund and dispute recovery around that path.
   Optional operator-approved full and partial dispatch recording now uses `orders.ship_reserved`
   with an immutable order version, native receipt recovery and a separate
   protocol checkpoint. The command consumes existing reserved stock atomically
   with order quantities, inventory ledger, events and receipt. Legacy `orders.ship`
   receipts retain their confirmation-only semantics. The kernel also supports
   partial stock consumption with line-bound holds and prior shipment receipt
   checks on SQLite and PostgreSQL. The HTTP adapter journals successive dispatches,
   recovers pinned commands before considering new approvals, and preserves
   cumulative progress across stale responses and checkpoint failures. Carrier
   dispatch and delivery verification remain open.
   Preserve committed spend until a verified reversal is reconciled. Ensure an
   order adopts an existing reservation instead of allocating inventory twice.
4. Unify spending controls across the enabled paths and run multi-worker crash
   and concurrency tests. Count economic effects, not merely successful requests.
5. Publish reproducible workload results and obtain independent security review.

## CLI sandbox and shadow provider money contract

Payment, tax and shipping providers under `cli/src/tools/providers/` calculate
with isolated decimal arithmetic and return decimal strings. The explicit fiat
currency allowlist in `money.js` defines zero, two or three fractional digits.
Unknown currencies and token symbols are rejected: a token symbol alone does
not specify chain, contract and scale. Provider-specific denomination exceptions
belong in a real adapter, not a guessed global default.

Payment instructions reject negative, zero, malformed and excess-precision
amounts instead of rounding a charge. Computed tax lines and shipping charges
round half away from zero at the currency boundary. Safe legacy numeric tool
inputs remain accepted for compatibility; exact decimal strings are preferred.
Conversion cannot recover precision already lost by a caller. New integrations
must send strings. Normalized shadow webhook `amount` fields use major units;
`amount_received`, `amount_captured`, `amount_refunded` and `amount_minor` (and
their camelCase aliases) use integer currency minor units. Raw Stripe payloads
must be mapped by an adapter before using this envelope.

Settlement batches and reconciliation expose per-currency totals. Legacy scalar
totals are `null` for mixed-currency results; callers must use `totalsByCurrency`.
Payment create/capture/refund idempotency keys bind normalized requests and reject
conflicts. Terminal-state retries return the original operation. These maps are
still in memory and do not provide durable provider idempotency.

Regression command (from the repository root):

```sh
node --test cli/test/unit/provider-money.test.js cli/test/unit/payments-tools.test.js cli/test/unit/tax-tools.test.js cli/test/unit/shipments-tools.test.js
```

This proves local arithmetic, validation and replay behavior. It does not certify
live tax rates, currency conversion, real settlement, webhook ordering or recovery
across provider/database commits.

## Reference money contract

Money inputs are nonnegative decimal strings with at most 20 integer digits and
18 fractional digits. Quantities are positive safe integers. Payouts in the demo
use USDC only; an authority cap denominated in another currency is rejected.

Discounted unit prices retain their exact precision. Quote line totals round
half-up to cents and proposal totals sum those displayed line totals. Fresh
purchase quotes apply their demo handling fee and round the final total to cents.
Payout fees round half-up to cents; net proceeds and remaining balances preserve
sub-cent precision so net plus fees equals the exact debit. Returns never round
up through a supplied refund ceiling. Actual rails may require stricter precision;
their adapters must reject unrepresentable amounts, not silently change them.

Compatibility: discounted unit-price strings may now contain more than two
fractional digits. Proposal purchases treat `max_total` as a numeric ceiling,
not a string-equality check, while enforcing currency and merchant matching.

The reference handler remains a demo with optional SQLite persistence and simulated settlement.
Exact arithmetic does not make its principal bindings, wallet defaults or payout
authorization into production funds-transfer controls.
