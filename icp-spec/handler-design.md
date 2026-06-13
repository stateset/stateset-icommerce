# `stateset-icp-handler` — Sibling Repo Design

This document specifies the design of the `stateset-icp-handler` repository,
a thin protocol-handler that exposes ICP-1.0 over HTTP, MCP, and gRPC
transports. The handler delegates all commerce semantics to a backing
implementation (StateSet's RI by default, but pluggable).

The design mirrors `stateset-acp-handler`: small, single-purpose, easy for
adopters to drop in front of any commerce backend. It is **not** a
full-stack commerce platform.

## Why a separate repo

1. **Adoption surface.** A 5k-LOC handler with one Dockerfile is what a
   merchant integrates. The 250k-LOC engine is what runs underneath. These
   are two different products with different audiences.
2. **Implementation diversity.** Any team with a commerce backend
   (Saleor, Medusa, commercetools, an internal Spring service) can adopt
   ICP by writing a `Backend` adapter against a small Rust trait, then
   wrapping it in this handler. They do not need to fork the engine.
3. **Conformance.** The handler is the conformance-test target. The engine
   is a backend. Decoupling them lets backends compete on commerce
   features while sharing protocol surface.

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│  HTTP / gRPC / MCP transports                                     │
│  (axum-based; thin, no business logic)                           │
└──────────────┬─────────────────────────┬─────────────────────────┘
               │                         │
               ▼                         ▼
┌──────────────────────────┐  ┌─────────────────────────┐
│  ICP wire codec          │  │  Auth & identity        │
│  - canonical JSON (JCS)  │  │  - AID resolution       │
│  - Ed25519 + ML-DSA-65   │  │  - PrincipalBinding     │
│  - replay nonce cache    │  │     verification        │
└──────────────┬───────────┘  └─────────────┬───────────┘
               │                            │
               └────────┬───────────────────┘
                        ▼
            ┌──────────────────────────┐
            │  Backend trait           │     pluggable
            │  ----------------------  │
            │  fn create_intent(...)   │
            │  fn quote_intent(...)    │
            │  fn fund_escrow(...)     │
            │  fn fulfill(...)         │
            │  fn settle(...)          │
            │  fn dispute(...)         │
            └──────────┬───────────────┘
                       │
       ┌───────────────┼─────────────────┬────────────────┐
       ▼               ▼                 ▼                ▼
   StateSet RI    Saleor adapter   Medusa adapter   Custom backend
```

## Crate layout

```
stateset-icp-handler/
├── Cargo.toml                          # workspace
├── crates/
│   ├── icp-handler-core/               # Backend trait, types, errors
│   ├── icp-handler-codec/              # Wire codec (canonical JSON, signing)
│   ├── icp-handler-http/               # axum HTTP binding
│   ├── icp-handler-grpc/               # tonic gRPC binding
│   ├── icp-handler-mcp/                # MCP server binding
│   └── icp-handler-stateset/           # Backend adapter for stateset-icommerce
├── examples/
│   ├── minimal-merchant/               # 200-LOC demo merchant
│   └── two-agent-roundtrip/            # buyer+merchant, end-to-end ICP flow
├── tests/
│   └── conformance/                    # runs icp-conformance against handler
└── docker/
    ├── Dockerfile                      # multi-stage, < 50MB final image
    └── docker-compose.yml              # handler + RI + USDC settler stub
```

## The Backend trait

```rust
// icp-handler-core/src/backend.rs
#[async_trait]
pub trait Backend: Send + Sync + 'static {
    /// Receive a verified ICP Intent, decide whether to quote,
    /// and return a signed Quote (or a typed error).
    async fn quote(
        &self,
        intent: &Intent,
        context: &RequestContext,
    ) -> Result<Quote, BackendError>;

    /// The buyer has accepted the Quote. Reserve inventory, create
    /// internal order, return the EscrowFunding instructions.
    async fn accept(
        &self,
        quote_id: &QuoteId,
        accept: &AcceptEnvelope,
        context: &RequestContext,
    ) -> Result<EscrowFunding, BackendError>;

    /// Fulfillment evidence. Triggers the FUNDED → FULFILLED transition
    /// on the Settler when verified.
    async fn fulfill(
        &self,
        escrow_id: &EscrowId,
        evidence: &FulfillmentEvidence,
        context: &RequestContext,
    ) -> Result<FulfillmentReceipt, BackendError>;

    /// Read-side: subscribe to escrow events for an intent.
    fn observe(
        &self,
        intent_id: &IntentId,
    ) -> BoxStream<'static, EscrowEvent>;

    /// Disputes are routed through here. Backend may invoke its own
    /// arbiter logic, escalate to a human, or call out to a third-party
    /// arbitration service.
    async fn dispute(
        &self,
        escrow_id: &EscrowId,
        dispute: &DisputeIntent,
        context: &RequestContext,
    ) -> Result<DisputeOutcome, BackendError>;
}
```

The trait is intentionally small. Wire-format details, signature
verification, replay-nonce caching, AID resolution — all of that lives in
`icp-handler-codec` and never touches `Backend`. A backend adapter can
be ~500 LOC.

## HTTP surface

```
POST   /icp/v1/intents               # submit signed Intent, get Quote (or error)
POST   /icp/v1/quotes/:id/accept     # submit signed Acceptance
POST   /icp/v1/escrows/:id/fulfill   # submit fulfillment evidence
POST   /icp/v1/escrows/:id/dispute   # submit dispute
GET    /icp/v1/escrows/:id/events    # SSE stream of EscrowEvents
GET    /icp/v1/settlements/:id       # SettlementReceipt by id
GET    /icp/v1/settlers              # this handler's accepted Settler allowlist
GET    /icp/v1/.well-known/icp       # capabilities advertisement
```

All bodies are canonical JSON (`Content-Type: application/icp+json`); the
binary CBOR profile (`application/icp+cbor`) is reserved for icp-1.1.
Signatures are always computed over RFC 8785 JCS per spec §5.1.

## MCP binding

The MCP server exposes the same operations as MCP tools, with the same
type discipline: every tool input and output is a typed JSON object
matching the wire schemas in `icp-spec/schemas/`. An LLM agent that
already speaks MCP can transact ICP commerce without learning a new
protocol — it just calls the tools.

Tools (initial):

- `icp_intent_create` — buyer-side: build, sign, submit Intent
- `icp_quote_review` — buyer-side: examine merchant Quote, decide accept
- `icp_quote_sign` — merchant-side: sign and return a Quote for an Intent
- `icp_escrow_observe` — either side: stream escrow state transitions
- `icp_dispute_open` — either side: open a dispute on an escrow
- `icp_settlement_verify` — either side: verify a SettlementReceipt

These deliberately mirror the HTTP surface so an MCP-only agent has full
protocol parity.

## Conformance hookup

`tests/conformance/` runs `icp-conformance` (forthcoming sibling repo)
against a freshly-spun handler+stub-backend. Green CI = "this handler is
ICP-1.0 conformant." The conformance dashboard pulls the result.

A backend adapter that wants conformance certification runs the same
suite with the real backend wired up.

## Operational properties

- **Stateless handler.** All state lives in the backend or the Settler.
  The handler keeps only an in-memory replay-nonce LRU + an outbound
  EscrowEvent fan-out cache.
- **Horizontal scaling.** Behind any L7 LB. Sticky sessions not required.
- **Graceful drain.** SIGTERM → stop accepting new Intents, drain
  in-flight requests for `--grace` seconds, then exit. Mirrors the
  engine's existing graceful-shutdown pattern.
- **Backpressure.** Per-AID rate limits with token-bucket; configurable
  per-Settler too (since some Settlers have tight TPS limits).
- **Observability.** OpenTelemetry traces with one span per protocol
  step; structured logs with `intent_id`/`escrow_id`/`settlement_id`
  always present. Mirrors `stateset-observability`.

## Default StateSet backend adapter

`crates/icp-handler-stateset/` wires the engine's existing primitives
into `Backend`. Most methods are 10–30 LOC of glue:

```rust
async fn quote(&self, intent: &Intent, ctx: &RequestContext) -> Result<Quote, BackendError> {
    let order_draft = intent.to_order_draft()?;
    let priced = self.engine.pricing().price(&order_draft).await?;
    Quote::sign(&self.signing_key, intent.intent_id(), priced, ctx)
}
```

This adapter ships with the handler crate so the default deployment is
"`docker run stateset/icp-handler` and you're conformant."

## Build, version, ship

- Rust 1.85, edition 2024 (matches engine).
- Workspace lints inherited from engine workspace.
- Cargo features: `transport-http` (default), `transport-grpc`,
  `transport-mcp`, `backend-stateset` (default), `pqc-hybrid`.
- Released on a single track with the spec: `stateset-icp-handler` v0.1
  alongside ICP-1.0-DRAFT, v1.0 on ICP-1.0 Final.
- Docker image published to `ghcr.io/stateset/icp-handler` with reproducible
  builds (`cosign` signed, SBOM published).

## Out of scope

- KYC/KYB, AML, Travel Rule (these belong in a separate compliance
  middleware crate, mounted in front of the handler).
- Settler implementations (Circle/Stripe/etc adapters live in their own
  repos to keep the handler lean).
- Discovery and Agent Cards (AP2's Agent Cards or A2A's directory).
- Reputation calculations (defer to ICP-1.1).

## Implementation note for whoever builds this

Most of what's needed already exists in the engine:

| Need | Source |
|---|---|
| Ed25519 + ML-DSA-65 signing | `crates/stateset-crypto/src/sign.rs`, `pqc.rs` |
| Canonical JSON (RFC 8785 JCS) | `crates/stateset-crypto/src/canonicalize.rs` |
| Envelope/signature framing | `crates/stateset-sync/src/http_transport.rs` (`VesEventEnvelope`) |
| Escrow state machine | `crates/stateset-a2a/src/escrow/` |
| Dispute logic | `crates/stateset-a2a/src/disputes/`, `dispute_rules.rs` |
| Replay nonce cache | extract from `crates/stateset-sync/` |

Roughly 5,000–8,000 LOC of new Rust, plus glue. Two engineers, eight weeks.
