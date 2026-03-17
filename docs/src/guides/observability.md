# Observability & Telemetry

iCommerce provides structured logging, metrics, distributed tracing, and health checks across both the Rust core and CLI layers.

## Three Pillars

| Pillar | Rust Core | CLI Layer |
|--------|-----------|-----------|
| **Logging** | `tracing` crate, structured JSON | `cli/src/telemetry.js`, PII redaction |
| **Metrics** | Lock-free atomic counters | Per-tool invocation tracking |
| **Tracing** | OpenTelemetry-compatible spans | W3C Trace Context propagation |

## Logging

### Rust (stateset-observability)

```bash
# Enable debug logging for the commerce engine
RUST_LOG=stateset_embedded=debug ./my-app

# Enable all StateSet logs
RUST_LOG=stateset=trace ./my-app

# JSON format for log aggregation
RUST_LOG=stateset_embedded=info ./my-app --json-logs
```

Output:
```json
{"timestamp":"2026-03-16T10:30:45Z","level":"INFO","target":"stateset_embedded::orders","message":"order created","order_id":"ord_abc123","customer_id":"cust_xyz"}
```

See [Logging & Debugging](logging.md) for complete Rust logging configuration, custom spans, and OpenTelemetry integration.

### CLI Telemetry

The CLI telemetry module (`cli/src/telemetry.js`) tracks:

- Tool invocation name, duration, and success/failure
- Automatic PII redaction (emails → `***@***.com`, API keys → `sk-...***`)
- Agent ID and session ID for multi-agent correlation

PII is redacted before any data leaves the process — sensitive fields are never logged or transmitted.

## Metrics

### Rust Counters

```rust
use stateset_observability::counters;

// Increment on each operation
counters::orders_created().increment();
counters::payments_captured().increment();
counters::inventory_adjustments().increment();

// Read current values
let created = counters::orders_created().get();
```

Counters are lock-free atomics — zero contention even under high concurrency.

### CLI Metrics

Tool invocation metrics are available via the HTTP gateway:

```bash
curl -H "Authorization: Bearer $KEY" http://localhost:8080/metrics
# → {
#     "uptime": "2h 15m",
#     "uptimeMs": 8100000,
#     "totals": {
#         "toolCalls": 1547,
#         "toolErrors": 12,
#         "avgDurationMs": 23
#     }
# }
```

### A2A Metrics

```javascript
const metrics = await toolkit.executeTool('a2a_rate_limit_metrics', {});
// → { requests: 450, limit: 1000, remaining: 550, resetAt: '...' }

const health = await toolkit.executeTool('a2a_health_status', {});
// → { status: 'healthy', checks: { database: 'ok', sequencer: 'ok', ... } }
```

## Distributed Tracing

### W3C Trace Context

The A2A tracing service propagates trace context across agent boundaries:

```javascript
import { createTracingService } from '@stateset/cli/a2a/tracing';

const tracing = createTracingService({ maxSpans: 5000 });

// Create a span for a commerce operation
const result = await tracing.withSpan('process_order', async () => {
    const order = await toolkit.executeTool('create_order', params);
    await toolkit.executeTool('capture_payment', { orderId: order.id });
    return order;
});

// Inject trace context into outgoing HTTP headers
const headers = {};
tracing.inject(headers);
// → { traceparent: '00-{traceId}-{spanId}-01', tracestate: '...' }

// Performance percentiles
const metrics = tracing.getMetrics();
// → { p50: 12, p95: 45, p99: 120, errorRate: 0.02, throughput: 150 }
```

### OpenTelemetry (Rust)

```rust
use tracing_opentelemetry::OpenTelemetryLayer;

let tracer = opentelemetry_jaeger::new_agent_pipeline()
    .with_service_name("icommerce")
    .install_simple()?;

tracing_subscriber::registry()
    .with(OpenTelemetryLayer::new(tracer))
    .with(tracing_subscriber::fmt::layer())
    .init();
```

This sends spans to Jaeger, Zipkin, Datadog, or any OpenTelemetry-compatible backend.

## Health Checks

Three probe types for production deployments:

| Endpoint | Purpose | Response |
|----------|---------|----------|
| `GET /health` | Full health check (DB + sequencer + subsystems) | `{ status: 'healthy', checks: {...} }` |
| `GET /health/live` (or `/livez`) | Kubernetes liveness probe | `{ status: 'alive' }` |
| `GET /health/ready` (or `/readyz`) | Kubernetes readiness probe (tests DB) | `{ status: 'ready' }` |

Returns `200` when healthy, `503` when unhealthy.

## Heartbeat Monitor

The [heartbeat monitor](heartbeat.md) runs periodic commerce health checks:

| Checker | What It Monitors |
|---------|-----------------|
| `low-stock` | Items below stock threshold |
| `abandoned-carts` | Carts older than N hours |
| `revenue-milestone` | Revenue target for period |
| `pending-returns` | Returns older than N days |
| `overdue-invoices` | Unpaid invoices past due date |
| `subscription-churn` | Cancelled/past-due subscriptions |

Alerts flow through the EventBridge to Slack, Telegram, Discord, or any configured notification channel.

## Audit Trail

Every tool invocation is logged in the audit trail:

```javascript
// Query recent denials
const denials = await toolkit.executeTool('audit_query', { result: 'denied', limit: 10 });

// Export for compliance
await toolkit.executeTool('audit_export', { format: 'json', startDate: '2026-03-01' });
```

See [Compliance & Audit](../advanced/compliance.md) for GDPR exports, SOC 2 evidence packages, and VES proof generation.

## Agent Introspection

Debug individual agent decision-making:

```javascript
const dashboard = await toolkit.executeTool('a2a_agent_introspection', {
    agentId: 'fulfillment-agent'
});
// → { decisions: 150, avgTickMs: 23, recentDecisions: [...] }
```

See [A2A Advanced — Introspection](../a2a/advanced.md#agent-introspection) for details.

## Quick Reference

| What You Want | How To Get It |
|--------------|---------------|
| Debug a specific operation | `RUST_LOG=stateset_embedded::orders=debug` |
| See all tool calls | `GET /metrics` or `audit_query` |
| Track cross-agent latency | `tracing.getMetrics()` → p50/p95/p99 |
| Monitor commerce health | Enable heartbeat checks in config |
| Kubernetes probes | `GET /health/live` and `GET /health/ready` |
| Compliance export | `audit_export` or `generate_compliance_package` |
