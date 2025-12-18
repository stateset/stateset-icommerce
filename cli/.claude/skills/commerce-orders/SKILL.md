---
name: commerce-orders
description: Use when managing order lifecycles, fulfillment workflows, or order status transitions.
---

# Commerce Orders Skill

Domain knowledge for order management and fulfillment operations.

## Order Lifecycle

### Status Flow
```
         ┌─────────────────────────────────────┐
         │                                     │
         ▼                                     │
    [pending] ──► [confirmed] ──► [processing] │
         │              │              │       │
         │              │              ▼       │
         │              │         [shipped] ───┤
         │              │              │       │
         │              │              ▼       │
         │              │        [delivered]   │
         │              │                      │
         └──────────────┴──► [cancelled] ◄─────┘
                             [refunded]
```

### Status Definitions

| Status | Description | Next States |
|--------|-------------|-------------|
| `pending` | Order created, awaiting confirmation | confirmed, cancelled |
| `confirmed` | Payment verified, ready to process | processing, cancelled |
| `processing` | Being picked/packed | shipped, cancelled |
| `shipped` | In transit to customer | delivered |
| `delivered` | Received by customer | refunded |
| `cancelled` | Order cancelled | - |
| `refunded` | Payment reversed | - |

## Payment Status

| Status | Description |
|--------|-------------|
| `pending` | Awaiting payment |
| `paid` | Payment received |
| `failed` | Payment declined |
| `refunded` | Full refund issued |
| `partially_refunded` | Partial refund issued |

## Fulfillment Status

| Status | Description |
|--------|-------------|
| `unfulfilled` | No items shipped |
| `partially_fulfilled` | Some items shipped |
| `fulfilled` | All items shipped |
| `returned` | Items returned |

## Order Structure

```javascript
{
  id: "uuid",
  orderNumber: "ORD-12345",
  customerId: "customer-uuid",

  // Status
  status: "pending",
  paymentStatus: "paid",
  fulfillmentStatus: "unfulfilled",

  // Amounts
  totalAmount: 89.97,
  currency: "USD",

  // Items
  items: [
    {
      id: "item-uuid",
      sku: "WIDGET-001",
      name: "Premium Widget",
      quantity: 3,
      unitPrice: 29.99,
      total: 89.97
    }
  ],

  // Shipping
  shippingAddress: { ... },
  trackingNumber: "FEDEX123",

  // Metadata
  notes: "Gift wrap requested",
  createdAt: "2024-01-15T10:30:00Z",
  updatedAt: "2024-01-15T10:30:00Z"
}
```

## Order Number Format

Default format: `ORD-{sequence}`

Examples:
- `ORD-1`
- `ORD-12345`
- `ORD-1000000`

## Common Operations

### Create Order
Required:
- `customerId` - Customer UUID
- `items` - At least one item with sku, name, quantity, unitPrice

Optional:
- `currency` - Default USD
- `notes` - Order notes

### Update Status
Valid transitions only:
- pending → confirmed, cancelled
- confirmed → processing, cancelled
- processing → shipped
- shipped → delivered

### Ship Order
Requires:
- Order in `processing` status
- Optional tracking number

Sets:
- status → shipped
- fulfillmentStatus → fulfilled
- trackingNumber

### Cancel Order
Allowed for:
- pending orders
- confirmed orders
- processing orders (with inventory release)

Not allowed for:
- shipped orders (use return instead)
- delivered orders (use return instead)

## Order Events

| Event | Trigger |
|-------|---------|
| `order_created` | New order placed |
| `order_confirmed` | Payment verified |
| `order_shipped` | Shipment created |
| `order_delivered` | Delivery confirmed |
| `order_cancelled` | Order cancelled |
| `order_refunded` | Refund processed |

## Best Practices

1. **Validate before create** - Check customer exists, items valid
2. **Reserve inventory** - Hold stock when order placed
3. **Confirm payment** - Wait for payment before processing
4. **Track everything** - Always add tracking numbers
5. **Handle failures** - Release inventory on cancel

## Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `Order not found` | Invalid ID/number | Verify identifier |
| `Invalid status` | Bad transition | Check current status |
| `Customer not found` | Bad customer ID | Verify customer |
| `Already shipped` | Cannot modify | Create return instead |

## Shipping Integration

### Tracking Number Format
Varies by carrier:
- FedEx: 12-22 characters
- UPS: 1Z + 16 alphanumeric
- USPS: 20-22 digits
- DHL: 10 digits

### Setting Tracking
```javascript
ship_order({
  orderId: "uuid",
  trackingNumber: "FEDEX123456789"
})
```
