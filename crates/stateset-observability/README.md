# stateset-observability

[![crates.io](https://img.shields.io/crates/v/stateset-observability.svg)](https://crates.io/crates/stateset-observability)
[![docs.rs](https://docs.rs/stateset-observability/badge.svg)](https://docs.rs/stateset-observability)

Observability primitives for StateSet iCommerce:

- Structured tracing bootstrap (`init_tracing`)
- Business counters (orders, payments, inventory)
- RED metrics (`record_request_*`)
- SLO evaluation (`SloTarget`, `SloEvaluation`)
- Span/metric naming conventions (`conventions`)

## Tracing

```rust
use stateset_observability::init_tracing;

init_tracing("stateset-marketplace", "production", "us-east-1")
    .expect("tracing init");
```

## Metrics + RED + SLO

```rust
use std::time::Duration;
use stateset_observability::{init_metrics, MetricsConfig, SloTarget};

let metrics = init_metrics(MetricsConfig::default());

metrics.record_order_created("cust-1", 129.99);
metrics.record_request_success("order.create", Duration::from_millis(42));
metrics.record_request_error("order.create", Duration::from_millis(180));

let snapshot = metrics.snapshot();

let report = snapshot
    .evaluate_operation_slo(
        "order.create",
        SloTarget {
            min_success_rate: 0.95,
            max_avg_latency_ms: 200.0,
            min_requests: 2,
        },
    )
    .expect("operation metrics");

assert!(report.requests >= 2);
```

## Conventions

- Span names: `stateset.<normalized_operation>` (for example `stateset.order_create`)
- Metric names:
  - `stateset_requests_total`
  - `stateset_request_errors_total`
  - `stateset_request_duration_ms_total`
- Label keys:
  - `service`
  - `operation`
  - `environment`
  - `region`
  - `outcome`

Use `normalize_name`, `operation_span_name`, and `operation_metric_label` to keep cardinality stable.

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `otel` | OpenTelemetry OTLP export via `tracing-opentelemetry` | No |

The tracing bootstrap is deliberately lightweight and imposes no backend on
downstream applications; counters are `AtomicU64` and safe to clone across threads.

## Part of StateSet iCommerce

Wired into [`stateset-embedded`](https://crates.io/crates/stateset-embedded) so engine
operations are instrumented by default. Part of the
[StateSet iCommerce](https://github.com/stateset/stateset-icommerce) engine.

## License

MIT OR Apache-2.0
