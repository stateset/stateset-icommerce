# Logging and Debugging Guide

StateSet Embedded uses the `tracing` crate for structured logging and diagnostics.

## Quick Start

Add tracing subscriber to your application:

```rust
use tracing_subscriber::{fmt, EnvFilter};

fn main() {
    // Initialize tracing with environment filter
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Now StateSet logs will appear
    let commerce = Commerce::new("commerce.db").unwrap();
}
```

## Setting Log Levels

Use the `RUST_LOG` environment variable:

```bash
# All StateSet logs at debug level
RUST_LOG=stateset_embedded=debug ./my-app

# Only errors
RUST_LOG=stateset_embedded=error ./my-app

# Multiple targets
RUST_LOG=stateset_embedded=debug,sqlx=warn ./my-app

# Everything
RUST_LOG=trace ./my-app
```

## Log Levels

| Level | Use Case |
|-------|----------|
| `error` | Operation failures, unrecoverable errors |
| `warn` | Degraded performance, approaching limits |
| `info` | Key operations (order created, payment completed) |
| `debug` | Detailed operation flow, SQL queries |
| `trace` | Very verbose, includes all parameters |

## Structured Logging Output

```rust
use tracing_subscriber::fmt;

// JSON format for log aggregation
tracing_subscriber::fmt()
    .json()
    .init();
```

Output:
```json
{"timestamp":"2024-01-15T10:30:45Z","level":"INFO","target":"stateset_embedded::orders","message":"order created","order_id":"ord_abc123","customer_id":"cust_xyz"}
```

## Common Scenarios

### Debugging Order Creation

```bash
RUST_LOG=stateset_embedded::orders=debug ./my-app
```

Output:
```
2024-01-15T10:30:45Z DEBUG stateset_embedded::orders: creating order customer_id="cust_123" items=3
2024-01-15T10:30:45Z DEBUG stateset_embedded::orders: validating inventory sku="SKU-001" requested=2
2024-01-15T10:30:45Z DEBUG stateset_embedded::orders: inventory reserved sku="SKU-001" quantity=2
2024-01-15T10:30:45Z INFO stateset_embedded::orders: order created order_id="ord_456" total=59.98
```

### Debugging Database Issues

```bash
# SQLite
RUST_LOG=stateset_db::sqlite=debug ./my-app

# PostgreSQL
RUST_LOG=sqlx=debug,stateset_db::postgres=debug ./my-app
```

### Debugging Webhooks

```bash
RUST_LOG=stateset_embedded::events::webhook=debug ./my-app
```

Output:
```
2024-01-15T10:30:45Z DEBUG webhook: sending event url="https://example.com/webhook" event_type="order.created"
2024-01-15T10:30:45Z DEBUG webhook: response received status=200 duration_ms=45
```

## Custom Spans

For detailed tracing in your application:

```rust
use tracing::{info_span, instrument};

#[instrument(skip(commerce))]
async fn process_checkout(
    commerce: &Commerce,
    cart_id: &str,
    payment_method: &str,
) -> Result<Order> {
    let span = info_span!("checkout", cart_id, payment_method);
    let _guard = span.enter();

    // All operations within this function will be tagged
    let cart = commerce.carts().get(cart_id)?;
    let order = commerce.orders().create_from_cart(&cart)?;
    let payment = commerce.payments().create(&order.id, order.total)?;

    Ok(order)
}
```

## Integration with Observability Tools

### OpenTelemetry

```rust
use opentelemetry::sdk::trace::TracerProvider;
use tracing_opentelemetry::OpenTelemetryLayer;

let tracer = opentelemetry_jaeger::new_agent_pipeline()
    .with_service_name("my-commerce-app")
    .install_simple()?;

tracing_subscriber::registry()
    .with(OpenTelemetryLayer::new(tracer))
    .with(tracing_subscriber::fmt::layer())
    .init();
```

### Datadog

```rust
use tracing_subscriber::fmt::format::FmtSpan;

tracing_subscriber::fmt()
    .json()
    .with_span_events(FmtSpan::CLOSE)
    .with_current_span(true)
    .init();
```

## Filtering Sensitive Data

Avoid logging sensitive information:

```rust
use tracing::field::Empty;

// Use Empty for sensitive fields
tracing::info!(
    customer_id = %customer.id,
    email = Empty,  // Don't log email
    action = "customer_created"
);
```

## Performance Considerations

- Use appropriate log levels in production (`info` or `warn`)
- `debug` and `trace` levels have performance overhead
- JSON logging is slower than plain text
- Async logging reduces impact on main thread

```rust
// Production configuration
tracing_subscriber::fmt()
    .with_env_filter("stateset_embedded=info,warn")
    .with_ansi(false)  // No colors in prod logs
    .init();
```

## Troubleshooting

### No Logs Appearing

1. Ensure tracing subscriber is initialized
2. Check `RUST_LOG` environment variable
3. Verify target names match (use `=trace` to see all)

### Too Many Logs

Filter to specific modules:
```bash
RUST_LOG=stateset_embedded::orders=info,stateset_embedded::inventory=warn
```

### Async Context Lost

Use `tracing-futures` for async spans:
```rust
use tracing_futures::Instrument;

async fn my_operation() {
    commerce.orders().list()
        .instrument(info_span!("list_orders"))
        .await
}
```
