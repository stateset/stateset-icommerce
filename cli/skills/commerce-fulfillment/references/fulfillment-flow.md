# Fulfillment Flow Reference

## Pick-Pack-Ship Pipeline

```
Orders
  └─ Wave Planning (group orders)
       └─ Pick Tasks (retrieve items from locations)
            └─ Pack Tasks (package into cartons)
                 └─ Ship Tasks (label and hand off to carrier)
```

## Wave Planning

Waves group orders for efficient warehouse processing:

| Wave Type | Use Case |
|-----------|----------|
| Batch | Standard grouping of multiple orders |
| Priority | Rush/expedited orders |
| Zone | Orders fulfilled from same warehouse zone |
| Single | Individual order processing |

## Pick Task Fields

- `wave_id`: Parent wave
- `order_id`, `order_item_id`: Source order reference
- `sku`, `product_name`: Item to pick
- `location_id`: Warehouse location to pick from
- `quantity_requested` / `quantity_picked`: Expected vs actual
- `picked_by`: Worker who completed the pick

## Short Picks

When `quantity_picked < quantity_requested`:
1. Pick status becomes `Short`
2. Investigate location inventory
3. Options: find alternate location, backorder, or cancel line

## Pack Task / Cartons

Each pack task can contain multiple cartons:
- Carton has dimensions (length, width, height) and weight
- CartonItems link picked items to specific cartons
- Package types: Box, Envelope, Tube, Pallet, Custom

## Ship Task

Final handoff to carrier:
- Carrier and service level
- Tracking number
- Label generation
- Ship date recording

## Common Operations

```bash
stateset --apply "create wave type batch for orders ORD-001 ORD-002 ORD-003"
stateset --apply "release wave WAVE-001"
stateset --apply "complete pick PICK-001 quantity 10"
stateset --apply "add carton to pack PACK-001 dimensions 12x10x8 weight 5.2"
stateset --apply "complete ship SHIP-001 carrier UPS tracking 1Z999AA10123456784"
```
