# stateset-http

[![crates.io](https://img.shields.io/crates/v/stateset-http.svg)](https://crates.io/crates/stateset-http)
[![docs.rs](https://docs.rs/stateset-http/badge.svg)](https://docs.rs/stateset-http)

Turns an embedded commerce engine into a real HTTP API. REST endpoints plus
Server-Sent Events over [`axum`], with pagination, CORS, bearer auth, request-ID
tracing, and structured error responses.

Use this when the engine needs to serve clients it doesn't share a process with. If
you're embedding in your own Rust binary, you don't need it — talk to
[`stateset-embedded`](https://crates.io/crates/stateset-embedded) directly.

## Usage

```rust,no_run
use stateset_embedded::Commerce;
use stateset_http::ServerBuilder;
use std::net::SocketAddr;

# #[tokio::main]
# async fn main() -> Result<(), Box<dyn std::error::Error>> {
let commerce = Commerce::new(":memory:")?;
let addr: SocketAddr = "127.0.0.1:3000".parse()?;

ServerBuilder::new_from_env(commerce)?
    .bind(addr)
    .with_cors()
    .with_request_id()
    .with_bearer_auth("replace-me-with-a-secret")
    .with_ignore_tenant_header() // Explicit single-store deployment.
    .serve()
    .await?;
# Ok(())
# }
```

## Architecture

```text
┌────────────────────────────────────────────────┐
│                  HTTP Client                   │
│  ┌──────────────────────────────────────────┐  │
│  │         axum Router (this crate)         │  │
│  │  ┌────────────────────────────────────┐  │  │
│  │  │  stateset-embedded (Commerce)      │  │  │
│  │  │  ┌──────────────────────────────┐  │  │  │
│  │  │  │  SQLite / PostgreSQL         │  │  │  │
│  │  │  └──────────────────────────────┘  │  │  │
│  │  └────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────┘  │
└────────────────────────────────────────────────┘
```

## What You Get

- **REST endpoints** across the commerce domains — orders, customers, products,
  inventory, returns, payments, shipments, and the back-office surfaces
- **Server-Sent Events** for live order and inventory changes
- **Cursor pagination** with consistent envelope shapes
- **Structured errors** — typed JSON bodies, not bare status codes
- **Bearer auth** by default, with CORS and request-ID propagation as opt-in layers
- **OpenAPI** description generated from the route table

## Security Defaults

The builder creates a random bearer token by default. Call
`bearer_auth_token()` before `serve()` if you need that token for a local
development client; startup logs do not reveal the full value. A non-loopback
bind requires an explicit operator-owned token set with `with_bearer_auth`
or an actor/tenant-bound variant. Public startup also requires fail-closed
API authorization via `with_authz_engine` and rate limiting via
`with_rate_limit`. If a trusted gateway supplies both controls instead, call
`with_trusted_gateway_controls()` explicitly; the gateway must authenticate
actors, enforce permissions, strip client-supplied actor/forwarding headers,
and throttle traffic before forwarding it. Public binds cannot trust
`x-actor-id` or forwarded client IP headers without that declaration.
`/metrics` has a separate bearer token: `with_bearer_auth` only replaces the
API token. Configure `with_metrics_bearer_auth` with an operator-owned scrape
token before serving; keep that credential separate from the write-capable API
token. Without it, `/metrics` retains its generated default token. See the
[deployment guide](https://github.com/stateset/stateset-icommerce/blob/master/docs/src/advanced/deployment.md).

For shared hosting, configure `with_tenant_db_dir` and bind tokens to tenants
and actors. API requests include `x-tenant-id`; in tenant-bound deployments it
must match the operator-owned token binding. The single-store example explicitly
disables tenant-header routing while retaining the required request header.

## Durable request retries

With tenant database routing, reservations and completed responses are stored
in the authenticated tenant's database alongside its business records. They
survive server restarts and tenant backup/restore even when the default store
is in memory. Authentication and authorization run before response replay.

The server reserves each `(tenant, Idempotency-Key)` in the database before
executing a guarded POST or shipment-item DELETE. Replicas sharing that database
cannot execute the same reserved request twice. Completed responses replay for
24 hours; reusing a key
with a different request returns 422. An unavailable idempotency store returns
503 before execution. Clients must keep the same key when retrying. The request
fingerprint includes the method, path, exact query string and body bytes.

A request interrupted after reservation can have an uncertain outcome, including
when the mutation committed but its response could not be saved. Its reservation
does not expire: retries return 409 with `idempotency_in_progress`. This is
at-most-once execution within the retention window, not automatic crash recovery.
For reconciliation, stop the original worker, inspect the domain records and
audit trail, and locate the matching `http_idempotency_keys` row with
`response_status = 0`. An operator may complete it through
`HttpIdempotencyRepository::complete` using the original fingerprint and the
verified response. Delete an unresolved reservation only after proving that no
mutation occurred and no original worker can still commit. Do not bypass an
uncertain outcome by issuing a new key.

## Shipment lifecycle

Create shipments with `POST /api/v1/shipments`, including the recipient name,
shipping address and shipping method. The response includes `version` for
optimistic concurrency. Use `PATCH /api/v1/shipments/{id}` to apply a partial
update, for example:

```json
{ "status": "processing", "expected_version": 1 }
```

The normal sequence is `pending → processing → ready_to_ship → shipped →
in_transit → out_for_delivery → delivered`. Invalid transitions return 422;
stale versions return 409. Omitted or null fields retain their stored values.
An unchanged patch with a satisfied version precondition preserves timestamps
and version. Monetary patch fields such as `shipping_cost` require decimal
strings. Native updates and their outbox facts commit together.

Cancellation uses `POST /api/v1/shipments/{id}/cancel` with `{}` or an
`expected_version` precondition. It requires delete permission when authorization
is enabled. The general update route rejects cancellation, including spelling
aliases. Cancellation retains the shipment and its history and is permitted
only before carrier handoff.

Shipment creation accepts an optional `items` array. Each item has `sku`, `name`,
and a positive integer `quantity` (at most 2,147,483,647), plus optional
`order_item_id` and `product_id` UUIDs. A missing order-line ID is resolved only
when the SKU identifies one line. The native transaction validates order
membership, product/SKU consistency, and the total assigned to non-cancelled
shipments. Shipment creation, detail and list responses include the normalized
`items` with their shipment-item IDs.

Packing edits use these endpoints:

| Method | Path | Result |
| --- | --- | --- |
| POST | `/api/v1/shipments/{id}/items` | Add one item using the item fields above; returns 201 and the persisted item |
| DELETE | `/api/v1/shipments/{id}/items/{item_id}` | Remove that shipment's item; requires delete permission and returns 204 |

Edits are permitted only in `pending`, `processing` and `on_hold`; contents
freeze at `ready_to_ship`. Each edit advances the parent version and commits
its outbox fact atomically. Both endpoints accept an optional query parameter,
for example `POST /api/v1/shipments/{id}/items?expected_version=2` or
`DELETE /api/v1/shipments/{id}/items/{item_id}?expected_version=2`. The native
transaction checks the locked parent's version and returns 409 for stale edits,
without changing items or audit facts. Read the latest shipment to reconcile
before retrying with a new version. Omitting the parameter preserves the
unconditional edit behavior; malformed or unknown query fields return 400.

Use an `Idempotency-Key` to safely replay an identical request, including its
original version precondition, even after the successful edit advanced the
version. Changed bodies or query strings with the same key are refused.
Fingerprints include the exact query string: reordered or differently encoded
parameters also count as a different request. Existing persisted receipts for
requests with query strings use the previous fingerprint and cannot be replayed;
reconcile their outcome before using a new key. Requests without query strings
retain their existing fingerprint. A foreign item ID cannot bypass the shipment
path or tenant scope. Manifest assignments do not
reserve inventory or update order fulfilled quantities.

Agents using the CLI/MCP catalog can perform the same transitions with
`update_shipment`, using camelCase input names such as `expectedVersion`.
`create_shipment` requires `recipientName` and `shippingAddress`; `service`
remains an alias for `shippingMethod`. `ship_shipment`, `deliver_shipment` and
`cancel_shipment` also accept `expectedVersion`. All these tools require
`--apply` to mutate. Strict kernel mode excludes ungoverned shipment mutations;
typed governed fulfillment commands remain a separate release requirement.
Provider-label tracking reads preserve state. Apply observed provider events
through the write-gated `ingest_shipping_provider_webhook` tool; the former
`advanceStatus` option is no longer exposed by `track_shipping_label`.

## Part of StateSet iCommerce

Wraps [`stateset-embedded`](https://crates.io/crates/stateset-embedded) and exposes
the [`stateset-a2a`](https://crates.io/crates/stateset-a2a) agent surface. Part of the
[StateSet iCommerce](https://github.com/stateset/stateset-icommerce) engine.

## License

MIT OR Apache-2.0

[`axum`]: https://crates.io/crates/axum
