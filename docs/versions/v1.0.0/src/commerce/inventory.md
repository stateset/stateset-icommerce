# Inventory & Warehousing

Inventory management tracks stock levels, handles reservations for in-progress orders, and supports multi-location warehousing.

## Core Concepts

- **Inventory Item**: A SKU with a current stock level
- **Reservation**: A temporary hold on stock for a pending order (prevents overselling)
- **Adjustment**: A tracked change to stock level with a reason (receiving, sale, damage, audit)
- **Reorder Point**: The threshold below which the heartbeat monitor triggers a low-stock alert

## Operations

### Create an Inventory Item

```javascript
const item = commerce.inventory.createItem({
    sku: 'WIDGET-001',
    name: 'Premium Widget',
    initialQuantity: 100
});
```

### Adjust Inventory

```javascript
// Positive adjustment (receiving)
commerce.inventory.adjust('WIDGET-001', 50, 'Received shipment #PO-456');

// Negative adjustment (damage)
commerce.inventory.adjust('WIDGET-001', -3, 'Damaged in warehouse');
```

### Reserve and Release

```javascript
// Reserve stock for an order (prevents overselling)
const reservation = commerce.inventory.reserve('WIDGET-001', 10);

// Release if order is cancelled
commerce.inventory.release(reservation.id);
```

### Check Stock Levels

```javascript
const level = commerce.inventory.getLevel('WIDGET-001');
console.log(`Available: ${level.available}`);
console.log(`Reserved: ${level.reserved}`);
console.log(`On hand: ${level.onHand}`);
```

### Batch Adjustments

```rust
// More efficient than individual adjustments — single transaction
let results = commerce.inventory().batch_adjust(vec![
    BatchAdjustment { sku: "SKU-001".into(), delta: 10, reason: "restock".into() },
    BatchAdjustment { sku: "SKU-002".into(), delta: -5, reason: "sale".into() },
    BatchAdjustment { sku: "SKU-003".into(), delta: 20, reason: "restock".into() },
])?;
```

## Warehousing

For multi-location operations, the warehouse API provides:

- **Locations**: Physical warehouses or stores
- **Zones**: Areas within a warehouse (receiving, picking, packing, shipping)
- **Bins**: Specific storage positions
- **Transfers**: Move inventory between locations

## Lot and Serial Tracking

For regulated industries (food, pharmaceuticals, electronics):

```javascript
// Lot tracking
const lot = commerce.lots.create({
    sku: 'PHARMA-001',
    lotNumber: 'LOT-2026-03',
    expirationDate: '2027-03-16',
    quantity: 500
});

// Serial number tracking
commerce.serials.assign({
    sku: 'DEVICE-001',
    serialNumber: 'SN-ABC-12345',
    lotId: lot.id
});
```

## Heartbeat Low-Stock Alerts

The [heartbeat monitor](../guides/heartbeat.md) can automatically check for low stock:

```json
{
    "id": "low-stock",
    "checker": "low-stock",
    "intervalMs": 3600000,
    "enabled": true,
    "config": { "threshold": 10 }
}
```

When triggered, alerts flow through the EventBridge to Slack, Telegram, Discord, or any configured channel.

## MCP Tools

| Tool | Description |
|------|-------------|
| `list_inventory` | List all inventory items |
| `get_inventory_level` | Get stock level for a SKU |
| `adjust_inventory` | Adjust stock with reason |
| `reserve_inventory` | Create a reservation |
| `release_inventory` | Release a reservation |
| `create_inventory_item` | Add a new SKU to inventory |
