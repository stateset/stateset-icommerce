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
await toolkit.executeTool('enroll_customer', {
    programId: 'prog-001',
    customerId: 'cust-123',
});

// Award bonus points
await toolkit.executeTool('earn_points', {
    programId: 'prog-001',
    customerId: 'cust-123',
    points: 500,
    reason: 'birthday',
    note: 'Birthday bonus',
});

// Check member status
const member = await toolkit.executeTool('get_loyalty_account', {
    programId: 'prog-001',
    customerId: 'cust-123',
});
// → member.result.account.pointsBalance, member.result.account.currentTier

// Redeem points
await toolkit.executeTool('redeem_points', {
    programId: 'prog-001',
    customerId: 'cust-123',
    points: 2000,
    orderId: 'ord-456',
    note: 'Apply loyalty discount',
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
await toolkit.executeTool('charge_gift_card', {
    giftCardId: card.result?.giftCard?.id,
    amount: 35.00,
    orderId: 'ord-456',
});

// Check remaining balance
const balance = await toolkit.executeTool('get_gift_card', {
    identifier: card.result?.giftCard?.id,
});
// → { balance: 65.00, redeemed: 35.00, transactions: [...] }
```

## Store Credits

Issue credits for refunds, goodwill gestures, or promotional campaigns.

```javascript
// Issue store credit
const credit = await toolkit.executeTool('create_store_credit', {
    customerId: 'cust-123',
    amount: 25.00,
    reason: 'goodwill',
    note: 'Late delivery compensation',
    expiresAt: '2026-06-17',
});

// Apply at checkout
await toolkit.executeTool('apply_store_credit', {
    creditId: credit.result?.credit?.id,
    orderId: 'ord-789',
    amount: 25.00,
});

// Check credit balance
const credits = await toolkit.executeTool('list_store_credits', {
    customerId: 'cust-123',
});
// → { returned: 1, credits: [{ currentBalance: 25.00, status: 'active', ... }] }
```

## Wishlists

Customers can create private, public, or shared wishlists.

```javascript
// Create a wishlist
const createdWishlist = await toolkit.executeTool('create_wishlist', {
    customerId: 'cust-123',
    name: 'Holiday Gift Ideas',
    visibility: 'shared',  // 'private', 'public', or 'shared'
});

const wishlistId = createdWishlist.result?.wishlist?.id;

// Add items
await toolkit.executeTool('add_to_wishlist', {
    wishlistId,
    productId: 'prod-456',
    note: 'Size medium, blue color',
    priority: 2,
});

// Convert to cart
await toolkit.executeTool('convert_wishlist_to_cart', {
    wishlistId,
    clearWishlist: true,
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
    orderId: 'ord-456',
});

// Moderate reviews
await toolkit.executeTool('approve_review', {
    reviewId: 'rev-001',
});

// Get product review summary
const reviews = await toolkit.executeTool('list_reviews', {
    productId: 'prod-456',
    status: 'approved',
    limit: 20,
});

const summary = await toolkit.executeTool('get_review_summary', {
    productId: 'prod-456',
});
// → summary.result.summary.averageRating, summary.result.summary.totalReviews
```

## Customer Segments

Dynamic segments evaluate membership in real-time based on conditions.

```javascript
// Create a segment with compound conditions
await toolkit.executeTool('create_segment', {
    name: 'High-Value Repeat Buyers',
    conditions: [
        { field: 'orderCount', operator: 'gte', value: 5 },
        { field: 'totalSpend', operator: 'gte', value: 500 },
        { field: 'email', operator: 'contains', value: '@example.com' },
    ],
    conditionLogic: 'all',
});

// Check segment membership
const membership = await toolkit.executeTool('evaluate_segment_membership', {
    segmentId: 'seg-001',
    customerId: 'cust-123',
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
| `starts_with` | String prefix match |

Compound conditions support `all` (AND) and `any` (OR) logic.

## MCP Tools

| Tool | Description |
|------|-------------|
| `create_loyalty_program` | Create a loyalty program with tiers |
| `get_loyalty_program` | Get program details |
| `enroll_customer` | Enroll customer in a loyalty program |
| `get_loyalty_account` | Check member status and points |
| `earn_points` | Award points to a loyalty account |
| `redeem_points` | Redeem points for rewards or discounts |
| `create_gift_card` | Issue a gift card |
| `get_gift_card` | Check gift card balance |
| `charge_gift_card` | Apply gift card balance to an order |
| `create_store_credit` | Issue store credit |
| `list_store_credits` | Check customer credit balances |
| `apply_store_credit` | Use credit at checkout |
| `create_wishlist` | Create a wishlist |
| `add_to_wishlist` | Add product to wishlist |
| `convert_wishlist_to_cart` | Convert wishlist to cart |
| `create_review` | Submit a product review |
| `approve_review` | Approve a review for publication |
| `reject_review` | Reject a review with a reason |
| `get_review_summary` | Get review metrics for a product |
| `list_reviews` | List reviews with filters |
| `create_segment` | Create a customer segment |
| `evaluate_segment_membership` | Check whether a customer belongs to a segment |
