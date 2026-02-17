# stateset-observability

Observability utilities for StateSet iCommerce — structured logging, Prometheus metrics, and tracing integration.

## Usage

```rust
use stateset_observability::init_tracing;

init_tracing();
tracing::info!("Commerce engine started");
```

## License

MIT OR Apache-2.0
