---
name: commerce-returns
description: Use when processing return requests, managing RMAs, or handling refund workflows.
---

# Commerce Returns Skill

Domain knowledge for return merchandise authorization (RMA) processing and refund workflows.

## Return Lifecycle

### Status Flow
```
[requested] ──► [approved] ──► [received] ──► [refunded]
      │
      └───────► [rejected]
```

### Status Definitions

| Status | Description | Next States |
|--------|-------------|-------------|
| `requested` | Customer initiated return | approved, rejected |
| `approved` | RMA approved, awaiting items | received, cancelled |
| `rejected` | Return denied | - |
| `received` | Items received at warehouse | refunded, exchanged |
| `refunded` | Refund processed | - |
| `exchanged` | Exchange fulfilled | - |
| `cancelled` | Return cancelled | - |

## Return Structure

```javascript
{
  id: "uuid",
  returnNumber: "RMA-12345",
  orderId: "order-uuid",

  status: "requested",
  reason: "defective",
  reasonDetails: "Screen has dead pixels",

  // Items being returned
  items: [
    {
      orderItemId: "item-uuid",
      sku: "WIDGET-001",
      name: "Premium Widget",
      quantity: 1,
      condition: "defective"
    }
  ],

  // Refund details
  refundAmount: 29.99,
  refundMethod: "original_payment",

  // Timestamps
  requestedAt: "2024-01-15T10:00:00Z",
  approvedAt: null,
  receivedAt: null,
  completedAt: null
}
```

## Return Reasons

| Reason Code | Description | Typical Action |
|-------------|-------------|----------------|
| `defective` | Product defect/malfunction | Full refund, no restock |
| `wrong_item` | Incorrect item shipped | Full refund + reship |
| `not_as_described` | Differs from listing | Full refund |
| `changed_mind` | Customer decision | Refund - restocking fee |
| `better_price_found` | Found cheaper | Refund - restocking fee |
| `no_longer_needed` | No longer wants | Refund - restocking fee |
| `damaged` | Arrived damaged | Full refund, file claim |
| `other` | Other reason | Case by case |

## Item Condition

| Condition | Description | Restockable |
|-----------|-------------|-------------|
| `new` | Unopened, unused | Yes |
| `like_new` | Opened but unused | Yes |
| `used` | Shows use | Maybe |
| `damaged` | Customer damaged | No |
| `defective` | Product defect | No |

## Return Policy Guidelines

### Standard Policy (Example)
- **Window**: 30 days from delivery
- **Condition**: Unused, in original packaging
- **Restocking fee**: 15% for buyer's remorse
- **Free returns**: Defective items, wrong item shipped

### Calculating Refund
```
Original Amount:     $99.99
- Restocking Fee:   -$15.00 (15%)
- Return Shipping:  -$8.99  (if customer pays)
= Refund Amount:     $76.00
```

### Exception Cases
- Defective: Full refund, no fees
- Wrong item: Full refund + free return shipping
- Damaged in transit: Full refund + carrier claim
- Final sale: No returns accepted

## Return Processing Workflow

### Customer Initiates Return
```
1. Customer contacts support
2. Verify order exists and eligible
3. Collect return reason
4. Create return request
5. Provide RMA number
```

### Review & Approve
```
1. Review return request
2. Check return policy eligibility
3. Verify reason is valid
4. Calculate refund amount
5. Approve or reject with reason
```

### Process Return
```
1. Customer ships items back
2. Receive and inspect items
3. Update condition assessment
4. Process refund
5. Restock if applicable
```

## Rejection Reasons

When rejecting a return:
- Outside return window
- Item shows customer damage
- Item not from this order
- Final sale item
- Missing components/packaging
- Hygiene/safety item used

Always document clearly:
```
"Item shows signs of customer damage (scratches on surface)
not covered under warranty. Return window for change of mind
items is 30 days, this request is 45 days from delivery."
```

## Refund Methods

| Method | Description | Timeline |
|--------|-------------|----------|
| `original_payment` | Credit to original card | 3-5 days |
| `store_credit` | Account credit | Immediate |
| `exchange` | Ship replacement | Upon receipt |
| `check` | Mail check | 7-10 days |

## Inventory Impact

### On Return Receipt
If restockable:
```
adjust_inventory(sku, +quantity, "Return RMA-12345")
```

If not restockable:
```
// No inventory adjustment
// Document as loss/damage
```

### On Exchange
```
1. Process return item (may restock)
2. Ship replacement (reserve + fulfill)
```

## Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `Order not found` | Invalid order ID | Verify order number |
| `Already returned` | Item has existing return | Check return status |
| `Outside window` | Past return deadline | Review for exception |
| `Invalid quantity` | More than purchased | Check order items |

## Metrics to Track

| Metric | Description |
|--------|-------------|
| Return rate | Returns / Orders |
| Reason breakdown | Count by reason |
| Processing time | Request to complete |
| Refund amount | Total refunded |
| Restock rate | Items restocked |

## Best Practices

1. **Respond quickly** - Acknowledge within 24 hours
2. **Be clear about policy** - Set expectations upfront
3. **Document everything** - Photos, condition notes
4. **Make exceptions wisely** - Balance customer vs cost
5. **Track patterns** - Identify product issues
6. **Simplify process** - Easy returns = customer loyalty
