# B2B Operations

iCommerce supports business-to-business workflows: supplier management, purchase order lifecycle, receiving and inspection, invoicing, and accounts payable/receivable reconciliation.

## Supplier Management

### Registering Suppliers

```javascript
await toolkit.executeTool('create_supplier', {
    name: 'Acme Components Inc.',
    email: 'orders@acme-components.com',
    phone: '+1-555-0100',
    address: '123 Industrial Blvd, Chicago, IL 60601',
});
```

### Listing Suppliers

```javascript
const suppliers = await toolkit.executeTool('list_suppliers', {});
// → { count: 12, suppliers: [{ id: 'sup-001', name: 'Acme Components', ... }, ...] }
```

## Purchase Order Lifecycle

```
Draft → Submitted → Approved → Ordered → Partially Received → Received → Closed
                                                                        → Cancelled
```

### Creating a Purchase Order

```javascript
await toolkit.executeTool('create_purchase_order', {
    supplierId: 'sup-001',
    items: JSON.stringify([
        { sku: 'COMPONENT-X', name: 'Circuit Board Rev3', quantity: 500, unitPrice: 4.50 },
        { sku: 'COMPONENT-Y', name: 'LED Module', quantity: 1000, unitPrice: 0.85 },
    ]),
    notes: 'Rush order — needed by March 25',
});
```

### Approving a Purchase Order

```javascript
await toolkit.executeTool('approve_purchase_order', {
    purchaseOrderId: 'po-001',
    approvedBy: 'Operations Manager',
});
```

Use policies to enforce approval rules:

```yaml
# policies/purchase-orders.yaml
name: PO Approval
domain: purchase_orders
rules:
  - name: require-manager-over-5000
    conditions:
      - field: total_amount
        operator: greater_than
        value: 5000
    actions:
      - type: require-approval
        reason: "POs over $5,000 require manager approval"
```

### Sending a Purchase Order

```javascript
await toolkit.executeTool('send_purchase_order', {
    purchaseOrderId: 'po-001',
});
```

### Receiving Goods

```javascript
// Current MCP flow: record receipt per SKU with inventory adjustments
await toolkit.executeTool('adjust_inventory', {
    sku: 'COMPONENT-X',
    quantity: 300,
    reason: 'PO po-001 received: partial shipment',
});
```

The current MCP surface does not expose a dedicated `receive_purchase_order` or `close_purchase_order`
command. Receiving is recorded through inventory adjustments plus PO state tracking in the admin or
embedded application layer.

## Invoicing

### Creating an Invoice

```javascript
await toolkit.executeTool('create_invoice', {
    customerId: 'cust-enterprise-01',
    orderId: 'ord-456',
    items: JSON.stringify([
        { description: 'Premium Widget (qty 100)', quantity: 100, unitPrice: 29.99 },
        { description: 'Express Shipping', quantity: 1, unitPrice: 49.00 },
    ]),
    dueDate: '2026-04-15',
    notes: 'Net 30 terms per contract agreement',
});
```

### Invoice Lifecycle

```
Draft → Sent → Viewed → Paid → Closed
                      → Overdue → Paid
```

### Sending an Invoice

```javascript
await toolkit.executeTool('send_invoice', {
    invoiceId: 'inv-001',
});
```

### Recording Payment

```javascript
await toolkit.executeTool('record_invoice_payment', {
    invoiceId: 'inv-001',
    amount: 3048.00,
    paymentMethod: 'wire_transfer',
    reference: 'WIRE-20260317-001',
});
```

### Reviewing Overdue Invoices

```javascript
const overdue = await toolkit.executeTool('get_overdue_invoices', {});
```

## Purchase Order and Invoice Workflow

One realistic current-tool workflow:

```
1. Detect low stock         → heartbeat monitor triggers alert
2. Find suppliers           → list_suppliers with capability match
3. Create purchase order    → create_purchase_order with items
4. Approve PO               → approve_purchase_order (policy-gated, with approver)
5. Send PO                  → send_purchase_order
6. Receive goods            → adjust_inventory for each received SKU
7. Create customer invoice  → create_invoice
8. Record payment           → record_invoice_payment
```

For autonomous procurement via AI agents:

```bash
stateset-suppliers "our COMPONENT-X stock is at 50 units. Create a PO to Acme for 500 units."
```

## Accounting Integration

B2B operations flow into the accounting subsystem:

| Event | Accounting Impact |
|-------|------------------|
| PO approved | Accounts payable liability created |
| Goods received | Inventory asset increased |
| Supplier invoice recorded | AP balance confirmed |
| Payment made | Cash decreased, AP cleared |
| Customer invoice sent | Accounts receivable created |
| Customer payment received | Cash increased, AR cleared |

See [Accounting & Finance](accounting.md) for general ledger integration.

## Money-Integrity Guarantees

The procurement modules enforce these invariants on both storage backends
(SQLite and Postgres):

**Purchase orders**
- Goods can only be booked against a purchase order that has actually reached
  the supplier. `receive` is guarded by the PO state machine
  (`PurchaseOrderStatus::can_transition_to`), so receiving against a draft,
  pending-approval, approved-but-unsent, cancelled or completed PO is refused
  with a conflict — approval can no longer be bypassed by receiving.
- Receipts are bounded: the quantity check re-reads what is already received
  under a write lock, so concurrent partial receipts accumulate exactly and can
  never exceed the ordered quantity.
- Every other lifecycle transition (submit, approve, send, acknowledge, hold,
  complete, cancel) is a guarded transition that reports the blocking status
  rather than silently overwriting it.

**Supplier invoices**
- A payment cannot exceed the invoice's remaining balance, and no payment may
  be recorded against a voided, cancelled or written-off invoice. Paying the
  remaining balance exactly still succeeds.
- Payments carrying a `payment_id` are idempotent: recording the same payment
  against the same invoice twice counts once, so a retried request cannot
  double-pay. The idempotency check and the write share the payment's own
  transaction.
- Void, write-off, send and dispute are guarded transitions. A paid invoice
  cannot be voided or written off (that would erase collected receivable), and
  a voided or written-off invoice cannot be resurrected into an open state
  where its balance would reappear in aging and collections.
- Recalculating an invoice from its applications derives the status from the
  money without discarding an operator's own classification: a partly paid
  invoice under dispute stays disputed.

**Credit**
- The credit limit is enforced against the customer's whole exposure —
  `balance + outstanding holds` — not the balance alone, so reservations can
  no longer be circumvented by charging directly.
- A charge applies the balance, consumes its matching reservation and writes
  the ledger entry in one transaction: a crash can no longer double-consume
  credit or leave the ledger disagreeing with the balance.

**Cost accounting**
- Cost adjustments follow a guarded lifecycle (pending → approved → applied,
  or rejected). An applied adjustment cannot be re-approved, re-applied or
  retroactively rejected, so a revaluation moves the item cost exactly once
  and the audit trail always matches the live cost.
- Claiming an adjustment and writing the new item cost share one transaction,
  so concurrent applies cannot both move the cost.

## MCP Tools

| Tool | Description |
|------|-------------|
| `list_suppliers` | List all suppliers |
| `create_supplier` | Register a new supplier |
| `list_purchase_orders` | List purchase orders |
| `create_purchase_order` | Create a PO to a supplier |
| `approve_purchase_order` | Approve a PO (policy-gated) |
| `send_purchase_order` | Send an approved PO to the supplier |
| `adjust_inventory` | Record receipt of incoming stock |
| `list_invoices` | List all invoices |
| `create_invoice` | Create an invoice |
| `send_invoice` | Send invoice to customer |
| `record_invoice_payment` | Record payment against invoice |
| `get_overdue_invoices` | List overdue invoices |
