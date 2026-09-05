# Economic kernel release gates

Status: engineering acceptance criteria, not a production certification.

A high tool count or passing unit suite does not establish safe autonomous
commerce. Release readiness requires evidence for the full economic lifecycle.
These gates track the source tree, including unreleased changes.

| Gate                      | Current evidence                                                                                                                                 | Required before claiming production completeness                                                                           |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------- |
| Exact money               | Rust decimal primitives; reference quote, subscription, return and payout arithmetic uses integers; malformed input and conservation regressions | Audit every provider conversion and enforce asset precision at payment boundaries                                          |
| Inventory consistency     | Duplicate-SKU reservations are checked together; reference snapshots use current balances; kernel reservation receipt tests                      | Concurrent buyers against durable merchant storage; recovery across acceptance, expiration, fulfillment and release        |
| Durable purchases         | Persisted dispatch, shared asset holds, authoritative lookup, fenced workers, scoped recovery and cancellation tests                             | Kill processes at each external commit boundary; prove no duplicate effects with actual providers                          |
| End-to-end authority      | Operator-owned identity/policy, signed acceptance, governed command adapters                                                                     | Cover purchase, fulfillment, return, refund and dispute workflows; enforce budgets across all enabled entrypoints          |
| Settlement and accounting | Simulated settlement plus local receipt evidence                                                                                                 | Real provider reconciliation, finality/reorg handling where applicable, double-entry reconciliation and governed reversals |
| Verifiable receipts       | Core signature and role/key checks; reference co-signing                                                                                         | Operated key registry, rotation/revocation distribution, independent verification against real provider evidence           |
| Portable runtime          | Native engine bindings and standalone Node purchase entrypoint                                                                                   | Equivalent purchase/recovery contracts in supported languages and PostgreSQL, including migration and concurrency tests    |
| Operational reliability   | Local recovery and test harnesses                                                                                                                | Measured SLOs, load/soak results, backup restoration, outage runbooks and alerts for unresolved money                      |
| Independent assurance     | Repository trust inventory                                                                                                                       | External security review, remediation verification and adversarial agent-task evaluation                                   |

## Immediate implementation order

1. Adopt native Commerce aggregates in merchant transactions. Optional reference
   SQLite persistence now covers protocol projections, atomic acceptance, restart
   replay and two-worker inventory contention (see [durable merchant](durable-merchant.md)).
   A separate `NativeMerchantCheckout` bridge now commits native orders, stock,
   budgets, outbox and receipts with lost-response recovery tests. It is not yet
   wired into the HTTP merchant, and host-owned quote/cart immutability remains
   required. A source-tree transactional checkout stock policy is now implemented
   for SQLite and PostgreSQL. The Node bridge supports strict-stock acceptance
   for tracked SKUs and checks native feature support before dispatch; HTTP
   merchant integration and live PostgreSQL verification remain pending.
   Quote acceptance must commit the order, reservation, escrow intent and event
   record atomically. Restart must preserve signer bindings and replay protection.
2. Connect one real payment provider using an operator-owned adapter. Define
   authoritative lookup, idempotency lifetime, recipient/asset checks and finality
   before enabling spend. Never treat a timeout or cache miss as proof of failure.
3. Complete governed fulfillment, refund and dispute recovery around that path.
   Preserve committed spend until a verified reversal is reconciled. Ensure an
   order adopts an existing reservation instead of allocating inventory twice.
4. Unify spending controls across the enabled paths and run multi-worker crash
   and concurrency tests. Count economic effects, not merely successful requests.
5. Publish reproducible workload results and obtain independent security review.

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
