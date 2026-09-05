# Durable purchases for sandboxed agents

The Node binding's `@stateset/embedded/purchase-runtime` entrypoint coordinates
a purchase across inventory, payment and order systems. It imports neither the
CLI nor a model SDK. This API is included in source release 1.32.0; registry
availability depends on the separate Node package release pipeline. The caller
supplies a `better-sqlite3`-compatible database.

An agent submits only a quote ID, an idempotency key and spending constraints.
The operator supplies identity, quote verification, authorization and adapters.
Keep those operator capabilities outside model-controlled code and arguments.
An agent with unrestricted access to the database or credentials can bypass
this boundary; the runtime is not an operating-system sandbox.

```javascript
import Database from 'better-sqlite3';
import { PurchaseRuntime, SqlitePurchaseStore } from '@stateset/embedded/purchase-runtime';

const db = new Database('./purchases.db');
db.pragma('journal_mode = WAL');
db.pragma('busy_timeout = 5000');
const store = new SqlitePurchaseStore(db);
const runtime = new PurchaseRuntime({
  store,
  identity: {
    agentId: 'acme.procurement.7', principalId: 'acme',
    tenantId: 'acme', storeId: 'production',
  },
  policyVersion: 'procurement-v4',
  resolveQuote: operatorVerifiedQuoteLookup,
  authorize: operatorAuthorization,
  adapters: operatorAdapters,
  allowApply: false, // Preview by default. Only the operator enables execution.
});
store.provisionBudget(runtime.scope, {
  id: 'procurement-september', asset: 'USDC', limit: '100000',
  expiresAt: '2026-10-01T00:00:00Z',
});

const purchase = await runtime.buy({
  idempotencyKey: 'procurement-request-184', quoteId: 'quote-91',
  budgetId: 'procurement-september', maxAmount: '5000', asset: 'USDC',
});
```

The three `operator*` values above are integration points, not built-in payment
providers. `resolveQuote` returns a verified snapshot with `id`, `counterpartyId`,
`amount`, `asset` and `expiresAt`. `authorize` must return `{ allowed: true }`
explicitly and can include a decision ID and policy evidence. It is called on
each resume. Changing policy versions prevents new dispatch under an old
operation; already-dispatched effects are still reconciled.

## Adapter contract

Supply five adapters: `reserve_inventory`, `pay`, `create_order`,
`confirm_inventory`, and `release_inventory`. Each exposes asynchronous
`execute(context)` and `lookup(context)`. Context contains an immutable-by-copy
operation snapshot and the stable step `idempotencyKey`.

Every provider must guarantee that repeated execution with that key cannot
repeat the economic effect. Lookup must be authoritative, not an eventually
consistent cache. Responses are:

| Status | Meaning |
| --- | --- |
| `succeeded` | Effect confirmed; include durable `evidence` |
| `failed` | Definitive rejection with no effect; optional `reason` |
| `pending` / `unknown` | Outcome not final; retain the hold and reconcile |
| `not_found` | Lookup has proved no effect exists and same-key submission is safe |

Inventory evidence requires `reservation_id`; order evidence requires
`order_id`; payment evidence requires `transaction_id`, exact decimal `amount`
and `asset` matching the quote. The host must verify the recipient, chain,
finality and authenticity before returning successful payment evidence. The
runtime does not independently query a chain or authenticate provider responses.

`createKernelPurchaseAdapter` adapts governed engine commands to this contract.
It requires operator-owned policy/principal, a deterministic `buildPayload`, an
evidence projection and durable `readReceipt(idempotencyKey)` from the same
commerce store. The basic adapter does not generate delegation credentials or
budget commitments for policies that require them. Kernel rejection stays a
rejection; do not relax production policy to make a demo pass.

Order creation must adopt the existing inventory reservation. Do not independently
call checkout and allocate the same units again. Confirming inventory retains
the allocation; shipment, not purchase completion, consumes on-hand stock.

## Recovery and money invariants

Workers do not need to retain operation IDs in memory. `runtime.pending()` is
a read-only, identity-scoped view of unfinished purchases, including cases that
need operator attention. `runtime.recover()` processes one bounded page:

```javascript
const page = await runtime.recover({ limit: 100, after: savedCursor });
for (const result of page.results) {
  // Handle result.error, operation.busy and operation.skipped explicitly.
  // Persist/report needs_attention cases for an operator; never treat them as success.
  reportRecoveryResult(result);
}
savedCursor = page.nextCursor;
```

`savedCursor` is initially `null`; it and `reportRecoveryResult` belong to the
operator's scheduler. Limits must be integers from 1 to 1,000. Pagination orders
by operation ID, so completing an earlier page does not shift later pages.
These are live pages, not snapshots: after reaching a null cursor, start the next
scheduled scan from null to revisit uncertain outcomes and discover new work.
Report failures before advancing the cursor. One operation's failure does not
prevent the rest of the page from being processed.

Recovery requires apply mode, respects active leases, and rechecks status under
the lease. It reports `needs_attention` operations as skipped; use explicit
`resume(id)` only after operator investigation. There are no hidden timers,
automatic infinite retries, or wall-clock timeout guarantees. Providers must
bound their own requests, and the operator supplies scheduling/backoff.

`runtime.resume(purchase.id)` continues after restart using the same database
and operator configuration. Dispatch checkpoints are persisted before side
effects. Lost responses trigger lookup before any repeat submission. Expiring
worker leases fence local writes; external idempotency is still required when
a slow worker overlaps its replacement.

Budget reservation and operation creation are one SQLite transaction. Agents
sharing principal, tenant, store and budget ID contend on the same balance.
Payment confirmation atomically moves reserved funds to spent funds. The
budget is an exact asset-denominated purchase-runtime ledger, not a replacement
for kernel fiat budgets or a synchronized wallet balance. Other execution paths
do not automatically consume it. Budget definitions are immutable; provision a
new ID for a new period.

`runtime.cancel(id)` first reconciles outstanding effects. A definitive unpaid
failure releases inventory before releasing budget. An unknown payment holds
both. A settled payment requires a separately governed refund and never silently
restores spending capacity. Failed compensation remains on the compensation
path. `needs_attention` requires operator investigation; it is not success.

Completion means payment, order and committed allocation are confirmed—not
shipment, delivery or irrevocable settlement. The returned receipt is a local
SHA-256-bound summary retaining adapter evidence. Its digest is not a signature
or nonrepudiation proof. Use authenticated provider receipts and the core
role-aware receipt verifier when those guarantees are required.

## Validation and remaining integration work

From the repository root, with CLI dependencies installed:

```bash
node --test cli/test/unit/purchase-runtime.test.js
node cli/examples/durable-purchase.mjs          # Preview
node cli/examples/durable-purchase.mjs --apply  # Local simulated effects only
```

The runnable example uses three deterministic agent clients sharing a budget.
It loses a payment response, rejects competing purchases, reopens the database
and reconciles to one simulated payment. The database is retained under a printed
temporary path for inspection. No LLM, live merchant or payment network is used.

The suite covers preview, exact replay, conflicting replay, reopening SQLite
after a lost payment response, shared-budget contention, cancellation,
revocation, compensation and real kernel inventory receipt lookup. Payments in
these tests are simulated; no funds move.

Still required for a live deployment: verified quotes and delegation, a real
payment adapter with finality/reconciliation semantics, merchant order adoption,
refund/dispute workflows, scheduled reconciliation, backups, protected operator
keys and an end-to-end failure-injection run against those providers. This Node
SQLite coordinator does not establish PostgreSQL or cross-language parity.
