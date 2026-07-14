# Carts & Checkout

Shopping cart management with pricing, promotions, tax calculation, and checkout flows.

## Cart Operations

### Create a Cart

```javascript
const cart = commerce.carts.create({
    customerId: customer.id,
    currency: 'USD'
});
```

### Add and Remove Items

```javascript
commerce.carts.addItem(cart.id, {
    sku: 'WIDGET-001',
    name: 'Premium Widget',
    quantity: 2,
    unitPrice: 29.99
});

commerce.carts.removeItem(cart.id, 'WIDGET-001');
commerce.carts.updateQuantity(cart.id, 'WIDGET-001', 3);
```

### Apply Promotions

```javascript
commerce.carts.applyDiscount(cart.id, 'SUMMER20');
```

### Checkout

```javascript
const order = commerce.carts.checkout(cart.id, {
    paymentMethod: 'card',
    shippingAddress: {
        line1: '123 Main St',
        city: 'San Francisco',
        state: 'CA',
        zip: '94102',
        country: 'US'
    }
});
```

## Abandoned Cart Detection

The heartbeat monitor detects abandoned carts:

```json
{
    "id": "abandoned-carts",
    "checker": "abandoned-carts",
    "intervalMs": 86400000,
    "enabled": true,
    "config": { "minAgeHours": 24 }
}
```

## Cart Expiration

Carts have a configurable TTL. After the expiration period, the cart is automatically marked as abandoned. Any inventory reservations held by the cart are released.

## Guest vs. Authenticated Carts

```javascript
// Guest cart (no customer ID)
const guestCart = commerce.carts.create({ currency: 'USD' });

// Authenticated cart
const authCart = commerce.carts.create({ customerId: customer.id, currency: 'USD' });

// Merge: when a guest logs in, merge their cart into the authenticated one
commerce.carts.merge(guestCart.id, authCart.id);
```

## Express Checkout (Payment Links)

Create shareable checkout URLs for quick payment:

```javascript
const link = await toolkit.executeTool('create_payment_link', {
    amount: 59.98,
    currency: 'USD',
    description: '2x Premium Widget',
    expiresIn: '24h',
    successUrl: 'https://store.example.com/success',
    cancelUrl: 'https://store.example.com/cancel'
});
// → { url: 'https://pay.stateset.com/link/abc123', expiresAt: '...' }
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `create_cart` | Create a new cart (guest or authenticated) |
| `get_cart` | Get cart details with pricing |
| `add_cart_item` | Add item to cart |
| `remove_cart_item` | Remove item from cart |
| `update_cart_quantity` | Update item quantity |
| `apply_cart_discount` | Apply a promo code |
| `set_cart_shipping_address` | Set shipping destination |
| `set_cart_payment` | Set payment method |
| `get_shipping_rates` | Get available shipping options |
| `calculate_cart_total` | Get pricing breakdown (subtotal, tax, shipping, discounts) |
| `checkout_cart` | Convert cart to order (requires --apply) |
| `cancel_cart` | Cancel and release holds |
| `list_abandoned_carts` | Find stale carts for recovery |
| `create_payment_link` | Create shareable checkout URL |
| `get_payment_link_status` | Check link conversion status |
