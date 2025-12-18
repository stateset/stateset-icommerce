---
name: commerce-checkout
description: Use when managing shopping carts, checkout flows, or implementing the Agentic Commerce Protocol (ACP).
---

# Commerce Checkout Skill

Domain knowledge for shopping cart and checkout operations using the Agentic Commerce Protocol (ACP).

## Agentic Commerce Protocol (ACP)

ACP is a standardized protocol for AI agents to interact with commerce systems. It defines:

1. **Cart Lifecycle** - Creation, modification, completion
2. **Checkout Flow** - Address, shipping, payment, confirmation
3. **Recovery Patterns** - Abandoned cart handling

## Cart States

| State | Description | Transitions |
|-------|-------------|-------------|
| `active` | Cart is being modified | → ready_for_payment, cancelled, abandoned, expired |
| `ready_for_payment` | All info collected | → payment_pending, cancelled |
| `payment_pending` | Payment in progress | → completed, failed |
| `completed` | Order created | Terminal state |
| `cancelled` | User cancelled | Terminal state |
| `abandoned` | Marked for recovery | → active (if recovered) |
| `expired` | Cart timed out | Terminal state |

## Checkout Flow

### Standard Flow
```
1. Create Cart
   - Guest: email + name
   - Customer: customerId

2. Add Items
   - SKU, name, quantity, price
   - Can add multiple items

3. Set Shipping Address
   - firstName, lastName
   - line1, line2 (optional)
   - city, state, postalCode, country

4. Select Shipping Method
   - Get rates based on address
   - Choose carrier/service

5. Apply Discounts (optional)
   - Coupon codes
   - Automatic discounts

6. Set Payment
   - Method: credit_card, paypal, crypto
   - Token from payment provider

7. Complete Checkout
   - Validates all required fields
   - Creates order
   - Returns orderId, orderNumber
```

## Cart Totals

| Field | Description |
|-------|-------------|
| `subtotal` | Sum of item totals |
| `taxAmount` | Calculated tax |
| `shippingAmount` | Shipping cost |
| `discountAmount` | Applied discounts |
| `grandTotal` | Final amount due |

Formula:
```
grandTotal = subtotal + taxAmount + shippingAmount - discountAmount
```

## Payment Methods

| Method | Description |
|--------|-------------|
| `credit_card` | Visa, Mastercard, Amex |
| `paypal` | PayPal checkout |
| `crypto` | Cryptocurrency payment |
| `bank_transfer` | ACH/Wire |
| `buy_now_pay_later` | Affirm, Klarna |

## Fulfillment Types

| Type | Description |
|------|-------------|
| `shipping` | Ship to customer address |
| `pickup` | Customer pickup at location |
| `digital` | Digital delivery |

## Cart Expiration

- Default: 24 hours
- Custom: Set `expiresInMinutes` on create
- Expired carts cannot be modified
- Recovery: Create new cart with same items

## Abandoned Cart Recovery

Carts become "abandoned" when:
- User navigates away without completing
- Explicit abandon marking
- Inactivity threshold reached

Recovery workflow:
1. `get_abandoned_carts` - List recovery candidates
2. Send reminder email (external)
3. User returns and resumes checkout
4. Or create new cart with recovered items

## Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `Cart not found` | Invalid ID | Check cart ID |
| `Cart expired` | Timed out | Create new cart |
| `Empty cart` | No items | Add items first |
| `Missing address` | No shipping | Set address |
| `Invalid payment` | Bad token | Retry payment |

## Best Practices

1. **Save cart ID** - Store for session continuity
2. **Validate incrementally** - Check each step
3. **Handle failures gracefully** - Preserve cart state
4. **Show clear totals** - Break down all charges
5. **Confirm before complete** - Final review step

## Integration Notes

### Creating Cart for Guest
```javascript
{
  customerEmail: "guest@example.com",
  customerName: "Guest User",
  currency: "USD"
}
```

### Creating Cart for Customer
```javascript
{
  customerId: "uuid-of-customer",
  currency: "USD"
}
```

### Cart Item Structure
```javascript
{
  sku: "WIDGET-001",
  name: "Premium Widget",
  description: "High quality widget",
  quantity: 2,
  unitPrice: 29.99,
  imageUrl: "https://..."
}
```
