# Durable purchases for sandboxed agents

For cross-repository compatibility findings and the concrete Set transaction
verifier, see [Set integration gates](set-integration-gates.md). In particular,
the inspected sequencer's server-generated intent UUID currently blocks
pre-signed payer authorization; the full path is not yet interoperable.

The Node binding's `@stateset/embedded/purchase-runtime` entrypoint coordinates
a purchase across inventory, payment and order systems. It imports neither the
CLI nor a model SDK. This API is included in source release 1.33.0; registry
availability depends on the separate Node package release pipeline. The caller
supplies a `better-sqlite3`-compatible database.

An agent submits only a quote ID, an idempotency key and spending constraints.
The operator supplies identity, quote verification, authorization and adapters.
Keep those operator capabilities outside model-controlled code and arguments.
An agent with unrestricted access to the database or credentials can bypass
this boundary; the runtime is not an operating-system sandbox.

## Simple agent-facing `commerce.buy()` (source tree)

`createAgentCommerce` adds a closed, three-field purchase API over the durable
runtime. It does not modify the native `Commerce` class or install payment rails.
The host configures currency-to-chain/token mappings, payer, token precision and
budget; the agent cannot override them:

```javascript
import { createAgentCommerce } from '@stateset/embedded/purchase-runtime';

const commerce = createAgentCommerce({
  ...operatorRuntimeConfiguration, // store, identity, policy, verified quotes, adapters
  currencies: {
    USDC: {
      asset: operatorUsdcAssetId, // eip155:<chainId>/erc20:<tokenAddress>
      decimals: 6,
      payer: operatorPayerAddress,
      budgetId: 'procurement-september',
    },
  },
  allowApply: false, // only the operator may enable writes
});

const purchase = await commerce.buy({
  quoteId: 'quote-91',
  idempotencyKey: 'procurement-request-184',
  maxTotal: { amount: '5000.00', currency: 'USDC' },
});
```

Provision the budget in the supplied store using `commerce.scope` and the same
**full asset identifier**, not the ticker. This is an EVM ERC20 profile. USDC on
different chains, USD and SSDC are not interchangeable. The host must verify the
token deployment and its decimals; configuration does not attest them.

Quotes additionally require an explicit `payee` address and the exact configured
asset ID. Successful payment evidence must contain matching `payer`, `payee`,
`amount` and `asset`. Excess token precision is rejected before reservation.
Payment-evidence mismatches retain the budget hold and enter reconciliation;
they never authorize order creation. These checks do not authenticate an
untrusted adapter: chain receipts, contract events, signatures and finality must
be verified by the operator-owned payment integration.

The returned purchase ID is durable. `get`, `pending`, `resume`, `recover` and
`cancel` retain the runtime's scoped recovery contracts. `completed` means the
four purchase steps completed—not shipped or delivered. The receipt digest binds
a local summary; it is not an independently signed settlement receipt.

### Process-death regression

```bash
node --test cli/test/unit/purchase-runtime.test.js cli/test/unit/purchase-crash.test.js
```

The crash test commits a payment in a separate SQLite **provider double**, then
SIGKILLs the buyer before it checkpoints payment evidence. A fresh process
reconciles the payment, creates one order, confirms one reservation, and captures
the budget once. Replaying `buy()` returns the same purchase and receipt. An
injected clock advances past the dead worker's lease; this is not a measured
recovery-time benchmark. No network, signing key or real token is used.

The next integration gate is this same failure test against Set's USDC/x402
settlement path and native commerce adapters. A sequencer acknowledgement or
successful batch transaction alone is not proof that an individual payment
settled. Unknown outcomes must remain held until authoritative reconciliation.

## Lower-level runtime configuration

### Set batch-settlement adapter (source tree)

`createSetPaymentAdapter` implements the purchase adapter interface for Set's
`contracts/commerce/SetPaymentBatch.sol` `PaymentSettled` event ABI. This is **not**
a generic x402 HTTP client or an EIP-3009 transfer adapter; those signing and
submission protocols must not be interchanged.

```javascript
import { createSetPaymentAdapter } from '@stateset/embedded/purchase-runtime';

const pay = createSetPaymentAdapter({
  chainId: operatorChainId, // decimal string, no implicit network
  settlementContract: operatorSettlementContract,
  token: operatorUsdcToken,
  payer: operatorPayerAddress,
  rpc: operatorRpc, // (method, params) => parsed JSON-RPC result
  submit: durableSignedSubmission,
  findTransaction: durableTransactionLookup,
  allowSubmit: false, // independent explicit operator opt-in
});
// Supply pay as operatorRuntimeConfiguration.adapters.pay.
```

The canonical asset ID is `eip155:<chainId>/erc20:<lowercase-token-address>`.
Amounts must be exactly representable in six decimal places. Submission receives
immutable-by-copy `chainId`, `settlementContract`, `payer`, `payee`, `token`,
base-unit `amount`, `validUntil`, `intentId`, and `idempotencyKey`. The bytes32
intent ID defaults to a versioned SHA-256 commitment to the step key and these
terms. The submission integration must use it unchanged.

For the coordinated sequencer protocol, explicitly configure
`intentEncoding: 'sequencer-uuid-v1'` and an operator-owned
`getSequencerCapabilities()` callback. This derives a deterministic UUID v8,
zero-pads its bytes to match the sequencer's on-chain encoding, and checks server
capability before submission. The gateway must send that UUID as `intent_id`
before signing; it must not let the server choose a different identity. Never
switch encoding for unresolved operations. See the
[cross-repository integration gates](set-integration-gates.md) for remaining
gateway, authorization-admission and end-to-end verification work.

The host **must persist that binding before signing or broadcasting**, allocate
and persist the payer nonce, build the contract's EIP-712 authorization, enforce
signing policy, and durably index transaction hashes by the same intent/key.
The journal below supplies persistence; chain-specific encoding and signing
remain host integrations. This adapter never
reads private keys, grants allowances, deploys contracts, or chooses endpoints.

Reconciliation checks chain ID, successful transaction receipt, exactly one
matching event from the configured settlement contract, intent ID, payer, payee,
token, base-unit amount, transaction/block/log identity, and canonical block hash.
The receipt block must be at or below the RPC's `finalized` head; unsupported
finalized queries fail closed, with no confirmation-count fallback. See the
[Ethereum JSON-RPC specification](https://ethereum.org/en/developers/docs/apis/json-rpc/).
The returned `rpc_finalized` evidence trusts the operator's RPC and configured
contract. It is not a light-client proof, contract-code attestation or independent
L1 finality verification; deployment and rollup-specific semantics need review.

A successful batch without this payment's event, a reverted batch, an unknown
transaction, or a reorg does not authorize completion. This adapter never returns
`not_found` or definitive `failed`: an outstanding signed intent might settle in
another batch. Missing evidence holds the budget and requires reconciliation,
not resubmission or automatic cancellation. Retain lookup access for every old
configuration while its obligations remain unresolved.

```bash
node --test cli/test/unit/set-payment.test.js
```

These tests use mocked RPC responses and submission, including a purchase-runtime
integration test. Live Set deployment compatibility, durable signing/submission,
testnet settlement, and the process-kill test against the real rail remain gates.

### Durable Set submission journal (source tree)

`createDurableSetSubmission` provides the `submit` and `findTransaction` callbacks
for the payment adapter. Its SQLite journal records the immutable intent and a
uint64 payer authorization nonce atomically, then persists an immutable transaction
plan, then independently validated signed bytes and their transaction hash. Only
after the signed artifact commits may it call the broadcast callback.

```javascript
import { createDurableSetSubmission } from '@stateset/embedded/purchase-runtime';

const submission = createDurableSetSubmission({
  db: operatorJournalDatabase,
  scope: 'acme:procurement',
  nonceStart: operatorUnusedPayerNonce, // decimal string; not inferred from chain
  prepare: operatorPrepareTransactionPlan,
  sign: operatorConstrainedSigner,
  validateSigned: operatorIndependentTransactionVerifier,
  broadcast: operatorBroadcastRawTransaction,
  authorize: operatorSubmissionAuthorization, // must return exactly true
  allowSubmit: false,
});
// Pass submission.submit and submission.findTransaction to createSetPaymentAdapter.
```

The supplied database must use durable storage and appropriate WAL/synchronous
settings. Protect it like a signing system: signed transactions are broadcastable
bearer artifacts. The journal never returns raw bytes through transaction lookup
or recovery results. It is not a secret vault or OS sandbox.

Callback responsibilities are part of the security boundary:

- `prepare({ intent, payerNonce })` returns a JSON-serializable plan. It must safely
  repeat for the same intent, pin the **relayer transaction nonce** separately from
  the payer authorization nonce, and must not sign or broadcast.
- `sign({ intent, payerNonce, plan })` returns `{ rawTransaction, transactionHash }`.
  It must safely repeat for the same plan and must never broadcast internally.
- `validateSigned(input, rawTransaction)` must independently decode and verify the
  full transaction and EIP-712 authorization: chain, contract, payer/payee, token,
  amount, intent ID, validity window, payer nonce, relayer nonce and all other
  authorized calls/value. Return the independently computed transaction hash.
  A callback that merely echoes the supplied hash is **not** a verifier.
- `broadcast(rawTransaction)` submits exactly those bytes and returns their hash.
  Do not replace, reprice or re-sign transactions inside this callback.
- `authorize(intent)` is checked before preparation, signing and broadcasting.
  Revocation stops new broadcast, but does not erase signed artifacts or disable
  read-only transaction lookup. On-chain/account policy remains necessary.

Nonce counters are shared across scopes for the same chain, settlement contract
and payer **within this database**. Every writer for that payer namespace must
coordinate through the same journal or an equivalent external nonce allocator.
The operator must provision an unused starting nonce and reconcile existing chain
usage; separate database copies cannot safely allocate independently. Restoring
stale backups requires nonce/intent reconciliation before enabling submissions.

After a lost broadcast response, lookup returns the already-persisted hash—even
if broadcast acknowledgement was never stored. The operator should run
`submission.recover()` before purchase recovery. It rebroadcasts the **same signed
bytes**, never creates a new transaction. This also covers a crash after artifact
persistence but before broadcast. Broadcast acknowledgement is not settlement.
Concurrent workers may prepare/sign more than once, but only the persisted winning
artifact is broadcast. Pure lookup never signs or broadcasts, even in read-only mode.

Recovery returns bounded result arrays. Use `{ limit, after: lastIdempotencyKey }`
to advance past denied or stuck work, and periodically restart the scan. Expired
or revoked intents remain journaled for operator reconciliation; errors do not
release purchase budget holds. The journal does not implement replacement,
gas repricing, nonce-gap repair, or cancellation of outstanding authorizations.

```bash
node --test cli/test/unit/set-submission.test.js
```

Tests cover database reopen after a lost broadcast response, artifact-write
failure, concurrent workers, scope/nonce isolation, tampering, authorization
changes, and paged recovery. Signatures and hashes in these tests are explicit
test doubles, not evidence of live Ethereum/Set signing compatibility.

### Runtime setup

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
