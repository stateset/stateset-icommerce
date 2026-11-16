# Manufacturing & Supply Chain

iCommerce supports bills of materials, stock-backed work orders, production batches, receiving and supplier returns. Quantities, state transitions and physical stock routing are enforced by the database transaction on SQLite and PostgreSQL.

## Bills of materials

```javascript
const bom = await commerce.bom.create({ productId: product.id, name: 'Widget assembly' });
await commerce.bom.addComponent(bom.id, {
  componentSku: 'PART-A', name: 'Housing', quantity: 2,
});
await commerce.bom.activate(bom.id);
```

Components require valid identities and positive quantities. Revisions are unique, cycles are rejected, and published BOMs are immutable. Create a new revision for engineering changes. Recursive costing multiplies component quantities through nested assemblies and rejects cyclic graphs.

## Stock-backed work orders

```javascript
const workOrder = await commerce.workOrders.create({
  productId: product.id,
  bomId: bom.id,
  quantityToBuildExact: '1.5',
  stock: {
    sourceLocationId: componentShelf.id,
    outputLocationId: finishedGoodsShelf.id,
    outputSku: 'WIDGET',
  },
});
await commerce.workOrders.start(workOrder.id);
await commerce.workOrders.completeExact(workOrder.id, '0.75');
await commerce.workOrders.completeExact(workOrder.id, '0.75');
```

Creation snapshots the BOM materials. Release/start reserves components; consumption and finished output post atomically at explicit locations. Partial completion uses decimal quantities. Completion cannot exceed the remaining build quantity. Generic updates cannot reset lifecycle status or quantities already completed. Starting or completing from a prohibited state returns a conflict, including through HTTP.

Work-order material routes support listing/adding materials and explicit consumption. Additions must respect the work-order state and stock requirements. Quality holds and lot quarantine are checked by the shared movement path, including manufacturing consumption.

## Purchasing and receiving

```javascript
const supplier = await commerce.purchaseOrders.createSupplier({ name: 'Acme Parts' });
const po = await commerce.purchaseOrders.create({
  supplierId: supplier.id,
  items: [{ sku: 'PART-A', name: 'Housing', quantity: 100, unitCostExact: '2.50' }],
});
await commerce.purchaseOrders.submit(po.id);
await commerce.purchaseOrders.approve(po.id, 'user:buyer');
await commerce.purchaseOrders.send(po.id);
const [line] = await commerce.purchaseOrders.getItems(po.id);
await commerce.purchaseOrders.receive(
  po.id, [{ itemId: line.id, quantityReceivedExact: '100' }],
  warehouse.id, receivingLocation.id,
);
```

The convenience receive call requires an explicit warehouse and receiving location. It routes through canonical receiving and stock posting. PO lines can be edited only before approval. A receipt cannot exceed expected quantities, and accepted/rejected quantities determine line status at completion. Serial counts are validated; accepted serials are registered in the same transaction and remain unavailable for allocation until put-away.

For separate receiving and shelf placement use `receiving.createReceiptFromPo`, `getReceiptItems`, `startReceiving`, `receiveItems`, `completeReceiving`, `createPutAway` and `completePutAway`. MCP exposes these steps, including put-away assignment, start and cancellation. `get_receipt` returns the line IDs needed by `receive_receipt_items`. See [Inventory & Warehousing](inventory.md).

## Batches, costs and returns

```javascript
const batch = await commerce.productionBatches.create({
  name: 'Widget run', workOrderIds: [workOrder.id],
});
const costs = await commerce.costAccounting.getItemCost('WIDGET');
```

Production batches store relational work-order membership and derive progress from their members. Invalid legacy links require reconciliation. The admin manufacturing setup screen lists component recipes and batch members and supports draft BOM creation, adding components, publication, batch creation and exact item-cost settings.

Processing a vendor return requires a valid supplier, SKU and physical source location. It removes the returned stock atomically and can create one linked vendor credit. A quality hold or quarantine that blocks the stock must be resolved through its own workflow first. A vendor credit settles AP bills or payment obligations; it does not automatically represent every accounting treatment of a customer credit memo.

Manufacturing routings, capacity planning, MRP, engineering-change approvals, automatic rework and generalized scrap/backflush accounting are not provided by these workflows. Do not use a completed work-order edit to undo production; use supported stock/accounting corrections with an audit trail.
