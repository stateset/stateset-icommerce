# Shipments Flow

## Statuses

created -> in_transit -> out_for_delivery -> delivered

## Common Commands

- `stateset --apply "create shipment for order ORD-123 carrier fedex tracking 7946..."`
- `stateset --apply "ship order ORD-123 with tracking TRACK123"`
- `stateset --apply "deliver shipment SHIP-123"`
