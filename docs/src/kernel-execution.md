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
- approval evidence;
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

To produce the signature, first attach `AuthorityEvidence` with its issuer, key
ID, issue time, expiry, and an empty signature. Compute
`authority_signing_hash`, sign that digest, then replace the empty signature
with its hex-encoded Ed25519 value. The authority metadata is itself in the
signed preimage, so a bearer cannot extend the expiry or substitute an issuer.

`ExecutionReceipt<T>` is the stable response contract. It includes structured
status and retry guidance, affected aggregate/version, committed event IDs,
policy evidence, and an optional audit hash. Agents should branch on status,
error codes, and retry disposition—not error prose.

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
authority evidence (including its signature and validity window), deadline, or
payload produce `kernel.idempotency_conflict`. SQLite and PostgreSQL call the
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

This preserves the embedded, single-file deployment model while making safety,
authority, causality, and retries deterministic enough for autonomous agents.
