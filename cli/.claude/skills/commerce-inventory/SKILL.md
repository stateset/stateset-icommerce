---
name: commerce-inventory
description: Use when managing stock levels, inventory adjustments, or reservation workflows.
---

# Commerce Inventory Skill

Domain knowledge for inventory management, stock tracking, and reservation systems.

## Inventory Concepts

### Stock Quantities

| Quantity | Description | Formula |
|----------|-------------|---------|
| `onHand` | Physical units in warehouse | Actual count |
| `allocated` | Reserved for orders | Sum of reservations |
| `available` | Can be sold | onHand - allocated |

### Key Formula
```
Available = On-Hand - Allocated
```

### Example
```
On-Hand:    100 units (physical inventory)
Allocated:   25 units (reserved for orders)
Available:   75 units (can be sold)
```

## Inventory Item Structure

```javascript
{
  id: "uuid",
  sku: "WIDGET-001",
  name: "Premium Widget",
  description: "High quality widget",

  // Quantities
  totalOnHand: 100,
  totalAllocated: 25,
  totalAvailable: 75,

  // Thresholds
  reorderPoint: 20,
  reorderQuantity: 50,

  // Location breakdown
  locations: [
    { locationId: "warehouse-1", onHand: 60, allocated: 15 },
    { locationId: "warehouse-2", onHand: 40, allocated: 10 }
  ],

  createdAt: "2024-01-15T10:30:00Z",
  updatedAt: "2024-01-15T12:00:00Z"
}
```

## Inventory Operations

### Adjustments

| Type | Quantity | Use Case |
|------|----------|----------|
| Positive | +N | Received shipment, found inventory |
| Negative | -N | Damaged, lost, shrinkage |

Always document reason:
- "Received shipment PO-12345"
- "Damaged in warehouse - 3 units"
- "Cycle count correction"
- "Promotional giveaway"

### Reservations

Reservation lifecycle:
```
[Available] ──► [Reserved] ──► [Confirmed] (deducted)
                    │
                    └──────────► [Released] (returned)
```

| Action | Effect |
|--------|--------|
| Reserve | Available ↓, Allocated ↑ |
| Confirm | Allocated ↓, OnHand ↓ |
| Release | Allocated ↓, Available ↑ |

## Reservation Structure

```javascript
{
  id: "uuid",
  sku: "WIDGET-001",
  quantity: 5,

  referenceType: "order",
  referenceId: "order-uuid",

  status: "pending",  // pending, confirmed, released, expired

  expiresAt: "2024-01-15T11:00:00Z",
  createdAt: "2024-01-15T10:45:00Z"
}
```

## Transaction Types

| Type | Description |
|------|-------------|
| `adjustment_in` | Manual increase |
| `adjustment_out` | Manual decrease |
| `sale` | Sold (order fulfilled) |
| `return` | Returned from customer |
| `transfer_in` | Received from location |
| `transfer_out` | Sent to location |
| `reservation` | Stock reserved |
| `release` | Reservation released |

## Stock Level Alerts

| Level | Condition | Action |
|-------|-----------|--------|
| `ok` | Available > reorderPoint | Normal |
| `low` | Available ≤ reorderPoint | Consider reorder |
| `critical` | Available ≤ reorderPoint/2 | Urgent reorder |
| `out` | Available = 0 | Stop selling |

## Common Workflows

### Receive Inventory
```
1. get_stock(sku) - Check current levels
2. adjust_inventory(sku, +quantity, "Received PO-12345")
3. Verify new levels
```

### Order Fulfillment
```
1. reserve_inventory(sku, qty, "order", orderId)
2. [Process order...]
3. confirm_reservation(reservationId)
   OR
   release_reservation(reservationId) if cancelled
```

### Cycle Count
```
1. get_stock(sku) - Get system count
2. Physical count
3. If different:
   adjust_inventory(sku, difference, "Cycle count correction")
```

### Write Off Damaged
```
1. get_stock(sku) - Check current
2. adjust_inventory(sku, -damaged, "Damaged - [details]")
3. Document with photos (external)
```

## Multi-Location Inventory

When using multiple warehouses:

```javascript
// Total stock
totalOnHand: 100

// By location
locations: [
  { locationId: "east", onHand: 60 },
  { locationId: "west", onHand: 40 }
]
```

Operations can specify location:
- Reserve from specific location
- Adjust specific location
- Transfer between locations

## Reservation Expiration

Default: 15 minutes (900 seconds)

Expired reservations:
- Automatically released
- Stock returns to available
- Associated order may need attention

Custom expiration:
```javascript
reserve_inventory({
  sku: "WIDGET-001",
  quantity: 5,
  referenceType: "order",
  referenceId: "uuid",
  expiresInSeconds: 3600  // 1 hour
})
```

## Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `SKU not found` | Invalid SKU | Create inventory item |
| `Insufficient stock` | Not enough available | Check levels, adjust |
| `Reservation expired` | Timed out | Create new reservation |
| `Already confirmed` | Duplicate confirm | No action needed |
| `Negative stock` | Over-adjustment | Verify physical count |

## Best Practices

1. **Always check before adjust** - View current levels first
2. **Document all changes** - Include reason in every adjustment
3. **Use reservations** - Don't directly deduct for orders
4. **Monitor reorder points** - Set appropriate thresholds
5. **Regular cycle counts** - Verify physical vs system
6. **Handle expiration** - Clean up expired reservations
