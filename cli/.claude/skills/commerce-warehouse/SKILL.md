---
name: commerce-warehouse
description: Use when running warehouse operations — locations, receiving, waves and picking, cycle counts, transfers, or lot/serial traceability.
---

# Commerce Warehouse Skill

Domain knowledge for WMS operations: physical layout, inbound and outbound
flows, cycle counting, transfers, and traceability.

## Warehouse & Location Hierarchy

```
Warehouse (code, name, timezone)
   └── Location: zone ► aisle ► rack ► bin
                 flags: isPickable, isReceivable
```

- `create_warehouse` / `list_warehouses` / `get_warehouse` — warehouse master data
- `create_location` — optional `zone`, `aisle`, `rack`, `bin` plus
  `isPickable` / `isReceivable` flags
- `list_locations` / `get_location` — layout review
- `list_pickable_locations` — pickable locations holding a SKU in a warehouse
- `get_warehouse_sku_available_quantity` — location-level availability

Flags drive flow: receiving puts stock into **receivable** locations; waves
pick from **pickable** ones. A staging bin is often receivable but not pickable.

## Inbound Flow

```
inbound shipment ──► receipt ──► put-away into locations
```

### Inbound Shipments (from suppliers / 856 ASN)
```
create_inbound_shipment ──► mark_inbound_shipment_in_transit
        ──► mark_inbound_shipment_arrived ──► receive_inbound_shipment_line
```
- `cancel_inbound_shipment` — blocked once received or already cancelled (terminal-state guard)
- `check_inbound_shipments_supported` — feature probe before using the domain

### Receipts (against POs)
```
create_receipt_from_purchase_order   # lines seeded from PO quantities
        ──► start_receiving ──► complete_receiving
```
- `create_receipt` for ad-hoc (non-PO) receipts
- `cancel_receipt` — only before receiving completes; a receipt already in
  inspection/put-away cannot be cancelled
- Put-away (moving received stock into bins) is exposed on the HTTP receiving
  API; location-level adjust/move operations on the warehouse HTTP API land
  stock in its final bin
- The completed receipt is one leg of the AP **3-way match** (PO ↔ receipt ↔ bill)

## Outbound Flow

```
create_fulfillment_wave ──► release_fulfillment_wave ──► pick ──► pack ──► ship
                                     │
                                     └──► cancel_fulfillment_wave
```

### Waves
- `create_fulfillment_wave` — batch orders for picking
- `release_fulfillment_wave` — generates pick tasks
- `complete_fulfillment_wave` — all picks done

### Pick Tasks
```
list_pick_tasks ──► assign_pick_task ──► start_pick_task ──► [complete]
                                                │
                                                └──► cancel_pick_task
```
Pick completion is idempotent with an over-pick guard — completing an
already-completed/short pick is a no-op, and you cannot pick more than tasked.

### Pack & Ship Gates
- `check_order_ready_to_pack` — all picks complete?
- `check_order_ready_to_ship` — packed into cartons, ready for carrier?
- Carton/pack endpoints live on the HTTP fulfillment API; shipping uses
  `create_shipment` / `ship_order` from the shipments/orders domains

## Cycle Counts

```
create_cycle_count (draft) ──► start_cycle_count (in-progress)
        ──► record_cycle_counts ──► complete_cycle_count (completed)
                                            │
              cancel_cycle_count ◄──────────┘ (only before completion)
```

- `record_cycle_counts` — counted quantities per location/SKU
- `complete_cycle_count` — **transactionally applies variances** to location
  inventory and writes `cycle_count` movement audit records; fires a
  domain event
- `completed` and `cancelled` are terminal — a completed count cannot be
  reopened; create a new count instead

## Transfer Orders (inter-warehouse)

```
create_transfer_order ──► ship_transfer_order ──► receive_transfer_order_line
                                  │
                                  └──► cancel_transfer_order (not after received/cancelled)
```
- `check_transfer_orders_supported` — feature probe
- Receive line-by-line at the destination; received and cancelled are terminal

## Lots & Serials (Traceability)

### Lots
- `create_lot` — batch with expiry; `list_active_lots`, `list_available_lots_for_sku`
- Expiry management: `list_expiring_lots` / `list_expired_lots`
- Holds: `quarantine_lot` ⇄ `release_lot_quarantine`; `list_quarantined_lots`

### Serials
- `create_serial` — unit-level tracking; `list_available_serials`,
  `check_serial_availability`
- `mark_serial_sold` — on fulfillment; `quarantine_serial` for holds

## Inventory Adjust/Move Semantics

- Location-level adjust and move operations (warehouse HTTP API; the
  `adjust_inventory` tool covers item-level adjustments) operate per
  (location, SKU, lot)
- **Lot-less convention**: stock not lot-tracked uses the empty-string lot
  key consistently — a lot-less adjustment and a lot-less move address the
  same row. Do not invent placeholder lot numbers for untracked SKUs
- Every adjustment needs a documented reason; cycle-count variances are
  applied by `complete_cycle_count`, not by manual adjustments

## Best Practices

1. **Probe support first** — `check_*_supported` before transfer/inbound work
2. **Respect terminal states** — received, completed, and cancelled documents are final
3. **Count in draft, apply on complete** — record all counts before completing; completion posts variances atomically
4. **FEFO for lots** — pick from `list_available_lots_for_sku` ordered by expiry
5. **Gate before ship** — always run the ready-to-pack/ready-to-ship checks
6. **Quarantine, don't adjust out** — quality issues get holds, not silent stock removal

## Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `Cannot cancel` | Document already received/cancelled | Terminal state — create a reversing document |
| `Wave not released` | Picking before release | `release_fulfillment_wave` first |
| `Not ready to pack` | Open pick tasks | Complete or cancel remaining picks |
| `Count not in progress` | Recording against a draft/completed count | `start_cycle_count` first |
| `Lot quarantined` | Picking held stock | `release_lot_quarantine` or pick another lot |
