# Backorder Management Reference

## Backorder Lifecycle

```
Out of Stock
  └─ Backorder Created (Pending)
       └─ Inventory Arrives
            ├─ Partial → PartiallyFulfilled
            └─ Full → Allocated
                 └─ ReadyToShip
                      └─ Fulfilled (complete)
```

## Backorder Fields

- `backorder_number`: Auto-generated (BO-YYYY-NNNN)
- `order_id`, `order_item_id`: Original order reference
- `customer_id`: Customer waiting for fulfillment
- `sku`, `product_name`: Item backordered
- `quantity_ordered`: Total quantity needed
- `quantity_fulfilled`: Quantity shipped so far
- `quantity_remaining`: Still outstanding
- `priority`: Low, Normal, High, Critical
- `expected_date`: When stock is expected
- `promised_date`: Date promised to customer

## Allocation

Reserve incoming inventory for specific backorders:
- `allocation_id`: Unique allocation reference
- `quantity`: Amount reserved
- `source_type`: Where inventory is coming from
- `source_id`: PO number, transfer ID, or work order
- `expiration`: Auto-release if not confirmed

**Allocation Statuses:** Reserved -> Confirmed -> Released/Expired

## Fulfillment Sources

| Source | Description |
|--------|-------------|
| Inventory | Existing warehouse stock becomes available |
| PurchaseOrder | Supplier PO received |
| Transfer | Stock transferred from another warehouse |
| Production | Manufacturing work order completed |

## Priority Processing

Process backorders in priority order:
1. **Critical** - Customer escalation, SLA breach
2. **High** - Important customer, time-sensitive
3. **Normal** - Standard processing (default)
4. **Low** - Non-urgent, can wait

## Reporting

### SKU Backorder Summary
- Total quantity backordered across all orders
- Number of affected orders and customers
- Earliest expected date

### Backorder Summary
- Total backorders by status
- Overdue backorders (past promised date)
- Average days outstanding
