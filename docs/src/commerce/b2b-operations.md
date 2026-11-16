# B2B operations

Supplier purchasing, receiving, supplier bills and customer invoices are distinct workflows. A purchase order authorizes procurement; approving it does not create an accounts-payable journal. Physical receipts record stock and its cost. Approved supplier bills establish the payable, and allocated payments or credits settle it.

## Purchase and receive

The Node example below uses an existing `commerce` instance. Store the returned IDs; they identify the purchase lines and physical receiving destination.

```javascript
const supplier = await commerce.purchaseOrders.createSupplier({ name: 'Acme Components' });
const warehouse = await commerce.warehouse.createWarehouse({ code: 'MAIN', name: 'Main' });
const dock = await commerce.warehouse.createLocation({
  warehouseId: warehouse.id,
  code: 'RECEIVING',
  locationType: 'receiving',
  isReceivable: true,
});
const po = await commerce.purchaseOrders.create({
  supplierId: supplier.id,
  currency: 'USD',
  items: [{ sku: 'COMPONENT-X', name: 'Circuit board', quantity: 10, unitCostExact: '4.50' }],
});
await commerce.purchaseOrders.submit(po.id);
await commerce.purchaseOrders.approve(po.id, 'operator:procurement');
await commerce.purchaseOrders.send(po.id);
const [line] = await commerce.purchaseOrders.getItems(po.id);
await commerce.purchaseOrders.receive(
  po.id,
  [{ itemId: line.id, quantityReceivedExact: '6' }],
  warehouse.id,
  dock.id,
);
```

The six accepted units are booked once into the selected warehouse and location. The PO remains partially received until the rest arrives. Concurrent partial receipts cannot exceed the ordered quantity. Receiving against draft, unapproved, unsent or terminal POs is refused. Foreign-currency receipts require a historical exchange rate into the store's base currency.

MCP's `operations` profile exposes `create_purchase_order`, `submit_purchase_order`, `approve_purchase_order`, `send_purchase_order`, `get_purchase_order_items` and `receive_purchase_order`. PO items are a JSON array string with integer `quantity` and decimal-string `unitCostExact`. Receiving takes explicit `warehouseId`, `receivingLocationId` and line quantities.

For a separate receiving workflow, use `create_receipt_from_purchase_order`, then `get_receipt` to obtain receipt-line IDs, `start_receiving`, `receive_receipt_items` and `complete_receiving`. Put-away uses `create_put_away`, `assign_put_away`, `start_put_away` and `complete_put_away`. A manually constructed PO receipt requires its supplier and valid PO-linked lines. Do not substitute a generic inventory adjustment for a purchase receipt: it does not record receipt provenance or advance the PO.

## Supplier bills and settlement

Create a bill with its supplier, currency, due date and exact line quantities/prices. Link its PO where applicable. Bill approval compares cumulative billing with ordered and accepted quantities/prices. A mismatch requires an explicit persisted override reason. The engine refuses self-approval: a trusted host supplies the maker and a different approver identity.

The finance MCP profile exposes `create_bill`, `approve_bill`, `create_bill_payment` and payment-run creation/approval/processing. Configure each endpoint's trusted `--actor`, permission gate and `--apply` posture. Approvals and payments require administrator authority. Tool arguments cannot select the acting identity.

Payments, supplier prepayments and vendor credits validate supplier, currency, available funds and remaining target balance in one transaction. A settlement reduces both the funding source and the bill or payment obligation. Disputed/cancelled bills cannot be paid; a bill with active settlements cannot be cancelled. Runs cannot mix currencies, and a run's maker cannot approve it.

## Customer invoices

Invoice quantities and amounts cross the Node finance API as decimal strings. Issue the invoice before applying payments, credit memos or write-offs.

```javascript
const invoice = await commerce.invoices.create({
  customerId: customer.id,
  currency: 'USD',
  items: [{ description: 'Components', quantity: '6', unitPrice: '9.99' }],
});
await commerce.invoices.send(invoice.id);
await commerce.invoices.recordPayment(invoice.id, {
  amount: '59.94',
  paymentId: receiptId, // a stable payment UUID, reused for retries
  paymentMethod: 'wire_transfer',
  reference: 'BANK-001',
});
```

`customer` and `receiptId` are application-owned values. Reusing a payment UUID with the same invoice and amount replays the original result; conflicting reuse refuses. Payment allocations from the payments repository require completed funds, the same customer/currency and sufficient value after refunds. Unapply an allocation before refunding its funds.

Partial write-offs reduce `balanceDue` without counting as cash collected. Their reversal restores the remaining receivable; terminal documents cannot be reopened through unapplication. Read the stored `balanceDue` instead of recomputing `total - amountPaid`. Draft invoices are excluded from aging and credit exposure. Historical statements use dated invoice, payment, credit and write-off events and require a selected currency when the customer has more than one.

Configured customer credit limits cover all unpaid orders and issued invoices, plus manual charges and reservations. Linked order/invoice exposure is accounted for without charging the same liability twice; unlinked legacy adjustments require reconciliation.

See [Accounting & Finance](accounting.md), the generated [tool catalog](https://github.com/stateset/stateset-icommerce/blob/master/cli/docs/TOOLS.md), and the [ERP upgrade guide](../guides/erp-upgrade.md) for posting configuration, permissions, retry limits and legacy-data reconciliation.
