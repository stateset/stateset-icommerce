# Inventory & Warehousing

The engine maintains warehouse balances, physical location stock, bin placement, lot custody and cost layers through a shared transactional stock path. SQLite serializes writers; PostgreSQL locks affected stock and document rows.

## Read and reserve stock

```javascript
const level = await commerce.inventory.getStock('WIDGET');
console.log(level.totalOnHand, level.totalAllocated, level.totalAvailable);
```

On-hand includes picked goods held in an explicit staging location until shipment. Picking changes custody; shipment removes stock. Reservations own particular stock slices so general inventory and lot-specific consumption cannot spend the same units concurrently. Quality holds and quarantine block the applicable movement/allocation paths.

Warehouse totals include unbinned physical locations as well as bins. A bin is a subdivision of physical placement, not an additional copy of inventory. Do not sum bin and warehouse balances together. Bin adjustments update their warehouse total atomically; moves preserve quantity.

## Receive and put away

```javascript
const receipt = await commerce.receiving.createReceiptFromPo(po.id, warehouse.id);
const [line] = await commerce.receiving.getReceiptItems(receipt.id);
await commerce.receiving.startReceiving(receipt.id);
await commerce.receiving.receiveItems({
  receiptId: receipt.id,
  receivingLocationId: dock.id,
  items: [{ receiptItemId: line.id, quantityReceived: '10' }],
});
await commerce.receiving.completeReceiving(receipt.id);
const task = await commerce.receiving.createPutAway({
  receiptId: receipt.id, receiptItemId: line.id,
  sku: line.sku, toLocationId: shelf.id, quantity: '10',
});
await commerce.receiving.startPutAway(task.id);
await commerce.receiving.completePutAway({ putAwayId: task.id });
```

Receiving posts accepted goods at the specified dock; put-away moves those same goods to the shelf. Each receipt line belongs to its receipt and cannot receive beyond the expected quantity. A failed multi-line receive rolls back every line. Repeating completion cannot add the goods twice. Cancellation and assignment follow the task state machine.

Accepted serials are registered transactionally and become allocatable only after put-away. PO-sourced backorder fulfillment consumes stock from the completed receiving destinations and bounds attribution to those receipts; it cannot fulfil merely because a PO line exists.

## Transfers and returns

Transfer shipping removes physical source stock into owned transit. Partial destination receipts reduce transit and add destination stock. Explicit receipt closure records remaining shortages; it does not create missing goods. Owned transit remains part of valuation until received or written off through the supported close workflow. Source/destination locations are required where physical routing matters.

Vendor-return processing removes stock from the named source and may create a linked supplier credit in the same transaction. Lot splits/merges preserve physical placements and reservations. Holds must be released or resolved before blocked stock moves.

## Counts, snapshots and valuation

Cycle counts validate their scope and commit variances atomically. Stock snapshots read stored balances, and aging reconstructs movements and cost-layer history at the requested date. Caller-supplied report lines cannot replace the underlying stock records.

Receipts and issues maintain FIFO, LIFO, average, standard or specific-lot carrying values as configured per SKU. Historical inconsistencies require reconciliation; changing the cost method while stock is held cannot silently rewrite carrying value.

The operations MCP profile is smoke-gated. Missing records, invalid states and unavailable external prerequisites must produce clean refusals. Successful receiving and put-away are also exercised against a real native store in CI.
