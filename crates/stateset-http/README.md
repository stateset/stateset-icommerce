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
let addr: SocketAddr = "0.0.0.0:3000".parse()?;

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
- **Bearer auth** by default, with optional CORS and request-ID propagation
- **OpenAPI** description generated from the route table

## Security Defaults

`ServerBuilder::new` generates a bearer token and protects API routes by default.
Configure an operator-owned token with `with_bearer_auth`, or use actor-bound
tokens with an authorization engine to enforce resource permissions. Disabling
authentication requires the explicit `without_auth` option. See the
[deployment guide](https://github.com/stateset/stateset-icommerce/blob/master/docs/src/advanced/deployment.md)
before exposing it publicly.

For shared hosting, configure `with_tenant_db_dir` and bind tokens to tenants
and actors. API requests include `x-tenant-id`; in tenant-bound deployments it
must match the operator-owned token binding. The single-store example explicitly
disables tenant-header routing while retaining the required request header.

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
