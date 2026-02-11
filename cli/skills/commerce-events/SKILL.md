---
name: commerce-events
description: Stream real-time commerce events and manage webhooks. Use when debugging live behavior (orders/inventory/customers/products/returns) or integrating external systems via webhooks.
---

# Commerce Events

Stream real-time commerce domain events (pub/sub) and manage webhook endpoints for event delivery.

## How It Works

1. Subscribe to an in-process event stream.
2. Filter events by category (orders, inventory, customers, products, returns).
3. Output as human-readable lines or JSON (one event per line).
4. Optionally manage webhook endpoints for event delivery.

## Usage

- CLI: `stateset-events`, `stateset-events --filter orders`, `stateset-events --json`.
- Webhooks: `stateset-events webhooks list`, `stateset-events webhooks add <url>`, `stateset-events webhooks remove <id>`.

## Event Fields

- `event_type`: Event type as snake_case (e.g. `order_created`, `inventory_adjusted`).
- `timestamp`: RFC3339 timestamp.
- Domain fields depend on event type (e.g. `order_id`, `customer_id`, `sku`, `quantity`).
- Note: the raw serde tag is `type`; the Node binding also adds `event_type` for convenience.

## Output

```json
{"event_type":"order_created","order_id":"...","customer_id":"...","total_amount":"100.00","item_count":2,"timestamp":"2026-02-10T00:00:00Z","type":"order_created"}
```

## Present Results to User

- Keep output scannable: show `event_type`, timestamp, and the most relevant identifiers.
- When filtering, state which categories or event types are included.
- For inventory, highlight `quantity_change` and `new_quantity` when present.

## Troubleshooting

- No events arriving: ensure you subscribed before triggering the operation that emits the event.
- Filter returns nothing: check event types are snake_case and match the core enum.
- Webhooks not listed: webhooks are in-process and not persisted unless you add persistence on top.

## References
- /home/dom/stateset-icommerce/crates/stateset-core/src/events.rs
- /home/dom/stateset-icommerce/crates/stateset-embedded/src/events/mod.rs
- /home/dom/stateset-icommerce/cli/bin/stateset-events.js
- /home/dom/stateset-icommerce/bindings/node/src/lib.rs
