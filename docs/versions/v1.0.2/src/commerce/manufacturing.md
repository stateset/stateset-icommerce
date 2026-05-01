# Manufacturing & Supply Chain

iCommerce includes full manufacturing and supply chain management: bills of materials, work orders, quality control, purchase orders, and supplier management.

## Bills of Materials (BOM)

Define the components needed to build a product:

```javascript
const bom = commerce.bom.create({
    productId: product.id,
    name: 'Widget Assembly',
    version: '1.0',
    items: [
        { sku: 'PART-A', name: 'Housing', quantity: 1 },
        { sku: 'PART-B', name: 'Circuit Board', quantity: 1 },
        { sku: 'PART-C', name: 'Screw Set', quantity: 4 }
    ]
});
```

### BOM Explosion

Calculate total material requirements for a production run:

```bash
stateset "explode BOM for 500 Premium Widgets"
```

## Work Orders

Track manufacturing jobs from draft through completion:

```
Draft → Scheduled → In Progress → Completed
                       └────────→ On Hold → In Progress
```

```javascript
const workOrder = commerce.workOrders.create({
    bomId: bom.id,
    quantity: 500,
    scheduledDate: '2026-04-01'
});

commerce.workOrders.start(workOrder.id);
commerce.workOrders.complete(workOrder.id, { yieldQuantity: 498 });
```

## Quality Control

```javascript
const inspection = commerce.quality.createInspection({
    workOrderId: workOrder.id,
    type: 'final',
    criteria: [
        { name: 'Visual', target: 'No scratches', result: 'pass' },
        { name: 'Dimension', target: '10.0mm ± 0.1', result: 'pass' }
    ]
});
```

## Purchase Orders

Manage procurement from suppliers:

```javascript
const po = commerce.purchaseOrders.create({
    supplierId: supplier.id,
    items: [
        { sku: 'PART-A', quantity: 1000, unitCost: 2.50 }
    ],
    expectedDelivery: '2026-03-25'
});

commerce.purchaseOrders.approve(po.id);
commerce.purchaseOrders.receive(po.id, { receivedQuantity: 995 });
```

## Supplier Management

```javascript
const supplier = commerce.suppliers.create({
    name: 'Acme Parts Co.',
    email: 'orders@acmeparts.com',
    leadTimeDays: 14
});
```

## Traceability

Track the complete genealogy of manufactured products:

```javascript
// Create lot during work order completion
const lot = commerce.lots.create({
    sku: 'WIDGET-001',
    lotNumber: 'LOT-2026-03-A',
    workOrderId: workOrder.id,
    quantity: 498,
    expirationDate: '2027-03-16'
});

// Assign serial numbers to individual units
for (let i = 1; i <= 498; i++) {
    commerce.serials.assign({
        sku: 'WIDGET-001',
        serialNumber: `SN-WGT-${String(i).padStart(5, '0')}`,
        lotId: lot.id
    });
}

// Later: trace a serial number back to its work order
const serial = commerce.serials.get('SN-WGT-00042');
// → { sku: 'WIDGET-001', lotId: 'lot-123', lotNumber: 'LOT-2026-03-A' }
const lot = commerce.lots.get(serial.lotId);
// → { workOrderId: 'wo-456', quantity: 498 }
```

## Cost Rollup

Track total manufacturing cost per work order:

```javascript
const cost = commerce.costAccounting.getWorkOrderCost(workOrder.id);
// → {
//     materialCost: 6250.00,   // BOM components
//     laborCost: 2500.00,      // Labor hours × rate
//     overhead: 1250.00,       // Allocated overhead
//     totalCost: 10000.00,
//     costPerUnit: 20.08       // totalCost / yieldQuantity
// }
```

## QC Failure Recovery

When a quality inspection fails:

```javascript
// Inspection fails
const inspection = commerce.quality.createInspection({
    workOrderId: workOrder.id,
    type: 'final',
    result: 'fail',
    criteria: [
        { name: 'Dimension', target: '10.0mm ± 0.1', actual: '10.3mm', result: 'fail' }
    ]
});

// Option 1: Rework — put the work order back to In Progress
commerce.workOrders.rework(workOrder.id, { reason: 'Dimension out of spec' });

// Option 2: Scrap — write off the units
commerce.inventory.adjust(sku, -failedQuantity, 'QC scrap: dimension failure');
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `create_bom` | Create a bill of materials |
| `explode_bom` | Calculate total material requirements |
| `create_work_order` | Create a manufacturing job |
| `start_work_order` | Begin production |
| `complete_work_order` | Record completion with yield |
| `create_purchase_order` | Create a PO for materials |
| `approve_purchase_order` | Approve for procurement |
| `receive_purchase_order` | Record receipt of materials |
| `list_suppliers` | List all suppliers |
| `create_quality_inspection` | Record a QC inspection |
| `create_lot` | Create a lot number |
| `assign_serial` | Assign a serial number |
| `get_work_order_cost` | Cost rollup for a work order |
