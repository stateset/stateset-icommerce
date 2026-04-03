# Customers & Segments

Customer management covers profiles, contact information, segmentation, and lifetime value tracking.

## Operations

### Create a Customer

```javascript
const customer = commerce.customers.create({
    email: 'alice@example.com',
    firstName: 'Alice',
    lastName: 'Smith',
    phone: '+1-555-0123'
});
```

### List and Search

```javascript
const customers = commerce.customers.list();
const customer = commerce.customers.get(customerId);
```

```bash
stateset "search customers like enterprise retail buyers"
stateset "find customers who ordered in the last 30 days"
```

### Customer Count

```javascript
const count = commerce.customers.count();
```

## Segmentation

Create customer segments for targeted campaigns and analytics:

```javascript
// MCP tool: create_customer_segment
const segment = await toolkit.executeTool('create_customer_segment', {
    name: 'High-Value Customers',
    criteria: {
        totalSpend: { greaterThan: 1000 },
        orderCount: { greaterThan: 5 }
    }
});
```

## Loyalty Programs

Track loyalty points and tier progression:

```javascript
// Award points
await toolkit.executeTool('award_loyalty_points', {
    customerId: customer.id,
    points: 500,
    reason: 'Purchase order #12345'
});

// Check balance
const balance = await toolkit.executeTool('get_loyalty_balance', {
    customerId: customer.id
});

// Redeem points
await toolkit.executeTool('redeem_loyalty_points', {
    customerId: customer.id,
    points: 200,
    orderId: order.id
});
```

## Wishlists

```javascript
await toolkit.executeTool('add_to_wishlist', {
    customerId: customer.id,
    productId: product.id
});
```

## Customer Lifecycle Events

| Event | Trigger |
|-------|---------|
| `customer.created` | New customer registered |
| `customer.updated` | Profile information changed |
| `customer.deleted` | Customer removed |

## GDPR Data Handling

The exact GDPR tool surface depends on the runtime and deployment tier. Verify the named tools below against your actual MCP registry before depending on them in production workflows.

```javascript
// Export all customer data (Right to Portability)
await toolkit.executeTool('export_gdpr_subject_data', {
    customerId: customer.id,
    format: 'json'
});

// Delete all customer data (Right to Erasure)
await toolkit.executeTool('request_gdpr_erasure', {
    customerId: customer.id,
    confirm: true
});
```

See [Compliance & Audit](../advanced/compliance.md) for full GDPR handling.

## MCP Tools

| Tool | Description |
|------|-------------|
| `list_customers` | List all customers (paginated) |
| `get_customer` | Get customer by ID |
| `create_customer` | Create a new customer |
| `update_customer` | Update customer details |
| `delete_customer` | Remove a customer (requires approval) |
| `create_customer_segment` | Define a segment by criteria |
| `list_segments` | List defined segments |
| `award_loyalty_points` | Add loyalty points |
| `get_loyalty_balance` | Check point balance |
| `redeem_loyalty_points` | Use points on an order |
| `add_to_wishlist` | Add product to customer wishlist |
| `get_wishlist` | Get customer wishlist items |
