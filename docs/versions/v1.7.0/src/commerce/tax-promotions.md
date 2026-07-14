# Tax & Promotions

## Tax Engine

Multi-jurisdiction tax calculation supporting US state sales tax, EU VAT, and Canadian GST/PST/HST.

### Tax Rates

```javascript
// Get effective rate for a jurisdiction
const rate = commerce.tax.getEffectiveRate('US', 'CA', 'general');
// → { rate: 0.0725, jurisdiction: 'California', type: 'state' }
```

### Tax Exemptions

```javascript
const exemption = commerce.tax.createExemption(
    customer.id,
    'resale',          // Exemption type
    '2026-01-01'       // Effective date
);
```

### Nexus Detection

```javascript
// Check if you have tax nexus in a state
const nexus = await toolkit.executeTool('check_tax_nexus', {
    country: 'US',
    state: 'TX'
});
```

### MCP Tax Tools

| Tool | Description |
|------|-------------|
| `get_tax_rate` | Get rate for jurisdiction |
| `calculate_tax` | Calculate tax for an order |
| `create_tax_exemption` | Create an exemption |
| `list_tax_exemptions` | List active exemptions |
| `check_tax_nexus` | Check nexus status |

## Promotions & Coupons

### Create a Promotion

```javascript
const promo = commerce.promotions.create({
    code: 'SUMMER20',
    name: 'Summer Sale',
    discountType: 'percentage',   // or 'fixed'
    discountValue: 20,            // 20% off
    startDate: '2026-06-01',
    endDate: '2026-08-31'
});
```

### Activate and Deactivate

```javascript
commerce.promotions.activate(promo.id);
commerce.promotions.deactivate(promo.id);
```

### Coupons

```javascript
// Create a limited-use coupon
const coupon = commerce.promotions.createCoupon(promo.id, 'SAVE20NOW', 100);
// max 100 uses

// Validate before applying
const valid = commerce.promotions.validateCoupon('SAVE20NOW');
```

### MCP Promotion Tools

| Tool | Description |
|------|-------------|
| `list_promotions` | List all promotions |
| `create_promotion` | Create a promotion |
| `activate_promotion` | Activate a promotion |
| `deactivate_promotion` | Deactivate a promotion |
| `create_coupon` | Create a coupon code |
| `validate_coupon` | Check if coupon is valid |
| `apply_promotion` | Apply to an order/cart |

### Promotion Stacking Rules

By default, only one promotion can apply per order. To allow stacking:

```javascript
// Mark a promotion as stackable
const promo = commerce.promotions.create({
    code: 'LOYALTY10',
    name: 'Loyalty Discount',
    discountType: 'percentage',
    discountValue: 10,
    stackable: true          // Can combine with other stackable promotions
});
```

Stacking rules:
- **Non-stackable promotions**: Only the highest-value discount applies
- **Stackable promotions**: Applied sequentially (percentage discounts compound)
- **Exclusive promotions**: Prevent all other discounts when applied

### Minimum Order Requirements

```javascript
const promo = commerce.promotions.create({
    code: 'FREESHIP50',
    name: 'Free Shipping Over $50',
    discountType: 'fixed',
    discountValue: 9.99,          // Shipping cost
    minimumOrderAmount: 50.00,    // Only applies to orders ≥ $50
    applicableCategories: ['shipping']
});
```

### Promotion Budget Caps

Limit total discount value across all uses:

```javascript
const promo = commerce.promotions.create({
    code: 'SUMMER20',
    name: 'Summer Sale',
    discountType: 'percentage',
    discountValue: 20,
    maxTotalDiscount: 5000.00    // Stop after $5,000 in total discounts
});
```

## Shipping Zones

Define shipping rates by geographic zone:

```javascript
await toolkit.executeTool('create_shipping_zone', {
    name: 'West Coast',
    countries: ['US'],
    states: ['CA', 'OR', 'WA'],
    rates: [
        { carrier: 'USPS', method: 'ground', price: 5.99 },
        { carrier: 'FedEx', method: 'express', price: 14.99 }
    ]
});
```
