# Fraud Detection

iCommerce includes built-in fraud assessment for orders. Risk scoring combines geo-mismatch detection, device fingerprinting, IP analysis, and configurable rules to produce actionable fraud verdicts.

## Assessing an Order

```javascript
const assessment = await toolkit.executeTool('assess_order_fraud', {
    orderId: 'ord-123',
    customerIp: '203.0.113.42',
    deviceFingerprint: 'fp_abc123',
    billingAddress: { country: 'US', region: 'CA', postalCode: '90210' },
    shippingAddress: { country: 'US', region: 'NY', postalCode: '10001' },
});
// → {
//     riskScore: 72,
//     riskLevel: 'high',
//     recommendation: 'review',
//     signals: [
//         { type: 'geo_mismatch', description: 'Billing CA, shipping NY', weight: 25 },
//         { type: 'new_customer', description: 'First order from this customer', weight: 15 },
//         { type: 'high_value', description: 'Order total exceeds $500', weight: 20 },
//     ],
//     matchedRules: ['geo-mismatch-flag', 'high-value-review'],
// }
```

## Risk Levels

| Risk Level | Score Range | Default Action |
|------------|-------------|----------------|
| `low` | 0–30 | Auto-approve |
| `medium` | 31–60 | Flag for review |
| `high` | 61–85 | Hold for manual review |
| `critical` | 86–100 | Auto-decline |

## Fraud Signals

| Signal | Weight | Trigger |
|--------|--------|---------|
| `geo_mismatch` | 20–30 | Billing and shipping addresses in different regions/countries |
| `ip_country_mismatch` | 25 | Customer IP geolocates to different country than billing |
| `new_customer` | 10–15 | Customer has no prior order history |
| `high_value` | 15–25 | Order total exceeds configurable threshold |
| `velocity` | 20–30 | Multiple orders from same IP/device in short window |
| `device_fingerprint` | 15–20 | Known fraudulent device fingerprint |
| `email_domain` | 5–10 | Disposable email domain |

Signals are additive — the risk score is the sum of all matched signal weights.

## Fraud Rules

Create custom rules to flag specific patterns:

```javascript
// Create a fraud rule
await toolkit.executeTool('create_fraud_rule', {
    name: 'international-high-value',
    description: 'Flag international orders over $300',
    conditions: [
        { field: 'shipping_country', operator: 'neq', value: 'US' },
        { field: 'order_total', operator: 'gt', value: 300 },
    ],
    action: 'review',
    weight: 30,
    enabled: true,
});

// List active rules
const rules = await toolkit.executeTool('list_fraud_rules', {});

// Disable a rule
await toolkit.executeTool('update_fraud_rule', {
    ruleId: 'rule-001',
    enabled: false,
});
```

## Integration with Payments

Fraud assessment integrates with the payment flow:

```
Order created
  │
  ├─ assess_order_fraud() → risk score
  │
  ├─ risk = low     → auto-process payment
  ├─ risk = medium  → process with flag for post-review
  ├─ risk = high    → hold payment, notify for review
  └─ risk = critical → decline payment, notify customer
```

Use policies to automate this flow:

```yaml
# policies/payments.yaml
name: Payment Fraud Gate
domain: payments
rules:
  - name: block-critical-fraud
    conditions:
      - field: fraud_risk_level
        operator: equals
        value: "critical"
    actions:
      - type: deny
        reason: "Order flagged as critical fraud risk"
```

## Chargeback Tracking

```javascript
// Record a chargeback
await toolkit.executeTool('record_chargeback', {
    orderId: 'ord-123',
    amount: 149.99,
    reason: 'unauthorized_transaction',
    evidenceDeadline: '2026-04-01',
});

// Submit evidence
await toolkit.executeTool('submit_chargeback_evidence', {
    chargebackId: 'cb-001',
    evidence: {
        deliveryProof: 'Signed by recipient on 2026-03-10',
        trackingNumber: 'FEDEX-12345',
        customerCorrespondence: 'Customer confirmed receipt via email',
    },
});
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `assess_order_fraud` | Run fraud assessment with risk score |
| `get_fraud_assessment` | Get assessment details by ID |
| `create_fraud_rule` | Create a custom fraud rule |
| `list_fraud_rules` | List active fraud rules |
| `update_fraud_rule` | Enable/disable/modify a rule |
| `record_chargeback` | Record a chargeback event |
| `submit_chargeback_evidence` | Submit evidence for dispute |
