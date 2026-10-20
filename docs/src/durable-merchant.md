# Durable reference merchant

The ICP reference handler can persist protocol state in SQLite. This is a
loopback-only reference deployment with simulated balances and settlement—not
a production payment endpoint and not a replacement for native Commerce orders.

## Start explicitly

Install the repository's CLI dependencies. Supply an operator-owned Ed25519 PEM
private key and a stable merchant AID; do not put keys in model arguments or
source control. With Node 20.20 or newer, run from the repository root:

```bash
ICP_MERCHANT_KEY_FILE=/protected/merchant.pem \
ICP_MERCHANT_AID=aid:v1:zYourMerchant \
PORT=8787 node cli/examples/durable-merchant.mjs \
  --apply --demo --db ./merchant-protocol.db
```

Without both `--apply` and `--demo`, the launcher makes no changes and starts no
server. It requires a file-backed database and explicit identity configuration.
SQLite uses WAL, full synchronous durability and a five-second busy timeout.
Protect the database, WAL and key files with operator-only permissions; the
launcher is not an OS sandbox or secret manager.

The database pins the merchant AID and public key. Restarting with another key
fails closed instead of silently invalidating the verification history. Key
rotation needs a separate authenticated migration procedure; it is not automatic.

## What commits together

- Intent submission: authenticated nonce consumption, immutable intent identity,
  quote/proposal or payout state, and response construction.
- Quote acceptance: inventory deduction, reservation, reference order identity,
  escrow record, original funding instructions and signed initial event.
- Simulated fulfillment: escrow transitions, ordered signed events and settlement
  receipt. Retrying a completed fulfillment returns the original receipt.
- Dispute transition: state and signed event.

Responses are sent only after the SQLite transaction commits. SSE notifications
and local webhook calls are deferred until commit; they are not a durable outbox.
Stored escrow events can be replayed after restart, but live SSE notifications
are process-local and do not fan out across replicas.

Replay protection is shared across processes using the database. Live nonces are
never evicted to admit new work, and durable mode enforces at least 24 hours of
retention. Expiry and capacity settings remain operator configuration. Acceptance
replay verifies the original buyer before returning a committed result, even when
the quote has since expired. New acceptance of an expired quote is rejected.

Proposals and exact demo seller balances also survive restart. The reference
inventory catalog is seeded once; subsequent restarts do not replenish stock.
Reservation IDs and reference escrow/quote hashes use SHA-256. These reference
hashes are not asserted to be the calldata or hash scheme of a deployed contract.

## Verified failure cases

```bash
node --test cli/test/unit/icp-durable-store.test.js
```

The tests cover nonce retention/capacity and rollback, a `SIGKILL` during a
multi-record acceptance, an injected database failure before the acceptance
event commits, HTTP restarts after quote/acceptance/settlement, original-buyer
verification after restart, stable funding instructions, and two separate
merchant processes competing for inventory in the same database.

## Native checkout over HTTP

The source-tree launcher `cli/examples/native-merchant.mjs` connects the same
signed Intent and Quote acceptance endpoints to native Commerce checkout:

```bash
ICP_MERCHANT_KEY_FILE=/protected/merchant.pem \
ICP_MERCHANT_AID=aid:v1:zYourMerchant \
PORT=8787 node cli/examples/native-merchant.mjs --config ./merchant-config.mjs
# Add --apply to enable native order creation.
```

The operator-owned config module exports a function returning
`{ store, native, close? }`. `store` is a durable `SqliteProtocolStore` on a
**separate protocol database**, configured with WAL, full synchronous durability
and a busy timeout. `native` supplies:

- `commerce`: a native Commerce instance with cart fingerprint and stock-policy
  feature support;
- `principal`, `policy`, `storeId`: operator-owned kernel authority;
- `resolveCart(intent)`: a read-only authorization lookup returning
  `{ cartId, budgetId? }`, or `null`. Check the authenticated buyer's ownership
  and delegation before returning an existing cart; never accept an unchecked
  caller-provided cart ID. Cart creation and customer setup happen beforehand;
- `readReceipt(idempotencyKey)`: authoritative lookup in that Commerce store's
  kernel receipt ledger. Return parsed receipts, `null` only for definite
  absence, and throw on lookup failure. Use a native connection owner or a
  separate process; do not open the same commerce file through another bundled
  SQLite library in the engine process.

For example, the module's return value has this shape (the named resources are
created by your operator integration):

```javascript
return {
  store: protocolStore,
  native: {
    commerce,
    principal,
    policy,
    storeId,
    resolveCart: authorizeAndResolveExistingCart,
    readReceipt: lookupAuthoritativeKernelReceipt,
  },
  close: closeOperatorResources,
};
```

The launcher controls `allowApply` exclusively from `--apply`. Without it,
acceptance returns `checkout.status: "preview"`, a kernel verdict, and no order
or funding instructions. Signed quotes, nonces and preview receipts may still
be persisted. Configure principal and agent trust keys as for durable reference
mode; native mode also enforces trust by default.

Quotes take exact prices, currency and totals from `checkoutSnapshot`, check
the requested SKU quantities, maximum total and delegated amount/currency cap,
and sign the cart fingerprint. Quote expiry is bounded by intent, cart and
delegation expiry.
Acceptance requires that saved fingerprint and strict availability for tracked
SKUs. HTTP callers cannot supply kernel policy, principal, budget ID or an
alternate acceptance key. The immutable quote ID defines the retry identity.
Require budgets in the kernel policy and provision them separately.

Without a payment integration, acceptance returns the native order ID, `payment_status: "pending"`,
`checkout.status: "accepted"`, a pending protocol escrow projection, and
`funding: null`. `inventory_reservation` is null because native checkout owns
the allocation; the handler never creates a second reference reservation.
The protocol escrow is not a funded external escrow. Native mode advertises
only `purchase.create`; other verbs, escrow fulfillment, disputes and settlement
co-signing return 503 until governed integrations exist. Health reports
`checkout: "native"` and `settlement: "unconfigured"`.

The order, allocation, budget, outbox and receipt commit in the native
transaction. The protocol escrow and its signed initial event are a subsequent
atomic checkpoint. If that checkpoint fails or the process dies after native
commit, retry signed acceptance for the **same quote**. Receipt lookup recovers
the original order before retrying the checkpoint. An uncertain native outcome
returns HTTP 202 with `checkout.status: "reconciling"`; it is not success and
must be retried with the same quote. Native and protocol commits are not one
distributed transaction. Preserve and back up both stores; automatic cross-store
restore reconciliation remains an operator integration.

The protocol database pins its backend mode as well as the merchant identity.
Restarting a native store through the reference launcher fails closed. Start
with a fresh protocol database when changing backends; there is no automatic
migration of reference orders into native orders.

```bash
node --test cli/test/unit/native-merchant-http.test.js
```

These native-backed tests cover preview, authoritative pricing, changed quoted
terms, original-buyer authentication, two HTTP workers, stock and budget
contention, SIGKILL after native commit, protocol checkpoint rollback and
restart replay. They do not exercise a real payment provider or PostgreSQL.

### Optional Stripe payment recording

`icp-handler/src/stripe-payment.mjs` exports `createStripePaymentLookup` for
read-only reconciliation of an existing Stripe PaymentIntent. Configure it in
the operator module's `native.payments` field:

```javascript
import { createStripePaymentLookup } from './icp-handler/src/stripe-payment.mjs';

const payments = {
  provider: createStripePaymentLookup({
    apiKey: process.env.STRIPE_SECRET_KEY,
    accountId: operatorStripeAccountId,
    apiVersion: operatorPinnedStripeApiVersion,
    connectedAccount: false, // true adds the pinned Stripe-Account header
    livemode: false, // explicit operator choice; test mode by default
  }),
  resolvePayment: async (terms) => {
    // Read-only operator lookup. Return a pre-existing pi_... ID or null.
    return lookupAuthorizedPaymentIntent(terms.cartId, terms.buyer);
  },
};
// Include payments alongside commerce, principal, policy, storeId and readReceipt.
```

This first profile supports **USD, direct card charges and full capture only**.
The payment flow that creates the PaymentIntent must attach these metadata
values before payment: `stateset_cart_id`, `stateset_cart_fingerprint`, and
`stateset_buyer`. They must match the exact accepted native cart snapshot and
authenticated buyer. The resolver must authorize that association; no payment
reference, secret key or provider URL is taken from HTTP acceptance arguments.
API versions and account credentials remain operator configuration.

The adapter checks the authenticated account, test/live mode, currency, exact
minor-unit amount, metadata binding, full amount received and the expanded
latest charge. It rejects partial captures, refunds, disputes, destination
transfers, on-behalf-of charges and unsupported payment methods. This uses
Stripe's [PaymentIntent retrieval](https://docs.stripe.com/api/payment_intents/retrieve),
[Charge fields](https://docs.stripe.com/api/charges/object) and
[connected-account authentication](https://docs.stripe.com/connect/authentication).
Only GET requests are made, with redirects disabled; provider response bodies,
client secrets and card details are not persisted or returned.

With `--apply`, the handler pins the provider identity and payment reference in
its journal, then uses `payments.create` and `payments.complete` kernel commands
to record verified capture. Grant those capabilities and policy rules to the
operator principal. Checkout consumes its configured budget once. Payment
recording does not reuse that checkout budget: it records previously captured
money and issues no new external debit. A policy requiring another budget for
`payments.create` will refuse this initial integration; it is never bypassed.

The optional response field `payment.status` is separate from `checkout.status`:

| Payment status    | Meaning                                                                                                                                               |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pending`         | No mapped PaymentIntent or provider payment still incomplete                                                                                          |
| `reconciling`     | Lookup, binding, evidence or kernel outcome could not be established; retry the same signed quote acceptance                                          |
| `needs_attention` | Provider outcome requires investigation or a governed command was rejected                                                                            |
| `recorded`        | Native payment completion receipt recovered or committed; response includes native `payment_id` and order acceptance reports `payment_status: "paid"` |

A 200 checkout response may still have pending or reconciling payment. Never
fulfill solely because checkout was accepted. The provider reference remains
claimed across retries; it cannot pay two native orders through the same
protocol journal. Use one shared durable journal for the merchant's workers.
Other journals and write entrypoints require separate deduplication controls.

Native payment creation, completion and protocol projection are separate
recoverable commits. The journal keeps immutable commands, and authoritative
native receipt lookup runs before redispatch. Concurrent workers converge on
one payment. If the response or projection is lost after completion, retries
recover the receipt even during a provider outage. Preview performs no provider
lookup, creates no native payment and claims no provider reference.

`recorded` is historical capture evidence. It does not certify Stripe payout,
irrevocability, future refunds/disputes, or funded ICP escrow. Subsequent reversal
and accounting reconciliation, external charge creation, automatic recovery
scheduling and live-provider testing remain open. The initial escrow stays
pending and simulated fulfillment/settlement endpoints remain disabled.

```bash
node --test icp-handler/test/stripe-payment.test.mjs
node --test cli/test/unit/native-merchant-http.test.js
```

Tests use actual native orders/payments and controlled Stripe response fixtures.
They cover concurrency, stock/budget conservation, preview, mismatched provider
evidence, kernel denial, payment-reference reuse, process death after each native
payment step, and protocol checkpoint failure. No Stripe account or funds are
used; these are not live-provider certification results.

## Optional native dispatch recording

The native host can add `fulfillment.resolveDispatch(terms)`, a read-only lookup
of an **operator-approved full or partial dispatch record**. Signed acceptance retries for
the original quote then reconcile that record through `orders.ship_reserved`. HTTP buyers
cannot supply dispatch items, tracking, policy, principal or a new retry key.
The resolver must authorize the association with `terms.orderId` and
`terms.buyer`; return `null` while no dispatch is approved. Terms also include
`quoteId`, exact `amount` and `currency`.

```javascript
native: {
  // Existing commerce, principal, policy, storeId, readReceipt and resolveCart.
  // commerce must expose orders.get(id), including totalAmountExact and version.
  fulfillment: {
    resolveDispatch: async (terms) => dispatchLedger.approvedForOrder(terms.orderId),
  },
}
```

An approved record has this shape; its version and items come from the native
order reviewed by the operator, never from buyer input:

```javascript
{
  reference: 'warehouse:dispatch-123',
  orderId: 'native-order-uuid',
  orderVersion: 3,
  trackingNumber: 'carrier-tracking-reference',
  items: [{ orderItemId: 'native-order-line-uuid', quantity: 2 }],
}
```

Prepare the order through a governed `orders.transition` to `processing` and
record payment before approving dispatch. The bridge requires native payment
status `paid`, matching exact total/currency, and a `processing` or
`partially_shipped` order. Each approved item quantity is **additional units for
this dispatch**, at most the unshipped remainder. A record may name a subset of
order lines. Empty, duplicate, foreign, excessive or stale lines are refused. Grant the configured principal `orders.ship_reserved` in the kernel policy.
The native binary must advertise `orders.ship_reserved.v1` through
`kernelFeatures()` for initial full shipping. Partial or subsequent dispatches
also require `orders.ship_reserved.partial.v1`; a full-only binary refuses those
approvals before creating a claim.
The command pins `expected_version` and exact unit quantity, so a competing
order or payment-status change is checked inside the shipping transaction.
Policies requiring additional authority or a budget still apply and may refuse
the command; the bridge does not borrow the checkout spending budget.

With `--apply`, the protocol journal pins the dispatch reference and immutable
command before execution. One reference can be claimed only once per store in
that shared journal. A new dispatch needs a new reference and the current native
order version. For example, ship one of two units with `quantity: 1`, then approve
the remaining unit under a different reference after reading the resulting order.
The resolver should return the next approved record, or `null`. Returning an
already-used reference for this order only recovers progress; it cannot ship again.

A durable per-order pointer in the configured operator scope selects the current
immutable command. Workers pin
it atomically, so competing approvals cannot replace each other. The current
command must have a verified success receipt before another approval is considered.
At most one new command executes per acceptance request. Receipt lookup must distinguish authoritative absence
(`null`) from unavailable evidence (throw), as with payment reconciliation.
Retries and restarts reuse the same command, including after the kernel commits
but the response or protocol checkpoint is lost. An existing command no longer
requires the dispatch resolver to be online. Preview creates no dispatch claim
or command and invokes no dispatch lookup.

The optional `fulfillment.status` is `pending` without approval, `reconciling`
when lookup or execution is uncertain, `needs_attention` for invalid approval
or kernel rejection, and `recorded` after a verified native shipment receipt.
New recorded results contain `order_id`, `tracking_number`,
`inventory: "reserved_stock_consumed"`, `order_status`, `order_version`, and
cumulative `items: [{ order_item_id, quantity, shipped_quantity }]`. The acceptance
response reports `partially_shipped` until every unit has shipped, then `shipped`.
While a partial shipment is recorded, `next_dispatch_status` can be `pending`,
`reconciling`, or `needs_attention`; known progress remains visible during resolver
or receipt outages and a later command's rejection. A late worker cannot reduce
the recorded order version or shipped progress. Attention at the same version
is retained until a later verified shipment advances the projection.
Results are historical receipt evidence. The escrow remains
pending, funding remains null, and simulated fulfillment/settlement routes stay
disabled. Health adds `fulfillment_reconciliation: "configured"`.

**Inventory boundary:** `orders.ship_reserved` deducts reserved units from both
on-hand and allocated stock, preserving available stock. Partly consumed holds
keep their remaining quantities; fully consumed holds become fulfilled. Remaining
tracked order-line demand must exactly match line-bound open reservations, backed by
sufficient inventory balances. Expired, missing, excess or underallocated holds
are refused. Stock, order quantities, inventory ledger, outbox events and the
idempotent receipt commit together. Untracked SKUs have no stock to deduct.
This integration does not purchase a carrier label, dispatch goods, prove delivery, or release
escrow funds. Native outbox and receipt records provide shipment audit evidence;
the protocol escrow event stream continues to describe only escrow state.
Carrier reconciliation, refunds and disputes remain separate work. See
[kernel execution](kernel-execution.md) for the stock coverage and prior-receipt
requirements. A pinned rejected command is retained for operator reconciliation;
changing the resolver does not silently replace it.

The existing `orders.ship` command retains its reservation-confirmation behavior.
Commands already pinned by an earlier merchant build retain that command and
report `inventory: "depletion_unverified"` when recovered. Upgrading never
silently consumes inventory for a historical shipment. Reconcile those historical
holds separately; use the new capability for newly approved dispatches.

The native HTTP suite covers concurrent workers, process death after shipping,
protocol checkpoint failure, resolver and receipt outages, unpaid orders,
stale/foreign/excess dispatch records, policy denial and a payment-state race.
Partial-shipment tests cover multiple workers, successive approvals, recovery of
a pinned later dispatch without its resolver, and loss of the native response
or protocol checkpoint. Unit tests also interleave competing distinct approvals
and verify that stale projections cannot overwrite newer shipment progress.
Tests use local dispatch fixtures; no carrier service or physical dispatch occurs.

## Native checkout bridge

`icp-handler/src/native-checkout.mjs` exports `NativeMerchantCheckout` for hosts
using the embedded engine. The native HTTP launcher above wraps this bridge;
the reference launcher retains its simulated backend. Its operator-owned constructor accepts `commerce`, a
`SqliteProtocolStore`, `principal`, `storeId`, `policy`, `resolveQuote`,
`readReceipt`, optional `stockPolicy` and `requireCartFingerprint`, and an explicit
`allowApply: true` to enable writes.

Agents supply only `{ quoteId, idempotencyKey }` to `accept()`. The trusted quote
resolver supplies `{ quoteId, cartId, amount, currency, expiresAt, budgetId?, cartFingerprint? }`.
Amounts are decimal strings; currency must be supported by native checkout.
There is no implicit conversion from USDC to USD. Configure the kernel policy
with `requires_budget: true` and provision a scoped economic budget when budget
enforcement is required; a budget is not automatically created.

The bridge journals an immutable command and claims the cart before invoking
`checkout.commit`. The native transaction commits the actual order, inventory
allocation, budget consumption, outbox and execution receipt together. A quoted
total mismatch, insufficient budget, or policy rejection leaves no order or
stock allocation. Payment remains **pending**; acceptance is not settlement.
By default, the bridge permits backorders: it reserves available stock and
records shortages as backorders. To require full availability for tracked SKUs,
the operator sets `stockPolicy: 'reject_if_insufficient'`:

```javascript
const checkout = new NativeMerchantCheckout({
  ...operatorConfiguration,
  stockPolicy: 'reject_if_insufficient',
  allowApply: true,
});
await checkout.accept({ quoteId: 'quote:123', idempotencyKey: 'accept:123' });
```

This option is operator-owned, never an acceptance argument supplied by the agent.

The source-tree kernel now accepts `payload.stock_policy` on `checkout.commit`:
`"reject_if_insufficient"` rejects shortages for tracked inventory SKUs within
the native transaction; `"allow_backorder"` retains the existing behavior.
Omitting the field preserves legacy wire serialization and backorder defaults.
Preview aggregates demand across duplicate-SKU lines. Untracked SKUs retain the
engine's non-inventory behavior; strict mode does not turn them into tracked stock.
This requires rebuilt native bindings. The bridge checks
`commerce.kernelFeatures()` for `checkout.stock_policy.v1` before strict preview
or dispatch. Missing or malformed feature support fails closed; there is no
fallback to backorders. On recovery it checks the saved command's stock policy,
not the new constructor default. A downgraded binary may retrieve a committed
receipt but cannot dispatch an unresolved strict operation. Feature discovery is
a compatibility check on the operator's trusted engine, not remote attestation.

Kernel regression coverage includes two concurrent buyers competing for stock,
duplicate-SKU aggregate demand, preview/apply shortages, unchanged backorder
defaults, and policy changes under an existing idempotency key. Run these with
`cargo test -p stateset-db --test sqlite_kernel_outbox kernel_checkout`.

After a lost response, `resume(id)` reads the authoritative kernel receipt
before retrying. `readReceipt(idempotencyKey)` must query the same kernel ledger
and return `null` only for authoritative absence. Lookup errors return
`reconciling` without dispatch. The journal and kernel commit are separate
checkpoints, not a distributed transaction. Preview can record a native preview
receipt, but creates no order, stock allocation or journal claim.

### Binding quoted terms

With rebuilt native bindings, call `commerce.checkoutSnapshot(cartId)` when
issuing the quote. This read-only method returns `{ cart, fingerprint }` from
the same snapshot, with exact decimal strings and native snake-case field names.
Derive the offered terms from that returned cart and persist the fingerprint
with the immutable quote. **Never recompute the fingerprint at acceptance**:
doing so would authorize terms the buyer did not accept.

Configure `requireCartFingerprint: true` on the bridge and return the saved
`cartFingerprint` from the operator-owned resolver. The kernel compares it inside
the checkout transaction, before economic effects, in both preview and apply.
Same-price SKU substitutions and changes to customer, address, quantity or expiry
are rejected. The versioned commitment covers the complete cart, including
metadata and timestamps; even a harmless timestamp update requires a fresh quote.
Line query order does not affect the commitment. Existing committed operations
replay their original receipt even though checkout itself changed the cart.

The bridge requires `checkout.cart_fingerprint.v1` feature support whenever a
fingerprint is present, including recovery of an unresolved saved command. Old
binaries can retrieve committed receipts but cannot dispatch bound commands.
Enabling `requireCartFingerprint` also blocks dispatch of unresolved legacy
commands without a fingerprint; committed legacy receipts remain recoverable.
Omitting the fingerprint retains legacy behavior for existing callers: only
total, currency and deadline are checked. Production hosts should require it.

The host must still authenticate the buyer and authorize quote access in its
resolver. A cart hash does not authenticate the buyer, merchant or payment rail.
Cart claims
remain bound after rejection, so another acceptance key cannot bypass the
original result. Policy changes require an explicit operator recovery decision.
This bridge does not implement payment capture, refunds or escrow settlement.

Run the native integration regressions with Node 20 or newer:

```bash
node --test cli/test/unit/native-merchant-checkout.test.js
```

## Explicit remaining boundaries

Channel registration is disabled in durable mode until a durable delivery queue
is connected. The default zero-dependency in-memory reference server retains
its existing channel demos. `/healthz` reports the storage mode and identifies
settlement as simulated.

The persisted order is a protocol projection, not a native engine order. Native
aggregate adoption, stock expiration/release, complete fulfillment/refund/dispute
workflows, externally rooted delegation, endpoint authorization, real wallets,
verified rail finality, reconciliation and durable webhook delivery are still
required. Mock funding/release and default seller balances must never be used
to authorize real funds. Do not expose this launcher through a public proxy.

Back up and restore the database consistently with its WAL; copying only a live
`.db` file is insufficient. SQLite coordinates local processes, not independent
database copies or arbitrarily shared network filesystems. PostgreSQL parity,
operational SLOs and independent security review remain release gates.
