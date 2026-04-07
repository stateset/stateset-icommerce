# Orders & Fulfillment

Orders are the central aggregate in iCommerce, linking customers, products, inventory, payments, and shipments.

## Order Lifecycle

```
┌─────────┐    ┌────────────┐    ┌─────────┐    ┌───────────┐
│ Pending  │───►│ Processing │───►│ Shipped │───►│ Delivered │
└────┬────┘    └────────────┘    └─────────┘    └───────────┘
     │
     └───────────────────────────►┌───────────┐
                                  │ Cancelled │
                                  └───────────┘
```

## Creating Orders

### Rust
```rust
let order = commerce.orders().create(CreateOrder {
    customer_id: customer.id.clone(),
    items: vec![OrderItem {
        sku: "WIDGET-001".into(),
        name: "Premium Widget".into(),
        quantity: 2,
        unit_price: Decimal::new(2999, 2),
        ..Default::default()
    }],
    currency: Some("USD".into()),
    ..Default::default()
})?;
```

### Node.js
```javascript
const order = commerce.orders.create({
    customerId: customer.id,
    items: [
        { sku: 'WIDGET-001', name: 'Premium Widget', quantity: 2, unitPrice: 29.99 }
    ],
    currency: 'USD'
});
```

### Python
```python
order = commerce.orders.create(
    customer_id=customer.id,
    items=[
        {"sku": "WIDGET-001", "name": "Premium Widget", "quantity": 2, "unit_price": 29.99}
    ],
    currency="USD"
)
```

### CLI
```bash
stateset --apply "create an order for customer alice@example.com: 2x Widget at $29.99"
```

## Order Operations

| Operation | Description |
|-----------|-------------|
| `create(params)` | Create a new order |
| `get(id)` | Retrieve an order by ID |
| `list()` | List all orders |
| `list_by_status(status)` | Filter orders by status |
| `list_by_customer(customer_id)` | Orders for a specific customer |
| `update_status(id, status)` | Advance the state machine |
| `ship(id)` | Mark as shipped (requires "processing" status) |
| `cancel(id)` | Cancel an order (only from "pending") |

## Fulfillment Flow

A typical fulfillment flow combines several domain APIs:

```javascript
// 1. Create the order
const order = commerce.orders.create({ customerId, items, currency: 'USD' });

// 2. Reserve inventory for each line item
for (const item of items) {
    commerce.inventory.reserve(item.sku, item.quantity);
}

// 3. Process payment
const payment = commerce.payments.create({
    orderId: order.id,
    amount: order.total,
    method: 'card'
});

// 4. Move to processing
commerce.orders.updateStatus(order.id, 'processing');

// 5. Create shipment
const shipment = commerce.shipments.create({
    orderId: order.id,
    carrier: 'FedEx',
    trackingNumber: 'FEDEX-789'
});

// 6. Ship the order
commerce.orders.ship(order.id);
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `list_orders` | List orders with optional status filter |
| `get_order` | Get order details by ID |
| `create_order` | Create a new order |
| `update_order_status` | Advance order state |
| `ship_order` | Mark order as shipped |
| `cancel_order` | Cancel a pending order |

## Events

Orders emit events at each state transition:

| Event | Trigger |
|-------|---------|
| `order.created` | New order placed |
| `order.processing` | Moved to processing |
| `order.shipped` | Shipment confirmed |
| `order.delivered` | Delivery confirmed |
| `order.cancelled` | Order cancelled |

Events flow through the EventBridge to webhooks, SSE streams, and the VES sync layer.
