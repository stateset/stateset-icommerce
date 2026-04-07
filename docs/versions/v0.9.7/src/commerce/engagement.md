# Customer Engagement

iCommerce includes a complete customer engagement suite: loyalty programs, gift cards, store credits, wishlists, reviews, and dynamic customer segments. These tools enable retention strategies and lifetime value optimization.

## Loyalty Programs

Create tiered loyalty programs with points earning, multipliers, and perks.

### Creating a Program

```javascript
await toolkit.executeTool('create_loyalty_program', {
    name: 'Premium Rewards',
    pointsPerDollar: 10,
    currency: 'USD',
    tiers: [
        { name: 'Bronze', minPoints: 0, multiplier: 1.0, perks: ['Free shipping on orders over $50'] },
        { name: 'Silver', minPoints: 5000, multiplier: 1.5, perks: ['Free shipping', '10% off first order each month'] },
        { name: 'Gold', minPoints: 20000, multiplier: 2.0, perks: ['Free shipping', '15% off', 'Early access to sales'] },
        { name: 'Platinum', minPoints: 50000, multiplier: 3.0, perks: ['Free shipping', '20% off', 'Priority support', 'Exclusive products'] },
    ],
});
```

### Points Lifecycle

```
Purchase → Points earned (amount × pointsPerDollar × tier multiplier)
                ↓
Points balance → Redeem for rewards or discounts
                ↓
Refund → Points deducted proportionally
```

### Managing Members

```javascript
// Enroll a customer
await toolkit.executeTool('enroll_loyalty_member', {
    programId: 'prog-001',
    customerId: 'cust-123',
});

// Award bonus points
await toolkit.executeTool('award_loyalty_points', {
    programId: 'prog-001',
    customerId: 'cust-123',
    points: 500,
    reason: 'Birthday bonus',
});

// Check member status
const member = await toolkit.executeTool('get_loyalty_member', {
    programId: 'prog-001',
    customerId: 'cust-123',
});
// → { tier: 'Silver', points: 7500, lifetimePoints: 12000, multiplier: 1.5 }

// Redeem points
await toolkit.executeTool('redeem_loyalty_points', {
    programId: 'prog-001',
    customerId: 'cust-123',
    points: 2000,
    rewardType: 'discount',
    discountAmount: 20.00,
});
```

## Gift Cards

Issue, redeem, and track gift cards with balance management and expiration.

```javascript
// Create a gift card
const card = await toolkit.executeTool('create_gift_card', {
    initialBalance: 100.00,
    currency: 'USD',
    expiresAt: '2027-03-17',
    recipientEmail: 'friend@example.com',
    message: 'Happy birthday!',
});
// → { code: 'GC-ABCD-1234', balance: 100.00, expiresAt: '2027-03-17' }

// Redeem at checkout
await toolkit.executeTool('redeem_gift_card', {
    code: 'GC-ABCD-1234',
    amount: 35.00,
    orderId: 'ord-456',
});

// Check remaining balance
const balance = await toolkit.executeTool('get_gift_card', {
    code: 'GC-ABCD-1234',
});
// → { balance: 65.00, redeemed: 35.00, transactions: [...] }
```

## Store Credits

Issue credits for refunds, goodwill gestures, or promotional campaigns.

```javascript
// Issue store credit
await toolkit.executeTool('issue_store_credit', {
    customerId: 'cust-123',
    amount: 25.00,
    reason: 'Late delivery compensation',
    expiresAt: '2026-06-17',
});

// Apply at checkout
await toolkit.executeTool('apply_store_credit', {
    customerId: 'cust-123',
    orderId: 'ord-789',
    amount: 25.00,
});

// Check credit balance
const credits = await toolkit.executeTool('get_store_credits', {
    customerId: 'cust-123',
});
// → { available: 25.00, pending: 0, expired: 10.00, transactions: [...] }
```

## Wishlists

Customers can create private, public, or shared wishlists.

```javascript
// Create a wishlist
await toolkit.executeTool('create_wishlist', {
    customerId: 'cust-123',
    name: 'Holiday Gift Ideas',
    visibility: 'shared',  // 'private', 'public', or 'shared'
});

// Add items
await toolkit.executeTool('add_wishlist_item', {
    wishlistId: 'wl-001',
    productId: 'prod-456',
    quantity: 1,
    note: 'Size medium, blue color',
});

// Convert to cart
await toolkit.executeTool('wishlist_to_cart', {
    wishlistId: 'wl-001',
    customerId: 'cust-123',
});
```

## Product Reviews

Moderated review system with ratings and verification.

```javascript
// Submit a review
await toolkit.executeTool('create_review', {
    productId: 'prod-456',
    customerId: 'cust-123',
    rating: 4,
    title: 'Great quality',
    body: 'Excellent build quality, slightly slow shipping.',
    verified: true,  // customer actually purchased
});

// Moderate reviews
await toolkit.executeTool('moderate_review', {
    reviewId: 'rev-001',
    status: 'approved',  // 'approved', 'rejected', 'flagged'
});

// Get product review summary
const reviews = await toolkit.executeTool('get_product_reviews', {
    productId: 'prod-456',
});
// → { averageRating: 4.2, totalReviews: 47, distribution: { 5: 20, 4: 15, 3: 8, 2: 3, 1: 1 } }
```

## Customer Segments

Dynamic segments evaluate membership in real-time based on conditions.

```javascript
// Create a segment with compound conditions
await toolkit.executeTool('create_segment', {
    name: 'High-Value Repeat Buyers',
    conditions: {
        all: [
            { field: 'total_orders', operator: 'gte', value: 5 },
            { field: 'lifetime_value', operator: 'gte', value: 500 },
            { field: 'last_order_date', operator: 'within_days', value: 90 },
        ]
    },
});

// Check segment membership
const members = await toolkit.executeTool('get_segment_members', {
    segmentId: 'seg-001',
    limit: 50,
});

// Use segments in policies
// policies/promotions.yaml
// conditions:
//   - field: customer.segment
//     operator: in
//     value: ['high-value-repeat-buyers']
```

### Segment Operators

| Operator | Description |
|----------|-------------|
| `eq`, `neq` | Exact match / not equal |
| `gt`, `gte`, `lt`, `lte` | Numeric comparison |
| `in`, `not_in` | Value in / not in list |
| `contains` | String or array contains |
| `within_days` | Date within N days of today |
| `before`, `after` | Date comparison |

Compound conditions support `all` (AND) and `any` (OR) logic.

## MCP Tools

| Tool | Description |
|------|-------------|
| `create_loyalty_program` | Create a loyalty program with tiers |
| `get_loyalty_program` | Get program details |
| `enroll_loyalty_member` | Enroll customer in program |
| `get_loyalty_member` | Check member status and points |
| `award_loyalty_points` | Award bonus points |
| `redeem_loyalty_points` | Redeem points for rewards |
| `create_gift_card` | Issue a gift card |
| `get_gift_card` | Check gift card balance |
| `redeem_gift_card` | Apply gift card to order |
| `issue_store_credit` | Issue store credit |
| `get_store_credits` | Check credit balance |
| `apply_store_credit` | Use credit at checkout |
| `create_wishlist` | Create a wishlist |
| `add_wishlist_item` | Add product to wishlist |
| `wishlist_to_cart` | Convert wishlist to cart |
| `create_review` | Submit a product review |
| `moderate_review` | Approve/reject a review |
| `get_product_reviews` | Get reviews and summary |
| `create_segment` | Create a customer segment |
| `get_segment_members` | List segment members |
