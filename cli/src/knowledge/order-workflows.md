# Order Workflows

## Order Lifecycle

Orders progress through the following states:

```
pending → confirmed → processing → shipped → delivered
                    ↘ cancelled (from pending or confirmed only)
```

### State Descriptions
- **pending** — Order created but not yet confirmed (payment not yet captured)
- **confirmed** — Payment captured; order is ready to fulfill
- **processing** — Warehouse is picking/packing items
- **shipped** — At least one shipment has been dispatched with a tracking number
- **delivered** — All shipments confirmed delivered
- **cancelled** — Order was cancelled before shipment

## Cancellation Rules

- Only orders in `pending` or `confirmed` state can be cancelled
- Orders in `processing`, `shipped`, or `delivered` state cannot be directly cancelled
- If cancellation is needed after shipment, initiate a return (RMA) instead
- Cancellation triggers a full refund for captured payments

## Partial Fulfillment

When not all ordered items are immediately available:

1. **Split shipments** — ship available items now, remainder in a later shipment
2. **Backorder handling** — hold the order until all items are in stock, then ship together
3. Each shipment gets its own tracking number and delivery status
4. The order remains `processing` until all shipments are `delivered`

## Order Updates

- `update_order_status` — advance the order through its lifecycle
- `ship_order` — mark as shipped with a carrier and tracking number
- Order items and quantities cannot be changed after confirmation; cancel and re-create instead

## Returns After Delivery

- Returns are only valid within the return window (default: 30 days from delivery)
- Customer must have a valid RMA number before sending items back
- Return reasons: defective, wrong item, not as described, no longer needed, changed mind
- Refund issued after item inspection passes

## Common Order Queries

- `list_orders` — filter by status, date range, customer
- `get_order` — full details including line items, shipments, payments
- `get_order_status_breakdown` — analytics view of orders by status
- `get_top_products` — best-selling products by revenue or units

## Order ID Format

Orders use prefixed IDs: `ORD-XXXXX` where X is alphanumeric. UUID format is also accepted.
