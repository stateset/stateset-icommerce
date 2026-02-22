# Inventory Management

## Stock Types

Inventory quantities are tracked in distinct buckets:

- **Available** — quantity that can be sold right now (on_hand − reserved)
- **Reserved** — quantity held for pending orders but not yet deducted
- **Committed** — quantity confirmed for fulfillment (reservation confirmed)
- **In-transit** — quantity on the way from supplier; not yet received
- **On-hand** — total physical quantity in the warehouse

```
Available = On-hand − Reserved − Committed
```

## Reservation Workflow

Reservations prevent overselling during the window between cart creation and order confirmation.

```
reserve_inventory      → status: reserved
confirm_reservation    → status: committed (deducts from on-hand)
release_reservation    → cancels reservation (e.g. cart abandoned)
```

1. When a customer adds to cart: `reserve_inventory` (soft lock)
2. When checkout completes: `confirm_reservation` (hard deduct)
3. If cart is abandoned or order cancelled: `release_reservation`

## Low Stock Alerts and Reorder Points

- Each inventory item has an optional `reorder_point` threshold
- `get_low_stock_items` returns items where available ≤ reorder_point
- `get_inventory_health` gives a dashboard view: total SKUs, in-stock count, low-stock count, out-of-stock count
- The heartbeat monitor can emit alerts when stock drops below threshold

## Inventory Adjustments

- `adjust_inventory` — add or subtract units (positive = receive, negative = shrinkage/write-off)
- Always include a `reason` (received_shipment, sold, damaged, counted, transferred)
- Adjustments are logged for audit purposes

## Lot Tracking

For perishables and regulated goods, inventory is tracked by lot number:
- Lots have expiry dates and manufacture dates
- FIFO (first-in, first-out) or FEFO (first-expired, first-out) picking rules apply
- `list_lots` and `get_lot` provide lot-level visibility

## Serial Number Management

For high-value items that need individual unit tracking:
- Each unit gets a unique serial number at receiving
- Serial numbers are assigned to orders at pick time
- Returns track the specific serial numbers being returned

## Demand Forecasting

- `get_demand_forecast` — predicts quantity needed per SKU over a future period
- `get_revenue_forecast` — predicts future revenue based on historical trends
- Useful for purchase order planning and safety stock calculations
