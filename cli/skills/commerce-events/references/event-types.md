# Commerce Event Types Reference

## Event Naming Convention

Events follow the pattern: `{entity}.{action}`

## Event Types by Domain

### Orders
- `order.created` - New order placed
- `order.confirmed` - Order confirmed
- `order.processing` - Order in processing
- `order.shipped` - Order shipped
- `order.delivered` - Order delivered
- `order.cancelled` - Order cancelled

### Inventory
- `inventory.adjusted` - Stock quantity changed
- `inventory.reserved` - Stock reserved for order
- `inventory.reservation_confirmed` - Reservation confirmed
- `inventory.reservation_released` - Reservation released
- `inventory.item_created` - New inventory item

### Payments
- `payment.created` - Payment initiated
- `payment.completed` - Payment succeeded
- `payment.failed` - Payment failed
- `payment.refunded` - Refund issued

### Returns
- `return.created` - Return requested
- `return.approved` - Return approved
- `return.rejected` - Return rejected
- `return.completed` - Return processed

### Customers
- `customer.created` - New customer registered
- `customer.updated` - Customer info changed

### Products
- `product.created` - New product added
- `product.updated` - Product info changed
- `product.activated` - Product made active
- `product.deactivated` - Product deactivated

### Shipments
- `shipment.created` - Shipment created
- `shipment.shipped` - Shipment dispatched
- `shipment.delivered` - Shipment delivered

### Subscriptions
- `subscription.created` - New subscription
- `subscription.paused` - Subscription paused
- `subscription.resumed` - Subscription resumed
- `subscription.cancelled` - Subscription cancelled
- `billing_cycle.created` - New billing cycle

## Event Payload Structure

```json
{
  "event_id": "uuid",
  "event_type": "order.created",
  "entity_type": "order",
  "entity_id": "ord_123",
  "payload": { ... },
  "agent_id": "agent_uuid",
  "agent_signature": "base64_ed25519_sig",
  "sequence_number": 42,
  "created_at": "2025-01-15T10:30:00Z"
}
```

## Outbox Pattern

Events are written to a local outbox table before sync:
1. Commerce operation writes event to outbox (atomic with data change)
2. Sync process reads pending outbox events
3. Events pushed to sequencer (grpc://sequencer.stateset.network)
4. Sequencer assigns global sequence number
5. Event acknowledged and marked synced

## Idempotency Keys

| Field | Description |
|-------|-------------|
| `key` | Unique operation identifier |
| `result` | Cached response from first execution |
| `created_at` | When key was created |
| `expires_at` | Auto-expiration time |

Usage: Include idempotency key in write operations to prevent duplicates on retry.
