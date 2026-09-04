# AI commerce kernel execution

StateSet's domain repositories remain the embedded commerce engine. The AI
commerce kernel contract adds a uniform control plane around them: a versioned
command enters, policy and concurrency guards run, the domain mutation and its
event commit atomically, and a machine-readable receipt leaves.

## Contract

`stateset_core::CommandEnvelope<T>` carries the fields that must not be hidden
in prompts or process-local context:

- command identity and a required idempotency key;
- authenticated principal, tenant, delegation, and capabilities;
- store, correlation, causation, trace, and deadline context;
- expected aggregate version and expected policy version;
- approval and cryptographic authority evidence;
- an optional economic mandate (objective, issuer, subject, scope, validity);
- an optional exact resource commitment (budget, fiat money or non-fiat asset,
  quantity, counterparty, and supporting evidence);
- an explicit `preview` or `apply` execution mode.

Constructors default to `preview`. Runtimes must never infer `apply` from model
text. They should validate the envelope, authenticate the principal, evaluate
policy, verify approval and optimistic version, then invoke the domain command.

`KernelPolicy` is a versioned, deny-by-default command allow-list. Each command
rule declares required capabilities and whether approval evidence is mandatory.
Evaluation also rejects policy-version mismatch, approval scope mismatch, and
expired approval. The resulting decision ID, policy version, and stable reason
codes are copied into the receipt.

Command rules require tenant and store scope by default. Agent principals must
also identify their delegating principal. When approval is required, its
evidence is bound to the command type, tenant, store, and idempotency key; it
cannot be replayed for another merchant or semantic intent.

High-assurance deployments can add `with_signed_authority()` to a command rule
and register trusted Ed25519 keys on the policy. The authority signs an RFC 8785
canonical SHA-256 digest of the complete semantic intent, including principal,
tenant, store, payload, versions, deadline, and approval. Changing any bound
field invalidates the signature; key ID, issuer, and validity-window failures
produce stable policy reason codes.

## Bounded economic autonomy

`EconomicAgent` is the first-class operator-owned identity for an autonomous
actor. It binds an agent ID to its delegating principal, organizational role,
tenant, store, capabilities, budget IDs, credentials, and optional Ed25519
public key. `agent.command(...)` produces a correctly scoped, delegated kernel
envelope; the identity document itself stays outside model arguments.

`EconomicAuthority` productizes command policy as three explicit tiers. Each
rule names an autonomous ceiling, a higher approval ceiling, and therefore an
implicit deny range above it. `compile(agent)` verifies that the agent actually
holds the named capability and emits a deny-by-default `KernelPolicy` scoped to
that agent's tenant and store. Tiers may use exact fiat money or exact asset
amounts, but never mix denominations.

```rust,ignore
let agent = EconomicAgent::new(
    "agent:acme:procurement:7",
    "company:acme",
    "procurement",
    "tenant:acme",
    "store:production",
)
.with_capabilities(["payments.create"])
.with_budgets(["budget:procurement:monthly"]);

let authority = EconomicAuthority::new("procurement-v4").allow(
    "payments.create",
    EconomicAuthorityRule::money(
        "payments.create",
        Money::new(dec!(2500.00), CurrencyCode::USD),
        Money::new(dec!(25000.00), CurrencyCode::USD),
    )
    .with_budget(),
);
let policy = authority.compile(&agent)?;
```

Complete operator-owned JSON documents are available in
[`economic-agent.json`](../../kernel/examples/economic-agent.json) and
[`economic-authority.json`](../../kernel/examples/economic-authority.json).
The all-zero example key is deliberately non-production material.

The public intent vocabulary is deliberately smaller than the domain catalog.
`commerce.transactions(agent)` exposes `quote`, `buy`, `sell`, `pay`,
`fulfill`, `return_order`, `refund`, and `subscribe`. These methods create
framework-neutral `EconomicIntent` values carrying identity, scope,
idempotency, exact commitments, and protocol/domain payloads. Adapters map that
stable vocabulary onto ICP and governed domain commands; hundreds of lower-level
tools remain implementation detail.

`EconomicReceipt::from_execution(...)` projects a domain receipt into a compact
artifact containing the agent, principal, intent, exact commitment, policy
decision, canonical result hash, audit-chain hash, and optional settlement
evidence. Agent, merchant, and settler can independently Ed25519 co-sign the
same RFC 8785/SHA-256 digest. Verification uses a caller-owned trusted-key map;
embedded key material is never accepted as its own trust proof. Any change to
the result hash, transaction ID, amount, policy decision, or audit anchor
invalidates every signature.

`EconomicMandate` turns an objective into durable, machine-checkable command
context. It binds the objective to its issuing principal, the agent that may
pursue it, allowed command names, tenant/store boundaries, and a validity
window. `KernelCommandPolicy::with_mandate()` makes this context mandatory.
Policy rejects subject or issuer substitution, cross-tenant/store reuse,
commands outside the mandate, future mandates, and expired mandates.

`EconomicCommitment` declares what the command will place at risk:

```json
{
  "budget_id": "budget:cx-daily",
  "amount": { "amount": "149.00", "currency": "USD" },
  "counterparty_id": "customer:1837",
  "quantity": "1",
  "evidence": ["ticket:991", "return:rma-42"]
}
```

Non-fiat and chain-qualified assets use a separate exact contract rather than
pretending token symbols are ISO currencies:

```json
{
  "asset_amount": {
    "amount": "25.125",
    "asset": "eip155:8453/erc20:0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"
  }
}
```

Asset amounts are decimal strings and asset identifiers are trimmed,
case-sensitive values of at most 256 bytes. A commitment cannot contain both
`amount` and `asset_amount`, and an asset commitment cannot name a fiat budget.
Command rules can set `max_asset_amount` and `approval_above_asset`; both bind
the amount and exact asset identifier. A2A escrow creation and funding bind
these declarations to executor-observed escrow state before custody changes.
The escrow model canonicalizes bare symbols to uppercase, so those commitments
use forms such as `USDC`; chain-qualified identifiers and checksum-sensitive
addresses are preserved byte-for-byte.

Asset authority on other commands fails closed with
`policy.asset_binding_unsupported`. In particular, `x402.settle` records a
transfer that already occurred externally, so enforcing a newly declared limit
there would be after-the-fact accounting rather than authorization. Durable
asset budgets are intentionally deferred until the kernel has reservation,
release, expiry, refund, and final-settlement semantics; a permanent debit at
escrow funding would strand authority when custody later unwinds.

## Sequencer-to-kernel marketplace execution

The sequencer is an ordered transport, not an authority oracle.
`@stateset/cli/marketplace` provides `KernelMarketplaceBridge`, which consumes
signed `marketplace.award.created` events and derives governed commands from
local configuration. Buyer and merchant workers remain separate economic
actors: the buyer creates escrow while the winning merchant reserves inventory,
each under its own identity, principal delegation, capabilities, and policy.

The bridge maintains a durable SQLite inbox and cursor. It authenticates the
message against an operator-owned agent registry, enforces tenant/store scope,
and rejects planner output that changes the local principal. Command IDs and
idempotency keys are deterministically derived from the sequencer event, so a
crash between kernel execution and receipt publication safely replays the
sealed receipt instead of executing commerce twice. The cursor advances only
after terminal handling.

Marketplace signatures bind the complete canonical message, including the
award, accepted bid, counterparty, exact money and settlement asset, quantity,
addresses, expiry, and reply chain. Transport-level VES signatures should also
be enabled in production; application and transport signatures protect
different boundaries.

Command policy can apply a per-transaction `max_amount`, require approval only
above `approval_above`, cap inventory or capacity with `max_quantity`, and
evaluate declared `allowed_counterparty_ids`.
Currency mismatches fail closed and amounts remain decimal strings end to end.
Checkout, payment, refund, and subscription-charge executors on both SQLite
and PostgreSQL additionally bind the declared amount to the exact domain amount
before preview or apply. This stops an agent from obtaining authorization for
`$10.00` while placing `$100.00` in the payload. Refund binding occurs after
the payment is locked and the actual refundable amount is resolved;
subscription binding uses the locked billing-cycle total. Checkout preview
re-derives coupons and automatic promotions, then binds the resulting order
total. Apply performs the same comparison inside its checkout savepoint, so a
mismatch rolls back the order and inventory reservations. A completed cart
can only replay through its original kernel idempotency key; a different
economic command is rejected rather than receiving success or another debit.

`with_budget()` additionally requires a named, operator-provisioned durable
budget. SQLite and PostgreSQL lock its balance, verify principal, tenant, store,
currency, and validity window, then record exactly one debit per idempotency key
in the same transaction as the domain mutation and receipt. Preview performs
the complete balance check without debiting. Concurrent commands cannot spend
the same remaining balance, failed commands roll the debit back, and replaying
a successful command does not debit twice. `economic_budget_status` exposes
exact committed and available decimal-string balances.

Budget debits currently bind to `checkout.commit`, `payments.create`,
`payments.create_refund`, and `subscriptions.charge`. A policy that configures `with_budget()`,
`max_amount`, or `approval_above` for another command fails closed with
`policy.money_binding_unsupported` until that command has a domain-specific
observed-amount binding. A signed declaration alone is not treated as proof of
what the underlying mutation commits.

The same distinction applies to counterparties. Payments, refunds, and
subscription charges map their customer to the canonical economic identity
`customer:<uuid>` and compare it with the signed declaration. A mismatch or an
unresolvable declared target is rejected before mutation. Other executor
policies with a non-empty `allowed_counterparty_ids` reject with
`policy.counterparty_binding_unsupported` until they define an equivalent
observed identity mapping.

Provision budgets from trusted operator code, never from a model-facing tool:

```javascript
await commerce.provisionEconomicBudget({
  budget_id: 'budget:procurement:2026-09',
  principal_id: 'agent:procurement',
  tenant_id: 'tenant:acme',
  store_id: 'store:production',
  limit: { amount: '100000.00', currency: 'USD' },
  valid_from: '2026-09-01T00:00:00Z',
  expires_at: '2026-10-01T00:00:00Z',
});
```

Provisioning the identical definition is idempotent, which makes deployment
restarts safe. Reusing an ID with a changed principal, scope, limit, currency,
or validity window is rejected; budget authority cannot be reset by replacing
configuration. Native agent toolkits accept trusted `mandate` and `commitment`
values (or callbacks) through execution options before approval and authority
signing occur.

Every newly produced `ExecutionReceipt` contains `economic_context`: the
authenticated principal and delegation, store and correlation ID, mandate,
commitment, approval ID, and signed-authority issuer. That context is sealed in
the append-only receipt audit chain, producing a portable economic receipt
rather than an opaque success message.

To produce the signature, first attach `AuthorityEvidence` with its issuer, key
ID, issue time, expiry, and an empty signature. Compute
`authority_signing_hash`, sign that digest, then replace the empty signature
with its hex-encoded Ed25519 value. The authority metadata is itself in the
signed preimage, so a bearer cannot extend the expiry or substitute an issuer.

`ExecutionReceipt<T>` is the stable response contract. It includes structured
status and retry guidance, affected aggregate/version, committed event IDs,
policy evidence, portable economic context, and an optional audit hash. Agents
should branch on status, error codes, and retry disposition—not error prose.

The current additive wire version is `1.0`. Existing direct repository APIs are
unchanged; adapters can adopt the envelope incrementally.

## Deploying the strict MCP profile

The stdio and HTTP binaries load identity and policy only from
operator-controlled files or environment variables:

```bash
stateset-mcp --db ./store.db --apply \
  --kernel-policy ./kernel-policy.json \
  --kernel-principal ./kernel-principal.json \
  --kernel-store-id store:production
```

The equivalent environment variables are `STATESET_KERNEL_POLICY`,
`STATESET_KERNEL_PRINCIPAL`, and `STATESET_KERNEL_STORE_ID`. Supplying only part
of the configuration aborts startup. Policy and principal never enter the tool
schema, so an agent cannot replace its identity, capabilities, tenant, store,
or trusted keys. Strict mode is automatic when these inputs are configured.
For durable stdio or HTTP stores, `--apply` fails closed unless this complete
trusted profile is present. `--kernel-allow-legacy-writes` is the only explicit
CLI escape hatch; the ephemeral HTTP demo remains writable without configuring
production authority because its private temporary database is discarded.

Example principal file:

```json
{
  "id": "agent:checkout",
  "kind": "agent",
  "tenant_id": "tenant:acme",
  "delegated_by": "user:operator",
  "capabilities": ["checkout.commit", "payments.create"]
}
```

`--kernel-allow-legacy-writes` deliberately disables strict exposure for a
controlled migration. It should never be used on an autonomous-agent endpoint.
Starting templates are available in
[`kernel/examples/strict-policy.json`](../../kernel/examples/strict-policy.json)
and
[`kernel/examples/strict-principal.json`](../../kernel/examples/strict-principal.json);
replace every placeholder and narrow capabilities before deployment.
[`kernel/examples/bounded-economic-policy.json`](../../kernel/examples/bounded-economic-policy.json)
shows a signed procurement-style rule with a `$25,000.00` hard ceiling and
human approval above `$2,500.00`; its all-zero public key is intentionally a
non-production placeholder.

## Exact money

Agent-facing JSON uses `MoneyWire`:

```json
{ "amount": "9007199254740993.25", "currency": "USD" }
```

The amount is a base-10 string, never a JSON number. Conversion to `Money`
parses directly into `Decimal` and rejects significant fractional precision
beyond the currency's minor units. Payment and refund repository boundaries
enforce the same scale rule for both SQLite and PostgreSQL.

Existing JavaScript/Python numeric APIs remain available for compatibility,
but new agent protocols should use `payments.createExact` /
`payments.createRefundExact` in Node.js and `payments.create_exact` /
`payments.create_refund_exact` in Python. Payment and refund results expose
`amountExact` / `amount_exact` alongside the legacy numeric field. Numeric
bindings should be deprecated in a major-version migration rather than changed
silently. MCP payment/refund tools accept decimal strings and prefer these
exact methods while retaining a legacy-number compatibility path.

## Transactional facts

The `kernel_outbox` table holds durable events. Payment and refund creation,
inventory reservation lifecycle changes, order transitions, return lifecycle
changes, and ledger posting now insert versioned events in the same transaction
as their domain mutation on both SQLite and PostgreSQL. If either write fails,
neither commits. Money and quantity fields in event payloads are decimal
strings.

Return transitions are checked under a database transaction. SQLite takes an
immediate write transaction and PostgreSQL locks the return row; both enforce
the same state machine and optimistic version check before writing the return
and its event. Invalid or stale transitions therefore produce neither a state
change nor an outbox fact.

Consumers read unpublished rows in `(created_at, id)` order, deliver them at
least once, then set `published_at`. Delivery handlers must deduplicate by event
ID. Failures increment `attempts` and record `last_error`.

Production consumers should use `claim_pending` / `claim_pending_async` rather
than the compatibility `pending` reader. Claims atomically lease due events to a
named worker; PostgreSQL uses row locks with `SKIP LOCKED` so replicas can
compete safely. Lease-owned acknowledgement prevents one worker from completing
another's delivery. Failed deliveries clear the lease and set `next_attempt_at`;
once the configured attempt limit is reached, `dead_lettered_at` removes the
event from normal delivery without deleting its audit history.
Operators can inspect dead letters, explicitly redrive them with optional
attempt reset, and export `delivery_health` counters for ready, leased, delayed,
dead-lettered, and published events.

The `kernel_receipts` table reserves durable idempotency and receipt storage for
envelope-aware executors. Receipts are stored under both `command_id` and
`idempotency_key`. The key is bound to a SHA-256 hash of the semantic request;
the invocation ID, issue time, and preview/apply mode are excluded. A genuine
retry can therefore use a new invocation ID, and an authorized apply can
atomically promote its stored preview. Changes to command type, principal,
mandate, commitment, authority evidence (including its signature and validity
window), deadline, or payload produce `kernel.idempotency_conflict`. SQLite and PostgreSQL call the
same RFC 8785 canonical hashing implementation for this contract.

`SqliteDatabase::kernel_executor(policy)` now executes checkout commit, payment
creation, refund creation, inventory reservation, reservation
confirmation/release, order transitions, full or partial shipment, return
transitions, journal posting, and x402 settlement;
PostgreSQL exposes the corresponding asynchronous methods. Preview mode validates
current domain state and persists a non-mutating receipt. For a refund this
includes the payment's refundable status, currency precision, captured balance,
and all in-flight refund reservations. For inventory it includes effective
availability after expired holds, reservation state, and the current
inventory-balance version.

The governed inventory commands are `inventory.reserve`,
`inventory.reservation.confirm`, and `inventory.reservation.release`.

`products.create` atomically creates a draft product and all supplied variants
from one authorized intent. Variant prices, compare-at prices, and costs remain
exact decimals; slug and SKU uniqueness are checked under the database write
lock. PostgreSQL additionally takes deterministic advisory locks for the slug
and sorted SKU set, so concurrent commands with different retry keys converge
on one product and durable conflict receipts instead of leaking a raw unique
constraint failure. Preview performs the same semantic checks without creating
a catalog row, while apply commits the product, variants,
`products.created.v1` fact, and sealed receipt together.

`inventory.item.create` atomically creates the SKU master, its location balance,
an initial `receipt` inventory transaction when stock is positive, the
`inventory.item.created.v1` outbox fact, and the sealed command receipt. All
quantities are decimal strings at agent boundaries and unbounded `NUMERIC` in
PostgreSQL, preserving fractional and large quantities without float coercion.
Distinct command keys racing for the same SKU converge on one success and
durable, auditable conflict receipts.

Apply mode commits the payment, refund, reservation mutation, its causally
enriched outbox event, and the success receipt in one database transaction.
Reservation receipts expose the balance version before and after the command,
and honor `expected_version` with `after_conflict` retry guidance. Confirmation
may cover the full reservation or an exact decimal quantity. A partial
confirmation creates a confirmed split and keeps the unfulfilled remainder open;
the receipt identifies the confirmed split. Confirmation does not change the
allocated balance, while release restores availability and increments its
version. Replaying a completed lifecycle command produces neither a second split
nor a duplicate fact.

`orders.transition` applies the non-shipment order state machine under an order-version
check. It supports confirmation, processing, delivery, refund, and cancellation;
cancellation releases linked inventory reservations and backorders in the same
transaction. Shipment targets are rejected with
`commerce.shipment_command_required` because fulfillment needs the separate
line-aware `orders.ship` contract. Transition receipts identify the order
version before and after the mutation, and the committed `orders.updated.v1`
fact carries the authenticated command context.

`orders.ship` accepts either all remaining units or explicit positive quantities
per order line. Preview resolves the resulting `partially_shipped` or `shipped`
state and validates line ownership, remaining quantities, reservation expiry,
policy, and order version without mutation. Apply confirms the exact inventory
reservation portions, increments line shipment quantities, records tracking,
updates the order, and commits every inventory and order fact plus one receipt
atomically. Every caused fact is listed in the receipt and inherits the command's
principal, correlation, and causation context.

`returns.transition` applies the return state machine under the same governed
boundary. Preview reports the current return and predicted version; apply locks
the return, checks `expected_version`, commits `returns.updated.v1` with exact
string refund amounts, and stores the success receipt atomically. Missing,
stale, and invalid transitions become durable non-mutating rejection receipts.

`ledger.post` verifies authority and policy before atomically posting a balanced
draft journal entry, updating exact-decimal account balances, and recording its
receipt and causal event. Direct repository posting remains a trusted internal
primitive; autonomous callers should use the kernel executor.

Every persisted outcome is also appended to `kernel_receipt_audit_log`. Each
entry commits to the prior hash, semantic request hash, and RFC 8785-canonical
receipt using SHA-256. `verify_audit_chain` (or PostgreSQL's async equivalent)
recomputes the chain and identifies the first altered entry. PostgreSQL takes a
transaction-scoped chain lock so concurrent commands cannot create forks.

`audit_checkpoint` exports a portable, RFC 8785-canonical checkpoint containing
the entry count, chain head, generation time, and its own SHA-256 digest. Publish
that JSON outside the commerce database—such as to an append-only transparency
log, immutable object store, ledger, or notarization service. Later,
`verify_audit_checkpoint` proves the retained checkpoint still names the exact
historical chain head at the checkpoint's declared entry sequence, even after
newer receipts have been appended. A valid hash copied from another position in
the chain is rejected, as is altering either the checkpoint or any earlier
local receipt.

`x402.settle` accepts only sequenced intents and atomically binds the confirmed
transaction hash and block number to the intent, causal event, receipt, and
audit chain. Retries return the original anchored settlement receipt.

`checkout.commit` runs customer resolution, order and line creation, inventory
reservation/backorder decisions, order confirmation, cart completion, causal
event emission, and receipt sealing in one transaction on both databases. Its
preview validates cart readiness, exact money, addresses, and stock without
allocating an order number or writing domain state. The command deliberately
leaves payment pending; only a separate governed payment
or settlement fact may mark commerce paid.

`subscriptions.charge` is a governed collection request. It locks an eligible
scheduled or retryable billing cycle, verifies the subscription is active or
past due, validates the exact positive cycle amount, creates one pending
payment, links the cycle in `processing`, and commits
`subscriptions.charge_requested.v1` plus its receipt atomically. It never
reports an external processor success; payment outcome handling is separate.

The governed A2A custody lifecycle is `a2a.escrow.create`,
`a2a.escrow.fund`, `a2a.escrow.dispute`, `a2a.escrow.refund`, and
`a2a.escrow.release`. Creation
accepts an exact decimal amount, explicit expiry, participants, network, asset,
and release conditions. Funding is allowed once from `created`, refund is
allowed only from refundable states. Dispute atomically freezes funded value
and records the reason, category, time, and authenticated principal in the
escrow metadata and event. Every transition is serialized under a database
lock. Preview receipts project state without changing custody;
apply commits the state, versioned event, and sealed receipt atomically.

Formal resolution uses `a2a.dispute.file`,
`a2a.dispute.evidence.submit`, and `a2a.dispute.resolve`. Filing binds the case
to the command tenant and store, derives the respondent from the escrow, proves
that the authenticated principal or delegator controls the claimant address,
and freezes custody in the same transaction. Evidence is immutable,
content-addressed with SHA-256, size bounded, and accepted only from a bound
participant before the evidence deadline. Resolution requires resolver policy
authority and atomically updates both case and escrow. Full refund and seller
release derive their allocations; split resolution requires explicit,
non-negative exact-decimal buyer and seller amounts whose sum equals escrow
value. Escalation leaves funds frozen. Every outcome is receipt-sealed and
emits causally linked dispute and custody events.

`a2a.escrow.release` additionally evaluates the embedded escrow state under a database lock.
Only funded or active, unexpired escrows may release, and every stored
seller-fulfilled quote, buyer confirmation, elapsed time lock, milestone, or
explicit custom condition must pass. Apply commits the released state,
`a2a.escrow_released.v1` settlement instruction, and sealed receipt together;
unmet or expired conditions become durable non-mutating rejections.

The MCP A2A runtime and native executor use the same SQLite file. Its richer
quote and agent-card projections have distinct table names, while
`a2a_escrows` is intentionally shared. Consequently an escrow created or funded
through an A2A tool is the same aggregate released by the governed command—no
shadow-store synchronization or dual-write window exists. The HTTP server's
ephemeral mode uses a private temporary SQLite file rather than two unrelated
literal `:memory:` connections, preserving this invariant without making the
demo store durable.

When a trusted `kernel` configuration is supplied to the Node toolkit or MCP
server, strict exposure is the default: read tools remain discoverable, but a
mutation is advertised and executable only when it maps to a typed command in
the governed catalog. The one explicit governed composite,
`agentic_execute_plan`, is classified as `write`; it remains available for
multi-step automation, but every nested live write crosses the same typed
command executor and an unmapped step fails closed before its handler runs.
Mutation classification is fail closed: `write`,
`delete`, `admin`, missing, and any future permission class are mutating; only
an explicit `read` permission bypasses the command boundary. This closes direct
calls, plans, rollback execution, and MCP transport over the same boundary.
`kernel.strict=false` is an explicit legacy-migration escape hatch; it should
not be enabled for autonomous agents.

[`kernel/mutation-boundary.json`](../../kernel/mutation-boundary.json) is a
generated classification of the complete MCP registry. It records every tool,
permission, governed command mapping or blocked disposition, aggregate counts,
and a SHA-256 digest. The kernel CI and release-hygiene gates derive the report
again from the live registry and fail if it changes. A newly added mutation can
therefore never silently become available to a strict autonomous endpoint.

SQLite serializes writers with an immediate transaction. PostgreSQL uses a
transaction-scoped advisory lock per idempotency key, plus a row lock on the
payment during refund validation. Concurrent refunds therefore cannot reserve
more than the captured balance, while concurrent inventory commands serialize
on the balance row and cannot oversell stock. Policy denials, missing entities,
invalid money or quantities, over-refunds, insufficient stock, and version
conflicts are durable rejection receipts and never create a domain row or event.

## Remaining adoption work

1. Move every mutation currently classified `blocked` into a typed executor
   command (or remove it from autonomous exposure); Node and Python agent
   adapters already expose the governed boundary for the critical catalog.
2. Add transactional outbox events to remaining lower-risk mutation paths, prioritizing
   subscriptions and finance automation.
3. Expand multi-process crash-recovery and policy-version compatibility
   suites beyond the current release-gated critical command catalog.
4. Extend durable budget binding beyond payment/refund commands, then add
   explicit release, settlement, and period-rollover semantics for long-lived
   reservations.

This preserves the embedded, single-file deployment model while making safety,
authority, causality, and retries deterministic enough for autonomous agents.
