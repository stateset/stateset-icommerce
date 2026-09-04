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
// MCP tool: create_segment
const segment = await toolkit.executeTool('create_segment', {
    name: 'High-Value Customers',
    conditions: [
        { field: 'totalSpend', operator: 'gt', value: 1000 },
        { field: 'orderCount', operator: 'gt', value: 5 }
    ],
    conditionLogic: 'all'
});
```

## Loyalty Programs

Track loyalty points and tier progression:

```javascript
// Award points
await toolkit.executeTool('earn_points', {
    programId: 'prog-001',
    customerId: customer.id,
    points: 500,
    reason: 'manual',
    note: 'Purchase order #12345'
});

// Check account status
const account = await toolkit.executeTool('get_loyalty_account', {
    programId: 'prog-001',
    customerId: customer.id
});

// Redeem points
await toolkit.executeTool('redeem_points', {
    programId: 'prog-001',
    customerId: customer.id,
    points: 200,
    orderId: order.id
});
```

## Wishlists

```javascript
const createdWishlist = await toolkit.executeTool('create_wishlist', {
    customerId: customer.id,
    name: 'Holiday Picks'
});

const wishlistId = createdWishlist.result?.wishlist?.id;

await toolkit.executeTool('add_to_wishlist', {
    wishlistId,
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
await toolkit.executeTool('export_gdpr_data', {
    customerId: customer.id
});

// Delete all customer data (Right to Erasure)
await toolkit.executeTool('delete_gdpr_data', {
    customerId: customer.id,
    keepTransactions: true
});
```

See [Compliance & Audit](../advanced/compliance.md) for full GDPR handling.

## MCP Tools

| Tool | Description |
|------|-------------|
| `list_customers` | List all customers |
| `get_customer` | Get customer by ID or email via `identifier` |
| `create_customer` | Create a new customer |
| `create_segment` | Define a customer segment from conditions |
| `get_segment` | Fetch a segment by ID |
| `list_segments` | List defined segments |
| `earn_points` | Add loyalty points to an account |
| `get_loyalty_account` | Inspect a customer loyalty account |
| `redeem_points` | Redeem loyalty points |
| `create_wishlist` | Create a customer wishlist |
| `add_to_wishlist` | Add a product to a wishlist |
| `get_wishlist` | Get wishlist items |
| `list_wishlists` | List wishlists for a customer |
