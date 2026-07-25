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
- **Bearer auth**, CORS, and request-ID propagation as opt-in layers
- **OpenAPI** description generated from the route table

## Security Defaults

`with_bearer_auth` is opt-in, not implied — an unconfigured server is unauthenticated,
so bind it to localhost or put it behind a gateway during development. See the
[deployment guide](https://github.com/stateset/stateset-icommerce/blob/master/docs/src/advanced/deployment.md)
before exposing it publicly.

## Part of StateSet iCommerce

Wraps [`stateset-embedded`](https://crates.io/crates/stateset-embedded) and exposes
the [`stateset-a2a`](https://crates.io/crates/stateset-a2a) agent surface. Part of the
[StateSet iCommerce](https://github.com/stateset/stateset-icommerce) engine.

## License

MIT OR Apache-2.0

[`axum`]: https://crates.io/crates/axum
