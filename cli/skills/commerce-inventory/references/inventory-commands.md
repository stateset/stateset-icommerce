# Inventory Commands

## CLI (Natural Language)

- `stateset "stock level for WIDGET-001"`
- `stateset --apply "add 50 units of WIDGET-001 to inventory"`
- `stateset --apply "adjust inventory WIDGET-001 by -5 reason 'Damaged'"`
- `stateset --apply "reserve 5 units of WIDGET-001 for order ORD-123"`

## Reservation Flow

- Reserve -> Confirm -> Release
- Reservation holds reduce available stock until confirmed or released.

## Direct CLI

- `stateset-direct inventory stock <sku>`
- `stateset-direct inventory adjust <sku> <qty> <reason>`
