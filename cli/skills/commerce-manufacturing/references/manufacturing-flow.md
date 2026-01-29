# Manufacturing Flow

## BOM Steps

1. Create BOM for a product
2. Add components
3. Activate BOM

## Work Orders

- draft -> scheduled -> in_progress -> completed

## Common Commands

- `stateset --apply "create bom for SKU WIDGET-DELUXE"`
- `stateset --apply "add component PART-001 qty 2 to bom BOM-123"`
- `stateset --apply "create work order from BOM-123 qty 100"`
- `stateset --apply "start work order WO-123"`
- `stateset --apply "complete work order WO-123"`
