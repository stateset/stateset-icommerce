# Warehouse & Location Reference

## Warehouse Types

| Type | Use Case |
|------|----------|
| Distribution | General fulfillment center |
| Manufacturing | Production facility |
| Retail | Store-attached warehouse |
| ThirdParty | 3PL managed facility |
| Consignment | Vendor-owned stock |
| Returns | Reverse logistics center |

## Location Hierarchy

```
Warehouse
  └─ Zone (e.g., ZONE-A)
       └─ Aisle (e.g., A1)
            └─ Rack (e.g., A1-R01)
                 └─ Level/Bin (e.g., A1-R01-B03)
```

## Location Types

| Type | Purpose |
|------|---------|
| Bulk | Long-term high-volume storage |
| Pick | Active picking face |
| Staging | Temporary hold before shipping |
| Receiving | Inbound dock area |
| Shipping | Outbound dock area |
| Quarantine | Quality hold area |
| Returns | Returned goods processing |
| Production | Manufacturing floor |
| Packing | Pack stations |
| CrossDock | Direct transfer, no storage |

## Movement Types

| Type | Description |
|------|-------------|
| Receipt | Goods received into location |
| Transfer | Move between locations |
| Pick | Picked for fulfillment |
| Adjustment | Cycle count or correction |
| Shipment | Shipped out of location |
| Return | Returned goods placed |

## Location Inventory Fields

- `on_hand`: Total physical quantity
- `reserved`: Quantity reserved for orders
- `available`: on_hand minus reserved
- `min_quantity` / `max_quantity`: Replenishment thresholds

## Common Operations

```bash
stateset --apply "create warehouse WH-EAST type distribution"
stateset --apply "create location ZONE-A type bulk in warehouse WH-EAST"
stateset --apply "move 25 WIDGET-001 from LOC-A1-01 to LOC-B2-03"
stateset --apply "adjust location LOC-A1-01 SKU WIDGET-001 by -5 reason cycle_count"
```
