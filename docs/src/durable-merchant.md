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

## Native checkout bridge (separate integration)

`icp-handler/src/native-checkout.mjs` exports `NativeMerchantCheckout` for hosts
using the embedded engine. This is **not yet connected to the reference HTTP
launcher**. Its operator-owned constructor accepts `commerce`, a
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
