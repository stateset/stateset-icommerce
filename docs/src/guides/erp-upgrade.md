# ERP upgrade and reconciliation

This release changes ERP contracts and database invariants. Back up the database and verify the backup on a copy before upgrading. Run the new engine against a copy of production data, inspect migration results and reconcile existing inconsistencies before enabling writes. Do not alternate old and new binaries against the upgraded schema.

## Client changes

- Node finance amounts and finance MCP inputs/outputs use decimal strings. Send `"125.50"`, not `125.5`. Invoice item quantities, finance rates and cost values also use strings. Existing `*Exact` aliases remain where documented. Core commerce domains that retain legacy numeric aliases are outside this finance contract; use their exact APIs for financial calculations.
- Invoice creation accepts actual lines, currency and due date. Invoice responses expose `currency` and `balanceDue`. Use `balanceDue`, not `total - amountPaid`, because partial write-offs affect the collectible balance separately.
- Currency input validates the three-letter code format; it is not an assigned-ISO-code registry lookup. Invalid formats are rejected instead of defaulted.
- Purchase-order receiving requires warehouse and receiving-location IDs. Work orders require source/output routing. Picked stock remains on hand in an explicit staging location until shipment.
- Draft-only invoice and PO edits replace previously accepted edits to issued/approved documents. Published BOMs require a new revision. Lifecycle methods return conflicts on invalid transitions.
- Bills and payment runs record makers and reject self-approval. Trusted native hosts must supply actor identities; legacy records without a maker cannot retroactively establish maker-checker history.

## Identity and retries

HTTP approvals, settlements and postings require an Admin principal. Request-body `approved_by`, `posted_by`, `closed_by` and `locked_by` values do not establish audit identity. A trusted authentication adapter must bind the actor. Do not trust caller-supplied identity headers.

Supply `Idempotency-Key` on ERP HTTP mutations, including relevant PUT/PATCH/DELETE requests. Persist one key per logical operation, reuse its exact body on retry and use a new key for a different operation. Replay protection lasts for the configured receipt retention (24 hours by default). Missing keys return 428; conflicting reuse returns 422. If the engine cannot prove whether a previous request finished, the stored claim refuses execution and requires reconciliation. Do not delete the claim and blindly repeat a money/stock movement. Invoice `payment_id` provides an additional engine-level retry identity.

MCP uses `--actor <service-principal>` or `STATESET_ERP_ACTOR` supplied by the operator. Separate maker and approver endpoints/principals and configure the permission gate; `--apply` alone grants the legacy operator endpoint mutation authority. Governed journal posting uses the trusted kernel principal and requires kernel policy. MCP does not inherit HTTP idempotency middleware: use operation IDs where provided and inspect state before retrying an uncertain non-idempotent creation.

Admin financial actions verify the user with the identity service, require an administrator/owner role and stamp that identity. The development auth bypass is disabled in production. Do not share a single user identity between makers and approvers.

## Existing data that needs review

Migrations add custody, posting, history and identity records; they cannot infer every missing business event. In particular:

- Compare warehouse totals, physical locations, bins, lots, reservations, transit and cost layers. Resolve orphaned or ambiguous placements before receiving, transferring or consuming them. Reconcile legacy production-batch links to actual work orders.
- Review prepayment/vendor-credit applications against their target bill or obligation. Old applications may not have reduced target balances. Reconcile target balances and related ledger entries before paying again; this release does not automatically replay historical settlements.
- Review historical partial write-offs. Older behavior may have marked an entire invoice written off. Reconcile its remaining receivable and bad-debt record before reversing/reapplying amounts.
- Legacy refunded prepayments reconstruct the refunded amount exactly from original amount minus active applications; their stored update time becomes the available historical refund timestamp. Review that timestamp against bank evidence. It cannot recover an unrecorded event time.
- Statements refuse undated legacy direct payments without receipt history. Reconstruct dated receipts from authoritative payment/bank records through a reviewed migration; do not assign today's date simply to make a report pass.
- Credit enforcement includes all unpaid orders and issued invoices for customers with credit accounts. Unlinked manual credit charges/reservations are included conservatively. Reconcile overlap and account currency before re-enabling ordering for existing credit customers.
- Validate posting account mappings, open periods and historical FX rates. Reports now retain reversed originals and include unclosed earnings. Compare dated trial balances and subledgers after upgrade; changed totals can expose old missing/duplicate postings.

Credit memos against invoices linked to deferred-revenue contracts currently refuse automatic posting because a contract-adjustment workflow is required. Resolve the contract/schedule and ledger treatment explicitly with the operator; do not bypass the refusal by detaching the invoice or forcing a revenue-account credit.

This release does not add MRP, capacity planning, bank reconciliation, intercompany consolidation or a general contract-amendment workflow. The fix plan's separate capability-gap list remains separate from its 59 corrections.

## Verification before resuming operations

On the migrated copy, run representative receipt→put-away→shipment, transfer, manufacturing, supplier settlement and receivable/write-off lifecycles. Compare stock valuation, AP/AR aging and GL control accounts, including historical dates and reversals. Exercise maker-checker denial and identical/conflicting HTTP retries. Retain backup, reconciliation evidence and the migration version with the release record.

The generated [Node reference](../api/node-reference.md) and HTTP OpenAPI describe the supported signatures. Finance and operations MCP profiles run mandatory fresh-store smoke checks and seeded native workflow tests. These gates do not certify pre-existing production data; reconciliation is specific to the store being upgraded.
